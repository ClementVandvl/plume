//! Renders a snippet of a document to an image for the review preview.
//!
//! Two kinds of snippet, one pipeline. A *figure* is a TikZ picture, which a
//! webview cannot draw. A *passage* is a whole block that lays itself out —
//! a table, columns, a rule — which the HTML preview can only stack in one
//! column; converting every way LaTeX has of making a table is a chase with
//! no end, so the passage is typeset by the engine, with the charte's own
//! preamble, and shown as it will print. Both are compiled on their own with
//! the real engine, then rasterised.
//!
//! Results are cached next to the document, keyed by a hash of what was
//! compiled: editing a snippet produces a new file, leaving it alone costs
//! nothing, and a passage's key includes the preamble so a charte edit
//! re-renders it.

use crate::logbus;
use crate::templates::Template;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Renders are serialised.
///
/// The preview mounts every snippet at once, so without this several
/// `pdflatex` runs share one directory and delete each other's intermediate
/// files — producing "I can't find file `fig-….aux`" on figures that compile
/// perfectly on their own. Two blocks holding the same diagram also share a
/// cache key, so they would race on the very same paths.
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

/// What the preview wants drawn.
pub enum Snippet<'a> {
    /// A `tikzpicture`, on its own.
    Figure(&'a str),
    /// A block's whole LaTeX body, as the charte would set it.
    Passage(&'a str),
}

impl Snippet<'_> {
    /// The prefix of the cached file, so the two kinds never share a key.
    fn prefix(&self) -> &'static str {
        match self {
            Snippet::Figure(_) => "fig",
            Snippet::Passage(_) => "pas",
        }
    }

    /// The word the messages use.
    fn noun(&self) -> &'static str {
        match self {
            Snippet::Figure(_) => "schéma",
            Snippet::Passage(_) => "passage",
        }
    }

    /// The complete `.tex` that draws the snippet.
    ///
    /// A figure needs only the charte's colours, and `standalone` crops it to
    /// the drawing. A passage needs everything the charte defines — its
    /// packages, macros, fonts, `\leqslant` — so its preamble is taken whole,
    /// and the `preview` package crops the page to a minipage of the charte's
    /// own text width: line breaks fall where the PDF will put them, and a
    /// table wider than the page overflows here as it overflows there.
    fn document(&self, preamble: &str) -> String {
        match self {
            Snippet::Figure(tikz) => {
                let colours = preamble
                    .lines()
                    .filter(|line| line.trim_start().starts_with("\\definecolor"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "\\documentclass[border=4pt]{{standalone}}\n\
                     \\usepackage[T1]{{fontenc}}\n\
                     \\usepackage[utf8]{{inputenc}}\n\
                     \\usepackage{{lmodern}}\n\
                     \\usepackage{{amsmath,amssymb}}\n\
                     \\usepackage{{xcolor}}\n\
                     \\usepackage{{tikz}}\n\
                     {colours}\n\
                     \\begin{{document}}\n\
                     {tikz}\n\
                     \\end{{document}}\n"
                )
            }
            Snippet::Passage(latex) => format!(
                "{}\n\
                 \\usepackage[active,tightpage]{{preview}}\n\
                 \\setlength{{\\PreviewBorder}}{{4pt}}\n\
                 \\begin{{document}}\n\
                 \\begin{{preview}}\n\
                 \\begin{{minipage}}{{\\textwidth}}\n\
                 {latex}\n\
                 \\end{{minipage}}\n\
                 \\end{{preview}}\n\
                 \\end{{document}}\n",
                preamble.trim_end()
            ),
        }
    }

    /// What the cache key is made of: the source, plus everything else that
    /// changes the image.
    fn key(&self, preamble: &str) -> String {
        match self {
            Snippet::Figure(tikz) => hash(tikz),
            Snippet::Passage(latex) => hash(&format!("{preamble}\n{latex}")),
        }
    }
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
        let done = crate::proc::quiet(tool)
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
        let done = crate::proc::quiet(tool)
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

/// Renders the snippet and returns the image path, reusing the cache when
/// possible.
pub fn render(
    document_dir: &Path,
    root: &Path,
    template: &Template,
    snippet: Snippet<'_>,
) -> Result<PathBuf, String> {
    let preamble = crate::templates::render_preamble(root, template)
        .map_err(|e| format!("Préambule de la charte illisible : {e}"))?;
    let cache = document_dir.join(CACHE_DIR);
    let stem = format!("{}-{}", snippet.prefix(), snippet.key(&preamble));
    let noun = snippet.noun();

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

    let _guard = RENDER_LOCK.lock().map_err(|_| "Rendu indisponible.")?;

    // Another render may have produced it while we waited.
    if let Some(hit) = cached(&cache) {
        return Ok(hit);
    }

    fs::create_dir_all(&cache).map_err(|e| format!("Dossier des rendus : {e}"))?;

    // Each compilation gets its own directory, so nothing can collide with a
    // neighbour's intermediate files.
    let build = cache.join(format!(".build-{stem}"));
    let _ = fs::remove_dir_all(&build);
    fs::create_dir_all(&build).map_err(|e| format!("Dossier de compilation : {e}"))?;

    fs::write(build.join(format!("{stem}.tex")), snippet.document(&preamble))
        .map_err(|e| format!("Écriture du {noun} : {e}"))?;

    let engine = crate::engine::installed()
        .map(|path| ("tectonic", path))
        .or_else(|| {
            ["tectonic", "pdflatex", "xelatex"]
                .iter()
                .find_map(|name| crate::env_check::resolve_tool(name).map(|path| (*name, path)))
        })
        .ok_or("Aucun moteur LaTeX détecté.")?;

    let mut command = crate::proc::quiet(&engine.1);
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
        logbus::warn("latex", format!("{} non compilé : {detail}", capitalise(noun)));
        let _ = fs::remove_dir_all(&build);
        return Err(format!("Ce {noun} ne compile pas : {detail}"));
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
        .map_err(|e| format!("Enregistrement du {noun} : {e}"))?;

    let _ = fs::remove_dir_all(&build);

    logbus::detail(
        "latex",
        format!("{} rendu", capitalise(noun)),
        final_path.to_string_lossy().to_string(),
    );
    Ok(final_path)
}

fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn has_engine() -> bool {
        crate::env_check::resolve_tool("pdflatex").is_some()
            || crate::env_check::resolve_tool("tectonic").is_some()
            || crate::engine::installed().is_some()
    }

    fn scratch(name: &str) -> (PathBuf, Template) {
        let root = std::env::temp_dir().join(format!("plume-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        crate::templates::seed(&root).unwrap();
        let template = crate::templates::load(&root, "charte-maths").expect("bundled template");
        (root, template)
    }

    /// A passage is set inside the charte, not beside it: its preamble comes
    /// first and whole, the crop is the `preview` package's, and the width
    /// is the charte's text width. A figure keeps the lighter standalone.
    #[test]
    fn a_passage_is_typeset_with_the_charte_s_own_preamble() {
        let preamble = "\\documentclass[11pt,a4paper]{article}\n\\usepackage{tikz}\n\\definecolor{mcDef}{HTML}{A93226}\n";
        let passage = Snippet::Passage("\\begin{tabular}{cc}a & b\\end{tabular}").document(preamble);
        assert!(passage.starts_with("\\documentclass[11pt,a4paper]{article}\n\\usepackage{tikz}"));
        assert!(passage.contains("\\usepackage[active,tightpage]{preview}"));
        assert!(passage.contains("\\begin{minipage}{\\textwidth}\n\\begin{tabular}{cc}a & b\\end{tabular}\n\\end{minipage}"));

        let figure = Snippet::Figure("\\begin{tikzpicture}\\end{tikzpicture}").document(preamble);
        assert!(figure.starts_with("\\documentclass[border=4pt]{standalone}"));
        assert!(figure.contains("\\definecolor{mcDef}{HTML}{A93226}"));
        assert!(!figure.contains("a4paper"), "a figure does not carry the page");
    }

    /// The same table, before and after the charte changes: a different
    /// image. The same diagram: the same one, the charte having no say.
    #[test]
    fn a_passage_s_cache_key_follows_the_charte_but_a_figure_s_does_not() {
        let table = Snippet::Passage("\\begin{tabular}{c}x\\end{tabular}");
        assert_ne!(table.key("\\definecolor{mcDef}{HTML}{A93226}"), table.key("\\definecolor{mcDef}{HTML}{000000}"));
        let figure = Snippet::Figure("\\begin{tikzpicture}\\end{tikzpicture}");
        assert_eq!(figure.key("\\definecolor{mcDef}{HTML}{A93226}"), figure.key("\\definecolor{mcDef}{HTML}{000000}"));
    }

    /// The table that started this: a `tabular` with a diagram in each row,
    /// which the HTML preview can only show as prose with stray `&`. Rendered
    /// by the engine it comes back as one image, text width wide.
    #[test]
    fn a_table_with_diagrams_in_its_cells_renders_as_one_image() {
        if !has_engine() {
            eprintln!("no LaTeX engine on this machine, skipping");
            return;
        }
        let (root, template) = scratch("passage-test");
        let latex = "\\begin{center}\\begin{tabular}{|c|c|}\\hline\n\
                     $x \\in [a\\,;b]$ & \\begin{tikzpicture}[baseline=-0.5ex,x=0.5cm,y=0.5cm]\\draw[mcTexte,->] (0,0) -- (4,0);\\draw[mcDef,line width=1.2pt] (1,0) -- (3,0);\\end{tikzpicture} \\\\ \\hline\n\
                     \\end{tabular}\\end{center}";
        let path = render(&root, &root, &template, Snippet::Passage(latex)).expect("rendered");
        assert!(path.is_file());
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("pas-"));
        // Once more, from the cache this time: the same file, no compile.
        assert_eq!(render(&root, &root, &template, Snippet::Passage(latex)).unwrap(), path);
        let _ = fs::remove_dir_all(&root);
    }

    /// Reproduces the failure this module was built around: the preview mounts
    /// every diagram at once, and two blocks may hold the identical diagram.
    /// Before serialisation, concurrent runs deleted each other's intermediate
    /// files and reported "I can't find file `fig-….aux`".
    #[test]
    fn concurrent_renders_all_succeed() {
        if !has_engine() {
            eprintln!("no LaTeX engine on this machine, skipping");
            return;
        }
        let (root, template) = scratch("fig-test");

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
                .map(|tikz| scope.spawn(|| render(&root, &root, &template, Snippet::Figure(tikz))))
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
