//! Installs the LaTeX engine, so Plume needs nothing preinstalled.
//!
//! Requiring MacTeX — five gigabytes — was the last real barrier to installing
//! Plume on someone else's machine. Tectonic is a single binary that fetches
//! the LaTeX packages it needs on demand, which fits a teacher who wants an app,
//! not a TeX distribution.
//!
//! It is downloaded rather than bundled: the copy stays current, and updating
//! the engine later will not mean shipping a new version of Plume.
//!
//! No HTTP crate: `curl` and `tar` ship with macOS, Windows 10+ and Linux, and
//! this is the same trade the HEIC converters make. One less dependency to keep
//! up to date, and no TLS stack to vendor.

use crate::logbus;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Official source. Pinned to the project's own repository on purpose: this is
/// the one place Plume fetches an executable from, and it should never be
/// configurable from the interface.
const RELEASES_API: &str =
    "https://api.github.com/repos/tectonic-typesetting/tectonic/releases?per_page=20";

/// Used when the API cannot be reached — a rate limit should not leave the
/// teacher unable to install anything.
const FALLBACK_VERSION: &str = "0.15.0";
const FALLBACK_BASE: &str =
    "https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%400.15.0";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const TARGET: &str = "aarch64-apple-darwin";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const TARGET: &str = "x86_64-apple-darwin";
#[cfg(target_os = "windows")]
const TARGET: &str = "x86_64-pc-windows-msvc";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const TARGET: &str = "x86_64-unknown-linux-gnu";
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
const TARGET: &str = "unsupported";

#[cfg(windows)]
const BINARY: &str = "tectonic.exe";
#[cfg(not(windows))]
const BINARY: &str = "tectonic";

/// Where Plume keeps things it manages itself — never the user's workbook,
/// which belongs to them.
fn app_data() -> PathBuf {
    let home = crate::env_check::home_dir().unwrap_or_else(|| PathBuf::from("."));

    #[cfg(target_os = "macos")]
    let base = home.join("Library").join("Application Support");
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or(home);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = home.join(".local").join("share");

    base.join("app.plume.desktop").join("engine")
}

/// The managed engine, if it has already been installed.
pub fn installed() -> Option<PathBuf> {
    let path = app_data().join(BINARY);
    path.is_file().then_some(path)
}

fn run(tool: &str, args: &[&str]) -> Result<String, String> {
    let binary = crate::env_check::resolve_tool(tool)
        .ok_or_else(|| format!("« {tool} » est introuvable sur cette machine."))?;

    let output = Command::new(binary)
        .args(args)
        .output()
        .map_err(|e| format!("Lancement de {tool} : {e}"))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        return Err(format!("{tool} a échoué. {detail}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Picks the newest release carrying an asset for this platform.
///
/// Matching by substring rather than reconstructing a file name: the release
/// naming has drifted before, and a wrong guess would be a download that fails
/// for reasons nobody can read.
fn find_download() -> (String, String) {
    let archive = if cfg!(windows) { ".zip" } else { ".tar.gz" };

    let found = run("curl", &["-fsSL", RELEASES_API]).ok().and_then(|body| {
        let json: serde_json::Value = serde_json::from_str(&body).ok()?;
        let releases = json.as_array()?;

        releases
            .iter()
            .filter(|release| release["prerelease"].as_bool() != Some(true))
            .find_map(|release| {
                let tag = release["tag_name"].as_str().unwrap_or("").to_string();
                let url = release["assets"].as_array()?.iter().find_map(|asset| {
                    let name = asset["name"].as_str()?;
                    (name.contains(TARGET) && name.ends_with(archive))
                        .then(|| asset["browser_download_url"].as_str())
                        .flatten()
                })?;
                Some((tag, url.to_string()))
            })
    });

    found.unwrap_or_else(|| {
        logbus::warn(
            "latex",
            "Liste des versions inaccessible — repli sur la version connue.",
        );
        (
            format!("tectonic@{FALLBACK_VERSION}"),
            format!("{FALLBACK_BASE}/tectonic-{FALLBACK_VERSION}-{TARGET}{archive}"),
        )
    })
}

fn extract(archive: &Path, into: &Path) -> Result<(), String> {
    // bsdtar reads zip as well as tar, and ships with Windows 10+.
    run(
        "tar",
        &[
            "-xf",
            &archive.to_string_lossy(),
            "-C",
            &into.to_string_lossy(),
        ],
    )
    .map(|_| ())
}

/// Downloads and installs the engine, reporting each step.
///
/// The steps are coarse on purpose — this runs once, and a teacher wants to know
/// it is progressing, not the byte count.
pub fn install(on_step: &dyn Fn(&str)) -> Result<PathBuf, String> {
    if TARGET == "unsupported" {
        return Err("Aucun moteur Tectonic n'est publié pour cette plateforme.".into());
    }
    if let Some(existing) = installed() {
        return Ok(existing);
    }

    let dir = app_data();
    fs::create_dir_all(&dir).map_err(|e| format!("Dossier du moteur : {e}"))?;

    on_step("Recherche de la dernière version…");
    let (tag, url) = find_download();
    logbus::detail("latex", format!("Téléchargement de Tectonic {tag}"), url.clone());

    // A scratch directory, so a failed attempt never leaves a half-extracted
    // engine that `installed()` would then report as ready.
    let staging = dir.join(".download");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| format!("Dossier temporaire : {e}"))?;

    let archive = staging.join(if cfg!(windows) { "tectonic.zip" } else { "tectonic.tar.gz" });

    on_step("Téléchargement du moteur…");
    run(
        "curl",
        &["-fsSL", "--retry", "2", "-o", &archive.to_string_lossy(), &url],
    )
    .map_err(|e| {
        let _ = fs::remove_dir_all(&staging);
        format!("Téléchargement impossible : {e}")
    })?;

    on_step("Extraction…");
    extract(&archive, &staging).map_err(|e| {
        let _ = fs::remove_dir_all(&staging);
        e
    })?;

    // The archive layout has varied; find the binary wherever it landed.
    let binary = find_binary(&staging).ok_or_else(|| {
        let _ = fs::remove_dir_all(&staging);
        "L'archive téléchargée ne contient pas le moteur attendu.".to_string()
    })?;

    let destination = dir.join(BINARY);
    fs::rename(&binary, &destination)
        .or_else(|_| fs::copy(&binary, &destination).map(|_| ()))
        .map_err(|e| format!("Installation du moteur : {e}"))?;
    let _ = fs::remove_dir_all(&staging);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&destination, fs::Permissions::from_mode(0o755));
    }

    // Running it is the real check: a truncated download extracts happily and
    // then fails at the worst possible moment, mid-compilation.
    on_step("Vérification…");
    let version = Command::new(&destination)
        .arg("--version")
        .output()
        .map_err(|e| format!("Le moteur téléchargé ne démarre pas : {e}"))?;

    if !version.status.success() {
        let _ = fs::remove_file(&destination);
        return Err("Le moteur téléchargé ne répond pas correctement.".into());
    }

    logbus::detail(
        "latex",
        format!(
            "Moteur LaTeX installé — {}",
            String::from_utf8_lossy(&version.stdout).trim()
        ),
        destination.to_string_lossy().to_string(),
    );
    Ok(destination)
}

fn find_binary(root: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_binary(&path) {
                return Some(found);
            }
        } else if path.file_name().map(|n| n == BINARY).unwrap_or(false) {
            return Some(path);
        }
    }
    None
}

/// Removes the managed engine, so a broken install can be redone.
pub fn remove() -> Result<(), String> {
    let dir = app_data();
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("Suppression du moteur : {e}"))?;
        logbus::info("latex", "Moteur LaTeX supprimé");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_platform_has_a_known_target() {
        assert_ne!(TARGET, "unsupported", "add a Tectonic target for this platform");
    }

    #[test]
    fn the_engine_lives_outside_the_workbook() {
        let engine = app_data();
        let workbook = crate::workspace::root();
        assert!(
            !engine.starts_with(&workbook),
            "a managed binary must not sit in the user's documents"
        );
    }

    #[test]
    fn the_fallback_url_matches_this_platform() {
        let archive = if cfg!(windows) { ".zip" } else { ".tar.gz" };
        let url = format!("{FALLBACK_BASE}/tectonic-{FALLBACK_VERSION}-{TARGET}{archive}");
        assert!(url.starts_with("https://github.com/tectonic-typesetting/"));
        assert!(url.contains(TARGET));
    }

    #[test]
    fn nothing_is_reported_installed_before_installing() {
        // Only meaningful on a machine that has not installed it yet; the
        // assertion is that the check reads the filesystem rather than guessing.
        let path = app_data().join(BINARY);
        assert_eq!(installed().is_some(), path.is_file());
    }
}
