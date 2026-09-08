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

/// Pages of a compiled document, from its log: pdfTeX and XeTeX both write
/// the line, Tectonic included.
fn page_count(dir: &Path, stem: &str) -> Option<u32> {
    let log = std::fs::read_to_string(dir.join(format!("{stem}.log"))).ok()?;
    log.lines()
        .find(|l| l.starts_with("Output written on"))
        .and_then(|l| l.split('(').nth(1))
        .and_then(|l| l.split(' ').next())
        .and_then(|n| n.parse().ok())
}

/// The rows a sheet is cut into, smallest cell first: two columns always, and
/// as many rows as the document still fits in.
const FILL_ROWS: [u32; 4] = [4, 3, 2, 1];

/// The smallest type a slip may be set in: what "4 pages" already prints an
/// 11 pt document at, near enough, and read at a desk since.
const FILL_MIN_SIZE: u32 = 6;

/// The type size the document's class was given, in points: `11pt` in
/// `\documentclass[11pt,a4paper]{article}`. LaTeX's own default otherwise.
fn base_size(source: &str) -> u32 {
    source
        .lines()
        .find(|l| l.trim_start().starts_with("\\documentclass["))
        .and_then(|l| l.split('[').nth(1))
        .and_then(|l| l.split(']').next())
        .and_then(|options| {
            options
                .split(',')
                .map(str::trim)
                .find_map(|o| o.strip_suffix("pt").and_then(|n| n.parse().ok()))
        })
        .unwrap_or(10)
}

/// Where the document's body begins: the byte offset of the line that holds
/// the real `\begin{document}`. Not the first mention of it — the maths
/// charte's preamble opens with a comment saying what to copy "entre
/// \documentclass{...} et \begin{document}", and a cell injected there put
/// `\begin{document}` on line 7 and every `\usepackage` after it.
fn body_starts_at(source: &str) -> Option<usize> {
    let mut at = 0;
    for line in source.split_inclusive('\n') {
        let code = match line.find('%') {
            Some(i) if i == 0 || line.as_bytes()[i - 1] != b'\\' => &line[..i],
            _ => line,
        };
        if code.contains("\\begin{document}") {
            return Some(at);
        }
        at += line.len();
    }
    None
}

/// Whether smaller type could bring a document of `pages` pages at `base`
/// down to one. Text area scales with the square of the size; the banners
/// a charte sets in fixed sizes do not shrink, so this is optimistic — a
/// "no" is certain and saves the compiles, a "yes" is worth them.
fn shrinking_could_fit(pages: u32, base: u32) -> bool {
    pages as f64 * (FILL_MIN_SIZE as f64 / base as f64).powi(2) <= 1.0
}

/// The largest size below `base` at which `measure` reports a single page,
/// by bisection: 10 pt down to 6 pt is three compiles, not five. `None`
/// from `measure` — a compile that failed — ends the search with what was
/// found. Pagination is not perfectly monotone in the size, but near enough
/// that the bisection lands on a size that fits, which is what matters.
fn largest_fitting(base: u32, mut measure: impl FnMut(u32) -> Option<u32>) -> Option<u32> {
    let mut best = None;
    let (mut lo, mut hi) = (FILL_MIN_SIZE, base.saturating_sub(1));
    while lo <= hi {
        let mid = (lo + hi + 1) / 2;
        match measure(mid) {
            Some(1) => {
                best = Some(mid);
                lo = mid + 1;
            }
            Some(_) => hi = mid - 1,
            None => break,
        }
    }
    best
}

/// Writes the document on a paper the size of one cell, at `size`, compiles
/// it and counts its pages. Files are named after the cell and the size
/// (`fiche-cell3-8pt`), because the bisection's last compile is not always
/// its winner and the imposition must pick up the right PDF.
fn measure_cell(
    dir: &Path,
    source: &str,
    body: usize,
    stem: &str,
    rows: u32,
    base: u32,
    size: u32,
) -> Result<u32, String> {
    let height = 297.0 / rows as f64;
    // A page number on every slip would be absurd: it goes from the footer
    // when the charte uses fancyhdr, the running header staying, and the
    // whole plain style goes otherwise.
    let mut cell = format!(
        "\\geometry{{paperwidth=105mm,paperheight={height:.2}mm,margin=5mm,\
         headheight=12pt,headsep=2mm,footskip=4mm}}\n\
         \\ifdefined\\fancyfoot\\fancyfoot{{}}\\else\\pagestyle{{empty}}\\fi\n"
    );
    if size != base {
        cell.push_str(&format!("\\usepackage[fontsize={size}pt]{{scrextend}}\n"));
    }
    let tex = format!("{}{cell}{}", &source[..body], &source[body..]);
    let cell_stem = cell_stem(stem, rows, size);
    std::fs::write(dir.join(format!("{cell_stem}.tex")), tex)
        .map_err(|e| format!("Écriture de {cell_stem}.tex : {e}"))?;

    let pdf = compile(dir, &format!("{cell_stem}.tex")).map_err(|failure| {
        logbus::warn("latex", format!("Case de {rows} rangée(s) à {size} pt : compilation impossible"));
        failure
    })?;
    let pages = page_count(dir, &cell_stem)
        .ok_or_else(|| format!("Nombre de pages illisible dans {cell_stem}.log"))?;
    logbus::detail(
        "latex",
        format!("Case de {rows} rangée(s) à {size} pt : {pages} page(s)"),
        pdf.to_string_lossy().to_string(),
    );
    Ok(pages)
}

fn cell_stem(stem: &str, rows: u32, size: u32) -> String {
    format!("{stem}-cell{rows}-{size}pt")
}

/// Tiles as many copies of the document on an A4 sheet as fit.
///
/// Imposing pages shrinks a page that was mostly empty into a cell that is
/// then mostly empty too: one short exercise, four to a sheet, was three
/// quarters blank in every cell. So the document is *recomposed* instead:
/// compiled on a paper the size of one cell — two columns, `rows` rows,
/// 105 mm by 297/rows — and the smallest cell in which it still fits on a
/// single page wins. Each cell is tried at the charte's own type size first,
/// then at the largest smaller size that fits, never below `FILL_MIN_SIZE`:
/// the teacher asked for the fullest sheet, and eight slips at 7 pt beat six
/// at 11 pt. The floor is what "4 pages" already prints an 11 pt document
/// at, so nothing here is smaller than what was being read before; a charte
/// set at 9 pt comes down to 6 pt too, not to half of itself. A document
/// that does not fit even a half page at the floor is refused, and the fixed
/// counts remain.
///
/// The cell is set through `geometry`, and the size through `scrextend`,
/// injected before `\begin{document}` so they come after the charte's own
/// preamble; a charte that does not load geometry cannot be recomposed and
/// says so.
pub fn fill(dir: &Path, tex_name: &str) -> Result<PathBuf, String> {
    let source = std::fs::read_to_string(dir.join(tex_name))
        .map_err(|e| format!("Lecture de {tex_name} : {e}"))?;
    let stem = Path::new(tex_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "document".into());

    if !source.contains("{geometry}") {
        return Err("Cette charte ne charge pas geometry : Plume ne peut pas recomposer \
                    la fiche dans une case."
            .into());
    }

    let body = body_starts_at(&source)
        .ok_or("Le .tex n'a pas de \\begin{document} : rien à recomposer.")?;
    let base = base_size(&source);
    // A cell that compiled and was counted, whatever the count: without one,
    // the failure is the compile's, and its message is the one to show.
    let mut measured = false;
    let mut last_failure = String::new();
    for rows in FILL_ROWS {
        let mut trial = |size: u32| match measure_cell(dir, &source, body, &stem, rows, base, size) {
            Ok(pages) => {
                measured = true;
                Some(pages)
            }
            Err(failure) => {
                last_failure = failure;
                None
            }
        };

        let Some(pages) = trial(base) else { continue };
        let winner = if pages == 1 {
            Some(base)
        } else if shrinking_could_fit(pages, base) {
            largest_fitting(base, &mut trial)
        } else {
            logbus::detail(
                "latex",
                format!("Case de {rows} rangée(s) : {pages} pages, même {FILL_MIN_SIZE} pt n'y tiendrait pas"),
                String::new(),
            );
            None
        };
        let Some(size) = winner else { continue };

        // One page that fits its cell exactly: pdfpages lays 2×rows of them
        // on the sheet at scale one. No frame: the teacher cuts by eye, and
        // a rule down every slip reads as part of the sheet.
        let copies = 2 * rows as usize;
        let list = std::iter::repeat("1").take(copies).collect::<Vec<_>>().join(",");
        let cell_pdf = format!("{}.pdf", cell_stem(&stem, rows, size));
        let wrapper = format!(
            "\\documentclass{{article}}\n\
             \\usepackage[a4paper]{{geometry}}\n\
             \\usepackage{{pdfpages}}\n\
             \\begin{{document}}\n\
             \\includepdf[nup=2x{rows},pages={{{list}}}]{{{cell_pdf}}}\n\
             \\end{{document}}\n"
        );
        let name = format!("{stem}-max.tex");
        std::fs::write(dir.join(&name), wrapper).map_err(|e| format!("Écriture de {name} : {e}"))?;
        logbus::info(
            "latex",
            format!("Maximiser : {copies} exemplaires par feuille, en cases de {rows} rangée(s) à {size} pt"),
        );
        return compile(dir, &name);
    }

    if !measured {
        return Err(format!("La recomposition n'a pas compilé : {last_failure}"));
    }
    Err(format!(
        "La fiche ne tient pas dans une demi-page, même à {FILL_MIN_SIZE} pt : gardez 1 ou 2 pages \
         par feuille."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_largest_size_that_fits_is_found_in_three_compiles_not_five() {
        let mut tried = Vec::new();
        let size = largest_fitting(11, |size| {
            tried.push(size);
            Some(if size <= 8 { 1 } else { 2 })
        });
        assert_eq!(size, Some(8));
        assert!(tried.len() <= 3, "bisection over 10..6, not a walk: {tried:?}");
        assert!(tried.iter().all(|s| (6..=10).contains(s)), "never the base, never below the floor");

        assert_eq!(largest_fitting(11, |_| Some(2)), None, "nothing fits, not even the floor");
        assert_eq!(largest_fitting(6, |_| Some(1)), None, "a charte at the floor has nowhere to go");
        assert_eq!(largest_fitting(11, |_| None), None, "a failed compile ends the search");
    }

    /// Three pages at 11 pt can become one at 6 pt (area ×0.3); four cannot.
    /// A 9 pt charte has less room: two pages, yes; three, no.
    #[test]
    fn the_floor_is_only_tried_when_the_arithmetic_allows_it() {
        assert!(shrinking_could_fit(3, 11));
        assert!(!shrinking_could_fit(4, 11));
        assert!(shrinking_could_fit(2, 9));
        assert!(!shrinking_could_fit(3, 9));
    }

    /// The regression: the maths charte's preamble names `\begin{document}`
    /// in a comment on its fifth line, and the cell went there.
    #[test]
    fn the_cell_goes_before_the_real_begin_document_not_a_comment_naming_it() {
        let source = "\\documentclass{article}\n\
                      %  A copier tel quel entre \\documentclass{...} et \\begin{document}\n\
                      \\usepackage{geometry}\n\
                      \\begin{document}\nx\n\\end{document}\n";
        let at = body_starts_at(source).unwrap();
        assert!(source[at..].starts_with("\\begin{document}\nx"));
        assert_eq!(body_starts_at("\\documentclass{article}\n"), None);
        assert_eq!(
            body_starts_at("100\\% sure \\begin{document}\n"),
            Some(0),
            "an escaped percent is not a comment"
        );
    }

    #[test]
    fn the_charte_size_is_read_off_the_class_line() {
        assert_eq!(base_size("\\documentclass[11pt,a4paper]{article}\n"), 11);
        assert_eq!(base_size("\\documentclass[a4paper, 12pt]{article}\n"), 12);
        assert_eq!(base_size("\\documentclass{article}\n"), 10, "LaTeX's own default");
    }

    /// A short exercise recomposed into the smallest cell it fits: eight to
    /// a sheet, not four with three quarters blank. Skipped, and said so,
    /// without an engine.
    #[test]
    fn a_short_document_fills_the_sheet_unshrunk() {
        let dir = std::env::temp_dir().join("plume-fill-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("sheet.tex"),
            "\\documentclass[11pt,a4paper]{article}\\usepackage[a4paper,margin=2cm]{geometry}\n\
             \\begin{document}\n\\noindent\\textbf{Exercice 1} --- D\\'evelopper et r\\'eduire.\n\n\
             a) $4(3x+2)$ \\quad b) $-3(2x-5)$ \\quad c) $2x(x+7)$\n\\end{document}\n",
        )
        .unwrap();

        match fill(&dir, "sheet.tex") {
            Ok(pdf) => {
                assert!(pdf.ends_with("sheet-max.pdf"));
                assert!(dir.join("sheet-cell4-11pt.pdf").is_file(), "the smallest cell was tried first");
                assert_eq!(page_count(&dir, "sheet-cell4-11pt"), Some(1), "and the exercise fit it");
                assert_eq!(page_count(&dir, "sheet-max"), Some(1), "one sheet, eight copies");
            }
            Err(error) if error.contains("moteur") => {
                eprintln!("no LaTeX engine on this machine, skipping: {error}");
            }
            Err(error) => panic!("fill failed: {error}"),
        }
        // Left on disk on purpose: the sheet is worth looking at.
    }

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
