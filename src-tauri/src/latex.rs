//! Compiles a rendered `.tex` into a PDF with whatever engine is installed.
//!
//! Tectonic is preferred when present: single binary, fetches its own packages.
//! Otherwise we fall back to the user's TeX distribution. Plume will eventually
//! ship Tectonic so non-technical users need no TeX install at all.

use crate::logbus;
use std::path::{Path, PathBuf};

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
        let mut command = crate::proc::quiet(&engine);
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

/// Lays several pages of a finished PDF on each sheet, for printing.
///
/// `per_sheet` is 2 or 4 — the only counts that make a regular grid of A4 on
/// A4. Two go side by side on a landscape sheet, each scaled to fit half of
/// it; four go in a grid on a portrait sheet. `repeat` puts the same page in
/// every cell — a sheet printed in several copies and cut — instead of the
/// pages in sequence.
///
/// Done in LaTeX rather than by rewriting the PDF: `pdfpages` is one line and
/// is already in every engine Plume runs, and the page count comes from the
/// engine's own primitive rather than from reading the PDF, whose objects are
/// compressed. Each engine has its own primitive, so all three are tried.
pub fn impose(dir: &Path, pdf_name: &str, per_sheet: u8, repeat: bool) -> Result<PathBuf, String> {
    // The paper is set here, by geometry, and pdfpages is left only the grid.
    // Its own `landscape` key stacked the two pages into one column under the
    // engine Plume ships — Tectonic carries an older pdfpages and driver pair
    // — while a system xelatex put them side by side. A landscape paper is
    // the one thing every engine agrees on.
    let (nup, paper) = match per_sheet {
        2 => ("2x1", "a4paper,landscape"),
        4 => ("2x2", "a4paper"),
        other => return Err(format!("{other} pages par feuille : seuls 2 et 4 forment une grille.")),
    };
    let stem = Path::new(pdf_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "document".into());

    // Every page, or every page `per_sheet` times, as a literal list: pdfpages
    // reads its `pages` key without expanding it, so the list is built into
    // the command by an `\edef` before the command runs.
    let cell = std::iter::repeat("\\the\\mc@i")
        .take(per_sheet as usize)
        .collect::<Vec<_>>()
        .join(",");
    let pages = if repeat {
        format!(
            "\\newcount\\mc@i \\mc@i=1
\\def\\mc@list{{}}
\\loop\\ifnum\\mc@i>\\mc@n\\relax\\else
  \\ifx\\mc@list\\empty\\edef\\mc@list{{{cell}}}\\else\\edef\\mc@list{{\\mc@list,{cell}}}\\fi
  \\advance\\mc@i 1
\\repeat
\\edef\\plumeimpose{{\\noexpand\\includepdf[nup={nup},pages={{\\mc@list}}]{{{pdf_name}}}}}"
        )
    } else {
        format!("\\edef\\plumeimpose{{\\noexpand\\includepdf[nup={nup},pages=-]{{{pdf_name}}}}}")
    };

    let wrapper = format!(
        "\\documentclass{{article}}
\\usepackage[{paper}]{{geometry}}
\\usepackage{{pdfpages}}
\\makeatletter
% The page count, from whichever engine is running. The space after XeTeX's
% closing quote matters: its file-name scanner otherwise swallows the brace
% that ends the \\edef, and everything down to the next brace becomes the
% definition.
\\ifdefined\\XeTeXpdfpagecount
  \\edef\\mc@n{{\\the\\XeTeXpdfpagecount \"{pdf_name}\" }}
\\else\\ifdefined\\pdfximage
  \\pdfximage{{{pdf_name}}}\\edef\\mc@n{{\\the\\pdflastximagepages}}
\\else
  \\saveimageresource{{{pdf_name}}}\\edef\\mc@n{{\\the\\lastsavedimageresourcepages}}
\\fi\\fi
{pages}
\\makeatother
\\begin{{document}}
\\plumeimpose
\\end{{document}}
"
    );

    let name = format!("{stem}-x{per_sheet}.tex");
    std::fs::write(dir.join(&name), wrapper).map_err(|e| format!("Écriture de {name} : {e}"))?;
    logbus::info(
        "latex",
        format!(
            "Imposition : {per_sheet} pages par feuille, {}",
            if repeat { "la même page répétée" } else { "pages à la suite" }
        ),
    );
    compile(dir, &name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three pages, imposed both ways. Skipped, and said so, on a machine with
    /// no engine — passing silently there would prove nothing.
    #[test]
    fn imposition_puts_pages_on_sheets() {
        let dir = std::env::temp_dir().join("plume-impose-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("source.tex"),
            "\\documentclass[a4paper]{article}\\usepackage{lmodern}\\begin{document}
\\centering\\Huge Page 1\\newpage Page 2\\newpage Page 3\\end{document}",
        )
        .unwrap();

        match compile(&dir, "source.tex") {
            Ok(_) => {}
            Err(error) if error.contains("moteur") => {
                eprintln!("no LaTeX engine on this machine, skipping: {error}");
                return;
            }
            Err(error) => panic!("source did not compile: {error}"),
        }

        // 3 pages, each twice, 2 per sheet → 3 sheets.
        let two = impose(&dir, "source.pdf", 2, true).expect("2 per sheet, repeated");
        assert!(two.ends_with("source-x2.pdf"));
        // 3 pages in sequence, 4 per sheet → 1 sheet.
        let four = impose(&dir, "source.pdf", 4, false).expect("4 per sheet, in sequence");
        assert!(four.ends_with("source-x4.pdf"));

        // pdfTeX and XeTeX both write this line; the counts are the point.
        let sheets = |stem: &str| {
            std::fs::read_to_string(dir.join(format!("{stem}.log")))
                .ok()
                .and_then(|log| {
                    log.lines()
                        .find(|l| l.starts_with("Output written on"))
                        .and_then(|l| l.split('(').nth(1))
                        .and_then(|l| l.split(' ').next())
                        .and_then(|n| n.parse::<u32>().ok())
                })
        };
        if let Some(n) = sheets("source-x2") {
            assert_eq!(n, 3, "three pages twice, two per sheet");
        }
        if let Some(n) = sheets("source-x4") {
            assert_eq!(n, 1, "three pages in sequence, four per sheet");
        }

        assert!(impose(&dir, "source.pdf", 3, false).is_err(), "3 is not a grid");
        // Left on disk on purpose: the PDFs are worth looking at.
    }
}
