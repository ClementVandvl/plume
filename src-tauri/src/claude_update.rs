//! Keeps Claude Code current.
//!
//! Plume installs Claude Code once, then only ever runs it as `claude -p`.
//! Nothing guarantees that such an install moves on: a Homebrew or npm copy
//! never updates itself, and the teacher never opens the CLI to be told. The
//! version of the day Plume was set up stays — and a model alias like `opus`
//! resolves to whatever that version knew. Measured: a CLI a month old ran
//! Opus 5 when the teacher chose Opus, while Opus 5.5 had been out for weeks,
//! and nothing on screen said so.
//!
//! So Plume asks the official release channel what the latest version is, says
//! so when the installed one falls behind, and runs the update itself when the
//! teacher agrees. Never on its own: replacing a tool mid-reading is not a
//! decision to take on anyone's behalf.

use crate::logbus;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The channel the official installer reads. Plain text: `latest` holds a
/// version number, `<version>/manifest.json` its build date.
const RELEASES: &str = "https://downloads.claude.ai/claude-code-releases";

/// How far behind before the home screen says so. Claude Code ships almost
/// daily, so "any newer version" would be a permanent banner; a new model, on
/// the other hand, needs a CLI a few weeks recent at most. Two weeks separates
/// the two.
const STALE_AFTER_DAYS: i64 = 14;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeUpdate {
    pub installed: String,
    pub latest: String,
    /// Days between the installed build and the latest one; `None` when a
    /// build date could not be read.
    pub behind_days: Option<i64>,
    /// A newer version exists.
    pub available: bool,
    /// Far enough behind that newer models may be out of reach: said on the
    /// home screen, not only in the settings.
    pub important: bool,
}

/// `2.1.236 (Claude Code)` → `[2, 1, 236]`.
fn parse_version(text: &str) -> Option<Vec<u64>> {
    let word = text.split_whitespace().next()?;
    let parts: Option<Vec<u64>> = word
        .split(['.', '-'])
        .take(3)
        .map(|part| part.parse().ok())
        .collect();
    parts.filter(|parts| parts.len() == 3)
}

/// `[2, 1, 236]` → `2.1.236`.
fn plain(version: &[u64]) -> String {
    version.iter().map(u64::to_string).collect::<Vec<_>>().join(".")
}

/// `2026-08-19T16:49:51Z` → days since 1970-01-01. Only the date counts.
fn day_number(stamp: &str) -> Option<i64> {
    let mut parts = stamp.get(..10)?.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    // Howard Hinnant's days-from-civil, to spare a date crate one subtraction.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// What the check concludes, from the two versions and their build dates.
fn assess(
    installed: &str,
    latest: &str,
    installed_date: Option<&str>,
    latest_date: Option<&str>,
) -> Option<ClaudeUpdate> {
    let (have, newest) = (parse_version(installed)?, parse_version(latest)?);
    let available = newest > have;

    let behind_days = match (installed_date.and_then(day_number), latest_date.and_then(day_number)) {
        (Some(from), Some(to)) => Some((to - from).max(0)),
        _ => None,
    };

    // Without dates, a change of major or minor number stands in for age.
    let important = available
        && match behind_days {
            Some(days) => days >= STALE_AFTER_DAYS,
            None => newest[..2] != have[..2],
        };

    Some(ClaudeUpdate {
        installed: plain(&have),
        latest: plain(&newest),
        behind_days,
        available,
        important,
    })
}

fn fetch(url: &str) -> Result<String, String> {
    let curl = crate::env_check::resolve_tool("curl")
        .ok_or("« curl » est introuvable sur cette machine.")?;
    let output = crate::proc::quiet(curl)
        .args(["-fsSL", "--max-time", "10", url])
        .output()
        .map_err(|e| format!("Lancement de curl : {e}"))?;
    if !output.status.success() {
        return Err(format!("{url} ne répond pas."));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn build_date(version: &str) -> Option<String> {
    let manifest = fetch(&format!("{RELEASES}/{version}/manifest.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&manifest).ok()?;
    json.get("buildDate")?.as_str().map(str::to_string)
}

fn installed_version(claude: &Path) -> Option<String> {
    let out = crate::proc::quiet(claude).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    parse_version(&text).map(|_| text)
}

/// Compares the installed CLI with the latest release. `Ok(None)` when Claude
/// Code is not installed: there is nothing to update, the install row says the
/// rest.
pub fn check() -> Result<Option<ClaudeUpdate>, String> {
    let Some(claude) = crate::env_check::resolve_tool("claude") else {
        return Ok(None);
    };
    let installed = installed_version(&claude)
        .ok_or("Claude Code ne donne pas sa version.")?;

    let latest = fetch(&format!("{RELEASES}/latest"))?;
    // An error page from a proxy is not a version number.
    if parse_version(&latest).is_none() {
        return Err("Réponse inattendue du service de mises à jour de Claude Code.".into());
    }

    let installed_date = parse_version(&installed).and_then(|v| build_date(&plain(&v)));
    let latest_date = build_date(&latest);

    let update = assess(&installed, &latest, installed_date.as_deref(), latest_date.as_deref())
        .ok_or("Versions de Claude Code illisibles.")?;

    if update.available {
        logbus::info(
            "claude",
            format!(
                "Claude Code {} installé, {} disponible{}",
                update.installed,
                update.latest,
                update
                    .behind_days
                    .map(|days| format!(" ({days} jours d'écart)"))
                    .unwrap_or_default()
            ),
        );
    }
    Ok(Some(update))
}

/// How this copy of Claude Code gets updated.
#[derive(Debug, PartialEq)]
enum Channel {
    /// Installed by Homebrew, which owns the copy: updating it any other way
    /// would leave Homebrew's record and the binary disagreeing.
    Homebrew { kind: &'static str, name: String },
    /// The official installer or npm: the CLI updates itself.
    SelfManaged,
}

/// Read from where the binary really lives, links followed:
/// `/opt/homebrew/bin/claude` → `/opt/homebrew/Caskroom/claude-code/2.1.236/claude`.
fn channel_of(real_path: &Path) -> Channel {
    let parts: Vec<String> = real_path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    for (marker, kind) in [("Caskroom", "--cask"), ("Cellar", "--formula")] {
        if let Some(at) = parts.iter().position(|p| p == marker) {
            if let Some(name) = parts.get(at + 1) {
                return Channel::Homebrew { kind, name: name.clone() };
            }
        }
    }
    Channel::SelfManaged
}

/// The last thing a command said, for an error the teacher can pass on.
fn last_words(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .last()
        .unwrap_or("")
        .to_string()
}

/// Updates Claude Code the way it was installed, and returns the version now
/// in place.
pub fn update(on_step: &dyn Fn(&str)) -> Result<String, String> {
    let claude = crate::env_check::resolve_tool("claude")
        .ok_or("Claude Code n'est pas installé.")?;
    let before = installed_version(&claude).unwrap_or_default();
    let real: PathBuf = std::fs::canonicalize(&claude).unwrap_or_else(|_| claude.clone());

    on_step("Mise à jour de Claude Code…");

    let output = match channel_of(&real) {
        Channel::Homebrew { kind, name } => {
            let brew = crate::env_check::resolve_tool("brew")
                .ok_or("Claude Code vient de Homebrew, mais « brew » est introuvable.")?;
            logbus::detail("claude", "Mise à jour par Homebrew", format!("brew upgrade {kind} {name}"));
            crate::proc::quiet(brew).args(["upgrade", kind, &name]).output()
        }
        Channel::SelfManaged => {
            logbus::detail("claude", "Mise à jour", format!("{} update", claude.display()));
            crate::proc::quiet(&claude).arg("update").output()
        }
    }
    .map_err(|e| format!("Mise à jour de Claude Code impossible à lancer : {e}"))?;

    on_step("Vérification…");

    // A path under the old version's folder may be gone now: the lookup is
    // redone rather than reusing it.
    crate::env_check::forget_path();
    let after = crate::env_check::resolve_tool("claude")
        .and_then(|path| installed_version(&path))
        .ok_or("Claude Code ne répond plus après la mise à jour. Redémarrez Plume.")?;

    let moved = parse_version(&after) > parse_version(&before);
    if !output.status.success() && !moved {
        let detail = last_words(&output);
        logbus::error("claude", format!("Mise à jour échouée. {detail}"));
        return Err(format!("La mise à jour a échoué. {detail}"));
    }

    logbus::info("claude", format!("Claude Code : {before} → {after}"));
    Ok(after)
}

/// Whether a failure is the CLI saying it is too old for the model asked:
/// `Claude Code 2.1.236 does not support this model; version 2.1.280 or newer
/// is required`.
pub fn is_outdated_failure(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("does not support this model") || lower.contains("or newer is required")
}

/// The message a reading fails with when the CLI is too old for its model.
pub const OUTDATED: &str =
    "Claude Code est trop ancien pour ce modèle : mettez-le à jour depuis les Réglages";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_as_the_cli_prints_them() {
        assert_eq!(parse_version("2.1.236 (Claude Code)"), Some(vec![2, 1, 236]));
        assert_eq!(parse_version("2.1.281"), Some(vec![2, 1, 281]));
        assert_eq!(parse_version("<html>"), None);
        assert_eq!(parse_version(""), None);
    }

    /// Compared as numbers: as text, 2.1.99 would pass for newer than 2.1.281.
    #[test]
    fn versions_compare_as_numbers() {
        let update = assess("2.1.99", "2.1.281", None, None).unwrap();
        assert!(update.available);
    }

    /// The case that prompted all this: five weeks behind, Opus 5.5 out of reach.
    #[test]
    fn a_month_behind_is_worth_saying_on_the_home_screen() {
        let update = assess(
            "2.1.236 (Claude Code)",
            "2.1.281",
            Some("2026-08-19T16:49:51Z"),
            Some("2026-09-23T02:33:18Z"),
        )
        .unwrap();
        assert_eq!(update.installed, "2.1.236");
        assert_eq!(update.behind_days, Some(35));
        assert!(update.available && update.important);
    }

    /// A few days behind is the normal state of a CLI that ships daily.
    #[test]
    fn a_few_days_behind_stays_in_the_settings() {
        let update =
            assess("2.1.278", "2.1.281", Some("2026-09-19T10:00:00Z"), Some("2026-09-23T02:33:18Z"))
                .unwrap();
        assert!(update.available);
        assert!(!update.important);
    }

    #[test]
    fn without_dates_a_minor_version_change_stands_in_for_age() {
        assert!(!assess("2.1.236", "2.1.281", None, None).unwrap().important);
        assert!(assess("2.1.236", "2.2.0", None, None).unwrap().important);
    }

    #[test]
    fn a_current_or_newer_install_has_nothing_to_do() {
        for installed in ["2.1.281", "2.1.290"] {
            let update = assess(installed, "2.1.281", None, None).unwrap();
            assert!(!update.available && !update.important, "{installed}");
        }
    }

    #[test]
    fn day_numbers_cross_months_and_leap_years() {
        assert_eq!(day_number("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            day_number("2024-03-01").zip(day_number("2024-02-28")).map(|(a, b)| a - b),
            Some(2)
        );
        assert_eq!(day_number("bad"), None);
    }

    #[test]
    fn homebrew_copies_are_updated_by_homebrew() {
        assert_eq!(
            channel_of(Path::new("/opt/homebrew/Caskroom/claude-code/2.1.236/claude")),
            Channel::Homebrew { kind: "--cask", name: "claude-code".into() }
        );
        assert_eq!(
            channel_of(Path::new("/Users/x/.local/share/claude/versions/2.1.281")),
            Channel::SelfManaged
        );
    }

    #[test]
    fn a_too_old_cli_is_recognised() {
        assert!(is_outdated_failure(
            "API Error: 400 Claude Code 2.1.236 does not support this model; version 2.1.280 or newer is required. Run 'claude update'"
        ));
        assert!(!is_outdated_failure("Invalid API key"));
    }
}
