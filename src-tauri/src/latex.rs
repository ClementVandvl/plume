//! Compiles a rendered `.tex` into a PDF with whatever engine is installed.
//!
//! Tectonic is preferred when present: single binary, fetches its own packages.
//! Otherwise we fall back to the user's TeX distribution. Plume will eventually
//! ship Tectonic so non-technical users need no TeX install at all.

use crate::logbus;
use std::path::{Path, PathBuf};
use std::process::Command;

const ENGINES: &[&str] = &["tectonic", "pdflatex", "xelatex", "lualatex"];

/// Pulls the first real error out of a LaTeX log.
///
/// A raw log is unreadable for the target user; the `!` line is the one that
/// actually says what broke.
fn first_error(log: &Path) -> Option<String> {
    let text = std::fs::read_to_string(log).ok()?;
    text.lines()
        .find(|l| l.starts_with('!'))
        .map(|l| l.trim_start_matches('!').trim().to_string())
}

/// Compiles `<dir>/<tex_name>` and returns the produced PDF.
pub fn compile(dir: &Path, tex_name: &str) -> Result<PathBuf, String> {
    // The engine Plume installed itself comes first: it is the one the user was
    // told about, and a stale system TeX should not quietly take over.
    let (engine_name, engine) = crate::engine::installed()
        .map(|path| ("tectonic", path))
        .or_else(|| {
            ENGINES
                .iter()
                .find_map(|name| crate::env_check::resolve_tool(name).map(|path| (*name, path)))
        })
        .ok_or("Aucun moteur LaTeX détecté. Installez-le depuis les réglages.")?;

    let stem = Path::new(tex_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "document".into());

    // pdflatex needs two passes to settle references; tectonic handles it alone.
    let passes = if engine_name == "tectonic" { 1 } else { 2 };
    logbus::detail(
        "latex",
        format!("Compilation de {tex_name} avec {engine_name} ({passes} passe(s))"),
        engine.to_string_lossy().to_string(),
    );

    for pass in 0..passes {
        let mut command = Command::new(&engine);
        command.current_dir(dir);
        if engine_name == "tectonic" {
            command.args(["-X", "compile", "--keep-logs", tex_name]);
        } else {
            command.args(["-interaction=nonstopmode", tex_name]);
        }

        let output = command
            .output()
            .map_err(|e| format!("Lancement de {engine_name} impossible : {e}"))?;

        let pdf = dir.join(format!("{stem}.pdf"));
        let is_last = pass == passes - 1;

        if !output.status.success() && !pdf.exists() {
            let detail = first_error(&dir.join(format!("{stem}.log")))
                .unwrap_or_else(|| String::from_utf8_lossy(&output.stderr).trim().to_string());
            logbus::error("latex", format!("Compilation échouée : {detail}"));
            return Err(format!("La compilation a échoué : {detail}"));
        }

        if is_last {
            if !pdf.exists() {
                logbus::error("latex", "Aucun PDF produit.");
                return Err("La compilation n'a produit aucun PDF.".into());
            }
            logbus::detail("latex", "PDF produit", pdf.to_string_lossy().to_string());
            return Ok(pdf);
        }
    }

    Err("La compilation n'a produit aucun PDF.".into())
}
