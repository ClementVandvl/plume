//! Process spawning that stays invisible.
//!
//! On Windows, every console-subsystem child — `claude`, `curl`, a LaTeX
//! engine, even `taskkill` — opens its own terminal window unless the parent
//! says otherwise. Reading a course put three black consoles on top of the
//! app, one per concurrent page; macOS has no such notion, which is why the
//! problem never showed there.
//!
//! Every background spawn in Plume goes through here. The one deliberate
//! exception is opening a terminal for the Claude sign-in, where a window is
//! the whole point.

use std::ffi::OsStr;
use std::process::Command;

/// `CREATE_NO_WINDOW`: run the child without allocating a console.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A `Command` that will not open a console window on Windows.
pub fn quiet(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide(&mut command);
    command
}

#[cfg(windows)]
fn hide(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The flag must not change how a process runs or reports.
    #[test]
    #[cfg(not(windows))]
    fn a_quiet_command_still_runs_and_reports_status() {
        let ok = quiet("true").status().expect("spawn true");
        assert!(ok.success());
        let ko = quiet("false").status().expect("spawn false");
        assert!(!ko.success());
    }
}
