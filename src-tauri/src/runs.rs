//! Cancellation for long jobs that spend the user's quota.
//!
//! Checking a flag between pages is not enough: a page takes a minute or two,
//! so a run would keep burning quota long after the teacher asked it to stop.
//! The processes are therefore killed outright, and the flag only stops the
//! queue from starting the next one.
//!
//! Pages already read are kept. They cost real quota, and page recognition is
//! independent — a partial transcript is worth more than none.

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct Run {
    cancelled: bool,
    /// Live `claude` processes belonging to this run.
    pids: Vec<u32>,
}

static RUNS: Mutex<Option<HashMap<String, Run>>> = Mutex::new(None);

fn with<T>(action: impl FnOnce(&mut HashMap<String, Run>) -> T) -> T {
    let mut guard = RUNS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    action(guard.get_or_insert_with(HashMap::new))
}

/// Job keys. Reading and correcting a course are separate jobs, so cancelling
/// one must never stop the other.
pub fn reading(id: &str) -> String {
    format!("read:{id}")
}

pub fn correcting(id: &str) -> String {
    format!("fix:{id}")
}

/// Marks a job as running, clearing anything left by a previous one.
pub fn begin(id: &str) {
    with(|runs| {
        runs.insert(id.to_string(), Run::default());
    });
}

pub fn finish(id: &str) {
    with(|runs| {
        runs.remove(id);
    });
}

pub fn is_cancelled(id: &str) -> bool {
    with(|runs| runs.get(id).map(|run| run.cancelled).unwrap_or(false))
}

/// Records a child process so it can be killed if the teacher cancels.
pub fn watch(id: &str, pid: u32) {
    with(|runs| {
        if let Some(run) = runs.get_mut(id) {
            run.pids.push(pid);
        }
    });
}

pub fn unwatch(id: &str, pid: u32) {
    with(|runs| {
        if let Some(run) = runs.get_mut(id) {
            run.pids.retain(|known| *known != pid);
        }
    });
}

/// Kills a process without pulling in a libc dependency for one call.
fn kill(pid: u32) {
    #[cfg(windows)]
    let killer = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output();

    #[cfg(not(windows))]
    let killer = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output();

    let _ = killer;
}

/// Requests cancellation and stops the work already in flight.
///
/// Returns how many processes were killed, which is what the console reports —
/// "cancelled" with nothing stopped would be a lie worth catching.
pub fn cancel(id: &str) -> usize {
    let pids = with(|runs| {
        runs.get_mut(id).map(|run| {
            run.cancelled = true;
            std::mem::take(&mut run.pids)
        })
    });

    let Some(pids) = pids else { return 0 };
    for pid in &pids {
        kill(*pid);
    }
    pids.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_reports_cancellation_only_after_being_cancelled() {
        begin("doc");
        assert!(!is_cancelled("doc"));
        watch("doc", 999_999);
        assert_eq!(cancel("doc"), 1, "the watched process must be counted");
        assert!(is_cancelled("doc"));
        finish("doc");
        assert!(!is_cancelled("doc"), "a finished run leaves no state behind");
    }

    #[test]
    fn reading_and_correcting_are_independent_jobs() {
        let read = reading("cours");
        let fix = correcting("cours");
        assert_ne!(read, fix);

        begin(&read);
        begin(&fix);
        cancel(&read);
        assert!(is_cancelled(&read));
        assert!(!is_cancelled(&fix), "cancelling a read must not stop a correction");
        finish(&read);
        finish(&fix);
    }

    #[test]
    fn cancelling_an_unknown_run_is_harmless() {
        assert_eq!(cancel("never-started"), 0);
        assert!(!is_cancelled("never-started"));
    }

    #[test]
    fn a_finished_process_is_no_longer_a_target() {
        begin("doc2");
        watch("doc2", 1);
        watch("doc2", 2);
        unwatch("doc2", 1);
        assert_eq!(cancel("doc2"), 1);
        finish("doc2");
    }
}

#[cfg(test)]
mod kill_tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// The bookkeeping tests above only prove the list is managed. This one
    /// proves the point of the module: a live process actually dies.
    #[test]
    #[cfg(not(windows))]
    fn cancelling_kills_a_running_process() {
        let mut child = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn sleep");

        begin("kill-test");
        watch("kill-test", child.id());

        assert!(
            child.try_wait().unwrap().is_none(),
            "the process should still be running before cancelling"
        );

        assert_eq!(cancel("kill-test"), 1);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut exited = false;
        while Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                exited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        finish("kill-test");
        assert!(exited, "cancel must stop the process, not just set a flag");
    }
}
