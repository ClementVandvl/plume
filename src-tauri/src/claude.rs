//! Installs Claude Code, and opens the terminal for the one step that cannot be
//! automated.
//!
//! Telling a teacher to open PowerShell and paste a command is exactly the kind
//! of obstacle Plume exists to remove. The official installer is fetched and run
//! from the app instead.
//!
//! Signing in is deliberately *not* automated. It is a credential flow, it opens
//! a browser, and it belongs to the user. Plume opens a terminal running
//! `claude` and gets out of the way.

use crate::logbus;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// Official installers, from the documented URLs. Never configurable: this is
/// the one place Plume runs a script it did not ship.
#[cfg(not(windows))]
const INSTALLER_URL: &str = "https://claude.ai/install.sh";
#[cfg(windows)]
const INSTALLER_URL: &str = "https://claude.ai/install.ps1";

fn scratch() -> PathBuf {
    std::env::temp_dir().join("plume-claude-install")
}

/// Downloads the installer, runs it, and confirms the result.
pub fn install(on_step: &dyn Fn(&str)) -> Result<PathBuf, String> {
    let curl = crate::env_check::resolve_tool("curl")
        .ok_or("« curl » est introuvable ; impossible de télécharger l'installateur.")?;

    let dir = scratch();
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("Dossier temporaire : {e}"))?;

    let script = dir.join(if cfg!(windows) { "install.ps1" } else { "install.sh" });

    on_step("Téléchargement de l'installateur…");
    logbus::detail("claude", "Téléchargement de l'installateur", INSTALLER_URL);

    let downloaded = crate::proc::quiet(&curl)
        .args(["-fsSL", "--retry", "2", "-o"])
        .arg(&script)
        .arg(INSTALLER_URL)
        .output()
        .map_err(|e| format!("Lancement de curl : {e}"))?;

    if !downloaded.status.success() || !script.is_file() {
        let _ = fs::remove_dir_all(&dir);
        return Err("Téléchargement de l'installateur impossible. Vérifiez votre connexion.".into());
    }

    on_step("Installation…");

    // The script is downloaded first rather than piped into a shell: a failed
    // download then cannot be executed as a truncated program, and the file is
    // there to inspect if the install goes wrong.
    #[cfg(windows)]
    let outcome = crate::proc::quiet("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .output();

    #[cfg(not(windows))]
    let outcome = crate::proc::quiet("bash").arg(&script).output();

    let outcome = outcome.map_err(|e| {
        let _ = fs::remove_dir_all(&dir);
        format!("Exécution de l'installateur : {e}")
    })?;

    let _ = fs::remove_dir_all(&dir);

    if !outcome.status.success() {
        let detail = String::from_utf8_lossy(&outcome.stderr)
            .lines()
            .last()
            .unwrap_or("")
            .trim()
            .to_string();
        logbus::error("claude", format!("Installation échouée. {detail}"));
        return Err(format!("L'installation a échoué. {detail}"));
    }

    on_step("Vérification…");

    // The freshly written binary is not on the PATH this process inherited, so
    // the lookup has to be redone from the login shell.
    crate::env_check::forget_path();
    let installed = crate::env_check::resolve_tool("claude").ok_or(
        "Claude Code semble installé mais reste introuvable. Redémarrez Plume.",
    )?;

    let version = crate::proc::quiet(&installed)
        .arg("--version")
        .output()
        .map_err(|e| format!("Le binaire installé ne démarre pas : {e}"))?;

    logbus::detail(
        "claude",
        format!(
            "Claude Code installé — {}",
            String::from_utf8_lossy(&version.stdout).trim()
        ),
        installed.to_string_lossy().to_string(),
    );
    Ok(installed)
}

/// The message a reading fails with when the CLI's session has lapsed. The
/// interface reads it to offer the sign-in rather than a generic error.
pub const AUTH_REQUIRED: &str = "Claude doit être reconnecté";

/// Whether the CLI is signed in, as it reports it.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub logged_in: bool,
    pub email: Option<String>,
    pub subscription: Option<String>,
    /// Why the answer is "no", when it is not simply "not signed in".
    pub detail: Option<String>,
}

/// Reads `claude auth status --json`.
fn parse_auth_status(json: &str) -> AuthStatus {
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(status) => AuthStatus {
            logged_in: status.get("loggedIn").and_then(|v| v.as_bool()).unwrap_or(false),
            email: status.get("email").and_then(|v| v.as_str()).map(str::to_string),
            subscription: status
                .get("subscriptionType")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            detail: None,
        },
        Err(_) => AuthStatus {
            detail: Some("Réponse illisible de « claude auth status ».".into()),
            ..AuthStatus::default()
        },
    }
}

/// Asks the CLI whether it is signed in. Local and instant: it reads its own
/// stored session and makes no request. A session revoked on the server side
/// still reads as signed in here, until the first real call fails — which is
/// why a reading also watches for the failure itself.
pub fn auth_status() -> AuthStatus {
    let Some(claude) = crate::env_check::resolve_tool("claude") else {
        return AuthStatus {
            detail: Some("Claude Code n'est pas installé.".into()),
            ..AuthStatus::default()
        };
    };
    match crate::proc::quiet(&claude).args(["auth", "status", "--json"]).output() {
        Ok(out) => parse_auth_status(&String::from_utf8_lossy(&out.stdout)),
        Err(error) => AuthStatus {
            detail: Some(format!("« claude auth status » impossible : {error}")),
            ..AuthStatus::default()
        },
    }
}

/// Whether a failure message is the CLI saying its session has lapsed.
///
/// Matched on wording because that is all the CLI gives: an exit code of 1 and
/// a sentence. The sentences seen so far all name the key, the login, or the
/// authentication; the list is kept short on purpose and extended from real
/// failures, not imagined ones.
pub fn is_auth_failure(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "invalid api key",
        "/login",
        "not logged in",
        "not authenticated",
        "authentication",
        "unauthorized",
        "oauth token",
        "please log in",
        "please sign in",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Opens a terminal running `claude auth login`.
///
/// A terminal still, because the sign-in is the CLI's own browser round trip
/// and it may need a console to finish. But nothing to type: the command is
/// given, the browser opens, the teacher authorises, done. The old way opened
/// a bare `claude` and left "/login" for the teacher to know about.
pub fn open_login() -> Result<(), String> {
    let claude = crate::env_check::resolve_tool("claude")
        .ok_or("Claude Code n'est pas installé.")?;
    let path = claude.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    let launched = crate::proc::quiet("osascript")
        .arg("-e")
        .arg(format!(
            r#"tell application "Terminal" to do script "\"{}\" auth login" & return"#,
            path.replace('\\', "\\\\").replace('"', "\\\"")
        ))
        .spawn()
        .and_then(|_| {
            crate::proc::quiet("osascript")
                .args(["-e", r#"tell application "Terminal" to activate"#])
                .spawn()
        });

    #[cfg(target_os = "windows")]
    let launched = crate::proc::quiet("cmd")
        .args(["/c", "start", "", "powershell", "-NoExit", "-Command"])
        .arg(format!("& '{}' auth login", path.replace('\'', "''")))
        .spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let launched = ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"]
        .iter()
        .find_map(|terminal| {
            crate::env_check::resolve_tool(terminal).and_then(|bin| {
                std::process::Command::new(bin)
                    .arg("-e")
                    .arg(format!("{path} auth login"))
                    .spawn()
                    .ok()
            })
        })
        .ok_or_else(|| std::io::Error::other("no terminal"))
        .map(|_| ());

    launched.map_err(|e| format!("Impossible d'ouvrir un terminal : {e}"))?;
    logbus::info("claude", "Terminal ouvert pour « claude auth login »");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installer_url_is_official_and_fixed() {
        assert!(INSTALLER_URL.starts_with("https://claude.ai/install."));
        assert!(
            INSTALLER_URL.ends_with(if cfg!(windows) { ".ps1" } else { ".sh" }),
            "the installer must match the platform"
        );
    }

    /// What the CLI actually printed the day a reading died with "aucun
    /// message d'erreur" — and what it prints when everything is fine.
    #[test]
    fn a_lapsed_session_is_recognised_by_its_wording() {
        assert!(is_auth_failure("Invalid API key · Please run /login"));
        assert!(is_auth_failure("Error: Not logged in. Run `claude auth login`."));
        assert!(is_auth_failure("OAuth token has expired"));
        assert!(!is_auth_failure("La compilation a échoué : Undefined control sequence"));
        assert!(!is_auth_failure(""));
    }

    #[test]
    fn the_status_is_read_from_the_cli_s_json() {
        let signed = parse_auth_status(
            r#"{"loggedIn":true,"authMethod":"claude.ai","email":"prof@lycee.fr","subscriptionType":"max"}"#,
        );
        assert!(signed.logged_in);
        assert_eq!(signed.email.as_deref(), Some("prof@lycee.fr"));
        assert_eq!(signed.subscription.as_deref(), Some("max"));

        let out = parse_auth_status(r#"{"loggedIn":false}"#);
        assert!(!out.logged_in);
        assert!(out.detail.is_none(), "not signed in is not an error");

        let broken = parse_auth_status("not json");
        assert!(!broken.logged_in);
        assert!(broken.detail.is_some());
    }

    #[test]
    fn the_scratch_directory_is_temporary() {
        let dir = scratch();
        assert!(dir.starts_with(std::env::temp_dir()));
        assert!(!dir.starts_with(crate::workspace::root()));
    }
}
