//! External prerequisite detection on macOS, Windows and Linux.
//!
//! Two platform traps:
//!  - on macOS an app launched from Finder does not inherit the terminal PATH.
//!    It gets `/usr/bin:/bin:...`, while `claude` lives in `/opt/homebrew/bin`
//!    or `~/.local/bin`. So we ask the login shell for the real PATH.
//!  - on Windows the process PATH is correct, but an executable may be named
//!    `claude.exe`, `claude.cmd` or `claude.bat`.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static SEARCH_PATH: OnceLock<Vec<PathBuf>> = OnceLock::new();

/// Directories added after start-up — where an installer just wrote a binary.
///
/// The search path is resolved once and cached, so something installed while
/// Plume is running would stay invisible until a restart. Rather than rebuild
/// the cache, the well-known install locations are simply searched as well.
static EXTRA_PATHS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Called after an install, so the next lookup sees what was just written.
pub fn forget_path() {
    let Ok(mut extra) = EXTRA_PATHS.lock() else { return };
    extra.clear();

    if let Some(home) = home_dir() {
        for candidate in [".local/bin", "AppData/Local/Programs", "bin"] {
            let dir = home.join(candidate);
            if dir.is_dir() {
                extra.push(dir);
            }
        }
    }
}

#[cfg(windows)]
const EXECUTABLE_SUFFIXES: &[&str] = &[".exe", ".cmd", ".bat", ""];
#[cfg(not(windows))]
const EXECUTABLE_SUFFIXES: &[&str] = &[""];

/// Directories to search for binaries, including the login-shell PATH on Unix.
fn search_path() -> &'static [PathBuf] {
    SEARCH_PATH.get_or_init(|| {
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();

        #[cfg(not(windows))]
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
            if let Some(from_login) = crate::proc::quiet(&shell)
                .args(["-l", "-c", "printf %s \"$PATH\""])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|p| !p.is_empty())
            {
                for dir in std::env::split_paths(&from_login) {
                    if !dirs.contains(&dir) {
                        dirs.push(dir);
                    }
                }
            }

            // Fallbacks in case the login shell does not answer.
            for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/Library/TeX/texbin"] {
                let dir = PathBuf::from(extra);
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                }
            }
        }

        if let Some(home) = home_dir() {
            for extra in [".local/bin", ".cargo/bin"] {
                let dir = home.join(extra);
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                }
            }
        }

        dirs
    })
}

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let key = "USERPROFILE";
    #[cfg(not(windows))]
    let key = "HOME";
    std::env::var_os(key).map(PathBuf::from)
}

/// Public entry point for other modules that need to launch a detected tool.
pub fn resolve_tool(binary: &str) -> Option<PathBuf> {
    resolve(binary)
}

fn resolve(binary: &str) -> Option<PathBuf> {
    let extra = EXTRA_PATHS.lock().map(|e| e.clone()).unwrap_or_default();

    search_path().iter().chain(extra.iter()).find_map(|dir| {
        EXECUTABLE_SUFFIXES
            .iter()
            .map(|suffix| dir.join(format!("{binary}{suffix}")))
            .find(|candidate| candidate.is_file())
    })
}

/// Runs `<bin> <arg>` and returns the first line of output.
fn first_line(bin: &PathBuf, arg: &str) -> Option<String> {
    let out = crate::proc::quiet(bin).arg(arg).output().ok()?;
    let text = if out.stdout.is_empty() { out.stderr } else { out.stdout };
    String::from_utf8_lossy(&text)
        .lines()
        .next()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub key: &'static str,
    pub label: &'static str,
    /// What Plume cannot do without it. Shown in the UI, hence French.
    pub role: &'static str,
    pub found: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    /// What to do when missing. Shown in the UI, hence French.
    pub hint: Option<&'static str>,
    /// Official install page for the current platform.
    pub install_url: &'static str,
    /// Plume can install this one itself, so the UI offers a button rather than
    /// a link out to a download page.
    pub installable: bool,
    pub required: bool,
}

struct Spec {
    key: &'static str,
    label: &'static str,
    role: &'static str,
    binaries: &'static [&'static str],
    version_arg: &'static str,
    hint: &'static str,
    install_url: &'static str,
    required: bool,
}

fn check(spec: &Spec) -> ToolStatus {
    // An engine Plume installed itself is not on the PATH, and reporting it
    // missing would send the teacher to install what they already have.
    let found = if spec.key == "latex" {
        crate::engine::installed().or_else(|| spec.binaries.iter().find_map(|b| resolve(b)))
    } else {
        spec.binaries.iter().find_map(|b| resolve(b))
    };

    ToolStatus {
        key: spec.key,
        label: spec.label,
        role: spec.role,
        found: found.is_some(),
        version: found.as_ref().and_then(|p| first_line(p, spec.version_arg)),
        path: found.as_ref().map(|p| p.to_string_lossy().to_string()),
        hint: if found.is_some() { None } else { Some(spec.hint) },
        install_url: spec.install_url,
        installable: matches!(spec.key, "latex" | "claude"),
        required: spec.required,
    }
}

/// The reference LaTeX distribution depends on the platform.
#[cfg(target_os = "macos")]
const LATEX_INSTALL_URL: &str = "https://www.tug.org/mactex/";
#[cfg(target_os = "windows")]
const LATEX_INSTALL_URL: &str = "https://miktex.org/download";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const LATEX_INSTALL_URL: &str = "https://tug.org/texlive/";

#[cfg(target_os = "macos")]
const LATEX_HINT: &str = "Plume peut installer Tectonic pour vous, en un clic.";
#[cfg(target_os = "windows")]
const LATEX_HINT: &str = "Plume peut installer Tectonic pour vous, en un clic.";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const LATEX_HINT: &str = "Plume peut installer Tectonic pour vous, en un clic.";

#[derive(Serialize)]
pub struct Environment {
    pub tools: Vec<ToolStatus>,
    /// Every required prerequisite is present.
    pub ready: bool,
}

pub fn inspect() -> Environment {
    let specs = [
        Spec {
            key: "claude",
            label: "Claude Code",
            role: "Lit vos photos et rédige le LaTeX",
            binaries: &["claude"],
            version_arg: "--version",
            hint: "Plume peut l'installer pour vous. Il faudra ensuite vous connecter une fois, avec votre abonnement.",
            install_url: "https://code.claude.com/docs/en/setup",
            required: true,
        },
        Spec {
            key: "latex",
            label: "Moteur LaTeX",
            role: "Compile le .tex en PDF pour l'aperçu",
            binaries: &["tectonic", "pdflatex", "xelatex", "lualatex"],
            version_arg: "--version",
            hint: LATEX_HINT,
            install_url: LATEX_INSTALL_URL,
            required: true,
        },
    ];

    let tools: Vec<ToolStatus> = specs.iter().map(check).collect();
    let ready = tools.iter().all(|t| t.found || !t.required);
    Environment { tools, ready }
}
