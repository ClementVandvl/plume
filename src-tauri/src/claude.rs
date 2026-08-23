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
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

    let downloaded = Command::new(&curl)
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
    let outcome = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .output();

    #[cfg(not(windows))]
    let outcome = Command::new("bash").arg(&script).output();

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

    let version = Command::new(&installed)
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

/// Opens a terminal running `claude`, so the user can sign in.
///
/// Not automated on purpose: signing in opens a browser and involves the user's
/// own credentials. Plume brings them to the door and stops there.
pub fn open_login() -> Result<(), String> {
    let claude = crate::env_check::resolve_tool("claude")
        .ok_or("Claude Code n'est pas installé.")?;
    let path = claude.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    let launched = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            r#"tell application "Terminal" to do script "{} " & return"#,
            path.replace('\\', "\\\\").replace('"', "\\\"")
        ))
        .spawn()
        .and_then(|_| {
            Command::new("osascript")
                .args(["-e", r#"tell application "Terminal" to activate"#])
                .spawn()
        });

    #[cfg(target_os = "windows")]
    let launched = Command::new("cmd")
        .args(["/c", "start", "", "powershell", "-NoExit", "-Command"])
        .arg(&path)
        .spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let launched = ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"]
        .iter()
        .find_map(|terminal| {
            crate::env_check::resolve_tool(terminal)
                .and_then(|bin| Command::new(bin).arg("-e").arg(&path).spawn().ok())
        })
        .ok_or_else(|| std::io::Error::other("no terminal"))
        .map(|_| ());

    launched.map_err(|e| format!("Impossible d'ouvrir un terminal : {e}"))?;
    logbus::info("claude", "Terminal ouvert pour la connexion");
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

    #[test]
    fn the_scratch_directory_is_temporary() {
        let dir = scratch();
        assert!(dir.starts_with(std::env::temp_dir()));
        assert!(!dir.starts_with(crate::workspace::root()));
    }
}
