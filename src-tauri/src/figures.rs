//! Renders a single TikZ block to an image for the review preview.
//!
//! A webview cannot draw TikZ, and a diagram shown as "see the PDF" is exactly
//! the block a teacher most needs to check. So each figure is compiled on its
//! own with the real engine, then rasterised.
//!
//! Results are cached next to the course, keyed by a hash of the TikZ source:
//! editing a figure produces a new file, leaving it alone costs nothing.

use crate::logbus;
use crate::templates::Template;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// Figure renders are serialised.
///
/// The preview mounts every diagram at once, so without this several `pdflatex`
/// runs share one directory and delete each other's intermediate files —
/// producing "I can't find file `fig-….aux`" on figures that compile perfectly
/// on their own. Two blocks holding the same diagram also share a cache key,
/// so they would race on the very same paths.
static RENDER_LOCK: Mutex<()> = Mutex::new(());

const CACHE_DIR: &str = "figures";

/// FNV-1a. Not cryptographic — it only has to change when the source changes,
/// and it saves a dependency.
fn hash(source: &str) -> String {
    let mut value: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in source.as_bytes() {
        value ^= *byte as u64;
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{value:016x}")
}

/// The colour definitions from the course's own preamble, so a figure drawn in
/// `mcDef` red looks the same here as in the PDF.
fn colour_definitions(root: &Path, template: &Template) -> String {
    crate::templates::render_preamble(root, template)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.trim_start().starts_with("\\definecolor"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn standalone_document(colours: &str, tikz: &str) -> String {
    format!(
        r#"\documentclass[border=4pt]{{standalone}}
\usepackage[T1]{{fontenc}}
\usepackage[utf8]{{inputenc}}
\usepackage{{lmodern}}
\usepackage{{amsmath,amssymb}}
\usepackage{{xcolor}}
\usepackage{{tikz}}
{colours}
\begin{{document}}
{tikz}
\end{{document}}
"#
    )
}

/// Pulls the first real error out of a LaTeX log, which is far more useful than
/// the process exit code.
fn compile_error(log: &Path, stdout: &str, stderr: &str) -> String {
    let from_log = fs::read_to_string(log).ok().and_then(|text| {
        text.lines()
            .find(|line| line.starts_with('!'))
            .map(|line| line.trim_start_matches('!').trim().to_string())
    });

    from_log
        .or_else(|| {
            stdout
                .lines()
                .find(|line| line.starts_with('!'))
                .map(|line| line.trim_start_matches('!').trim().to_string())
        })
        .or_else(|| stderr.lines().next().map(str::to_string))
        .filter(|detail| !detail.is_empty())
        .unwrap_or_else(|| "le moteur LaTeX n'a rien produit".to_string())
}

/// Converts the compiled PDF into something the webview can display.
///
/// SVG first — it stays crisp at any zoom. PNG is the fallback for machines
/// without poppler's vector converter.
fn rasterise(dir: &Path, stem: &str) -> Result<PathBuf, String> {
    let pdf = dir.join(format!("{stem}.pdf"));

    if let Some(tool) = crate::env_check::resolve_tool("pdftocairo") {
        let svg = dir.join(format!("{stem}.svg"));
        let done = Command::new(tool)
            .args(["-svg", "-f", "1", "-l", "1"])
            .arg(&pdf)
            .arg(&svg)
            .output()
            .map(|out| out.status.success() && svg.exists())
            .unwrap_or(false);
        if done {
            return Ok(svg);
        }
    }

    if let Some(tool) = crate::env_check::resolve_tool("pdftoppm") {
        let base = dir.join(stem);
        let done = Command::new(tool)
            .args(["-png", "-r", "220", "-f", "1", "-l", "1", "-singlefile"])
            .arg(&pdf)
            .arg(&base)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false);
        let png = dir.join(format!("{stem}.png"));
        if done && png.exists() {
            return Ok(png);
        }
    }

    Err("Aucun convertisseur d'image disponible (pdftocairo ou pdftoppm).".into())
}

/// Renders `tikz` and returns the image path, reusing the cache when possible.
pub fn render(
    document_dir: &Path,
    root: &Path,
    template: &Template,
    tikz: &str,
) -> Result<PathBuf, String> {
    let cache = document_dir.join(CACHE_DIR);
    let stem = format!("fig-{}", hash(tikz));

    let cached = |cache: &Path| {
        ["svg", "png"]
            .iter()
            .map(|extension| cache.join(format!("{stem}.{extension}")))
            .find(|path| path.is_file())
    };

    // Cheap path first, before taking the lock.
    if let Some(hit) = cached(&cache) {
        return Ok(hit);
    }

    let _guard = RENDER_LOCK.lock().map_err(|_| "Rendu des schémas indisponible.")?;

    // Another render may have produced it while we waited.
    if let Some(hit) = cached(&cache) {
        return Ok(hit);
    }

    fs::create_dir_all(&cache).map_err(|e| format!("Dossier des schémas : {e}"))?;

    // Each compilation gets its own directory, so nothing can collide with a
    // neighbour's intermediate files.
    let build = cache.join(format!(".build-{stem}"));
    let _ = fs::remove_dir_all(&build);
    fs::create_dir_all(&build).map_err(|e| format!("Dossier de compilation : {e}"))?;

    let source = standalone_document(&colour_definitions(root, template), tikz);
    fs::write(build.join(format!("{stem}.tex")), source)
        .map_err(|e| format!("Écriture du schéma : {e}"))?;

    let engine = crate::engine::installed()
        .map(|path| ("tectonic", path))
        .or_else(|| {
            ["tectonic", "pdflatex", "xelatex"]
                .iter()
                .find_map(|name| crate::env_check::resolve_tool(name).map(|path| (*name, path)))
        })
        .ok_or("Aucun moteur LaTeX détecté.")?;

    let mut command = Command::new(&engine.1);
    command.current_dir(&build);
    if engine.0 == "tectonic" {
        command.args(["-X", "compile", &format!("{stem}.tex")]);
    } else {
        command.args(["-interaction=nonstopmode", "-halt-on-error", &format!("{stem}.tex")]);
    }
    let output = command
        .output()
        .map_err(|e| format!("Lancement de {} impossible : {e}", engine.0))?;

    if !build.join(format!("{stem}.pdf")).exists() {
        let detail = compile_error(
            &build.join(format!("{stem}.log")),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
        );
        logbus::warn("latex", format!("Schéma non compilé : {detail}"));
        let _ = fs::remove_dir_all(&build);
        return Err(format!("Ce schéma ne compile pas : {detail}"));
    }

    let produced = rasterise(&build, &stem).inspect_err(|_| {
        let _ = fs::remove_dir_all(&build);
    })?;

    let extension = produced
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "svg".into());
    let final_path = cache.join(format!("{stem}.{extension}"));
    fs::rename(&produced, &final_path)
        .or_else(|_| fs::copy(&produced, &final_path).map(|_| ()))
        .map_err(|e| format!("Enregistrement du schéma : {e}"))?;

    let _ = fs::remove_dir_all(&build);

    logbus::detail("latex", "Schéma rendu", final_path.to_string_lossy().to_string());
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Reproduces the failure this module was built around: the preview mounts
    /// every diagram at once, and two blocks may hold the identical diagram.
    /// Before serialisation, concurrent runs deleted each other's intermediate
    /// files and reported "I can't find file `fig-….aux`".
    #[test]
    fn concurrent_renders_all_succeed() {
        if crate::env_check::resolve_tool("pdflatex").is_none()
            && crate::env_check::resolve_tool("tectonic").is_none()
        {
            eprintln!("no LaTeX engine on this machine, skipping");
            return;
        }

        let root = std::env::temp_dir().join(format!("plume-fig-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        crate::templates::seed(&root).unwrap();
        let template = crate::templates::load(&root, "charte-maths").expect("bundled template");

        let shared = r"\begin{tikzpicture}\draw[mcDef,->] (0,0) -- (2,1);\end{tikzpicture}";
        let diagrams = [
            shared,
            shared, // same source, same cache key: the collision that failed
            r"\begin{tikzpicture}\draw[mcProp,->] (0,0) -- (0,2);\end{tikzpicture}",
            r"\begin{tikzpicture}\coordinate (A) at (0,0);\node at (A) {$A$};\end{tikzpicture}",
            r"\begin{tikzpicture}\draw[mcTexte,dashed] (0,0) rectangle (2,1);\end{tikzpicture}",
        ];

        let results: Vec<_> = thread::scope(|scope| {
            let handles: Vec<_> = diagrams
                .iter()
                .map(|tikz| scope.spawn(|| render(&root, &root, &template, tikz)))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let failures: Vec<String> = results
            .iter()
            .filter_map(|r| r.as_ref().err().cloned())
            .collect();
        assert!(failures.is_empty(), "{failures:?}");

        for path in results.into_iter().flatten() {
            assert!(path.is_file(), "{path:?} was not produced");
        }

        // Nothing left behind but the cached images.
        let leftovers: Vec<String> = fs::read_dir(root.join(CACHE_DIR))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| !name.ends_with(".svg") && !name.ends_with(".png"))
            .collect();
        assert!(leftovers.is_empty(), "temporary files left: {leftovers:?}");

        let _ = fs::remove_dir_all(&root);
    }
}
