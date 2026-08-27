//! What this machine can afford.
//!
//! Reading three pages at once means three Claude Code processes, each several
//! hundred megabytes, on top of the app's own webview. On an 8 GB Windows
//! laptop that pushed the system into swap until it froze on a 47 MB
//! allocation. The parallelism therefore adapts to the memory actually
//! present — and the teacher can override it from the settings.

use std::sync::OnceLock;

static TOTAL_GB: OnceLock<Option<f64>> = OnceLock::new();

/// Total physical memory, measured once per run.
pub fn total_memory_gb() -> Option<f64> {
    *TOTAL_GB.get_or_init(detect)
}

#[cfg(target_os = "macos")]
fn detect() -> Option<f64> {
    let out = crate::proc::quiet("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    let bytes: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(bytes / (1024.0 * 1024.0 * 1024.0))
}

#[cfg(target_os = "windows")]
fn detect() -> Option<f64> {
    // Win32_OperatingSystem reports kilobytes.
    let out = crate::proc::quiet("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_OperatingSystem).TotalVisibleMemorySize",
        ])
        .output()
        .ok()?;
    let kb: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kb / (1024.0 * 1024.0))
}

#[cfg(target_os = "linux")]
fn detect() -> Option<f64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: f64 = text
        .lines()
        .find(|line| line.starts_with("MemTotal"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some(kb / (1024.0 * 1024.0))
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn detect() -> Option<f64> {
    None
}

/// Concurrent page reads for a given memory budget.
///
/// Each `claude` easily reaches half a gigabyte; below 10 GB the machine also
/// runs a browser and the webview, so one at a time is the honest choice.
/// Unknown memory gets the middle value rather than the historical three.
pub fn concurrency_for(total_gb: Option<f64>) -> usize {
    match total_gb {
        Some(gb) if gb < 10.0 => 1,
        Some(gb) if gb < 16.0 => 2,
        Some(_) => 3,
        None => 2,
    }
}

/// The parallelism used when the teacher left the setting on automatic.
pub fn auto_concurrency() -> usize {
    concurrency_for(total_memory_gb())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_rule_protects_small_machines() {
        assert_eq!(concurrency_for(Some(8.0)), 1, "the frozen 8 GB laptop");
        assert_eq!(concurrency_for(Some(9.9)), 1);
        assert_eq!(concurrency_for(Some(12.0)), 2);
        assert_eq!(concurrency_for(Some(16.0)), 3);
        assert_eq!(concurrency_for(Some(64.0)), 3);
        assert_eq!(concurrency_for(None), 2, "unknown must not assume plenty");
    }

    /// Detection must work on the platform running the suite.
    #[test]
    fn this_machine_reports_its_memory()  {
        let gb = total_memory_gb().expect("memory should be measurable here");
        assert!(gb > 0.5 && gb < 4096.0, "implausible memory: {gb} GB");
    }
}
