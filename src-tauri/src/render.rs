//! Renders the IR into LaTeX, through the document's template.
//!
//! Deterministic and cheap: re-rendering costs nothing, so changing the
//! template or switching audience never calls the model again.

use crate::ir::{Block, Transcript};
use crate::templates::Template;
use std::path::Path;

/// Every audience, used when exporting the teacher's full version.
pub const AUDIENCE_ALL: &str = "all";

fn escape_nothing(latex: &str) -> &str {
    // The model already emits LaTeX. Escaping here would double-escape it.
    latex
}

/// Page-layout constructs the recogniser is told not to emit.
///
/// They are not rewritten — the block belongs to the teacher — but they are
/// reported, because a `minipage` pair that does not fit produces a stray rule
/// and a half-empty page in the PDF while looking fine in the preview.
/// Kept identical to `hasLayout` in the preview. The two disagreed once —
/// `\\vspace` warned in the console without flagging on screen, `tabular` the
/// other way round — so a passage could be reported in one place and silent in
/// the other.
const LAYOUT_COMMANDS: &[&str] = &[
    "\\begin{minipage}",
    "\\begin{multicols}",
    "\\begin{tabular}",
    "\\rule{",
    "\\hfill",
    "\\newpage",
    "\\vspace",
];

/// Environments where `&` is an alignment tab rather than a mistake.
const ALIGNING: &[&str] = &[
    "align", "align*", "aligned", "alignat", "alignat*", "gather", "gather*",
    "gathered", "split", "cases", "array", "matrix", "pmatrix", "bmatrix",
    "vmatrix", "Vmatrix", "smallmatrix", "tabular", "tabularx", "flalign",
    "flalign*", "multline", "multline*", "eqnarray", "eqnarray*",
];

/// Reads the environment name if `text` opens with `prefix`.
fn env_after(text: &str, prefix: &str) -> Option<&'static str> {
    let rest = text.strip_prefix(prefix)?;
    let name = rest.split('}').next()?;
    ALIGNING.iter().copied().find(|known| *known == name)
}

/// An `&` outside any environment that gives it a meaning.
///
/// The model reaches for alignment tabs on its own when a calculation runs over
/// several lines. Emitted bare they are a LaTeX error — "Misplaced alignment tab
/// character &" — and the preview showed them as literal text next to a mangled
/// fraction. The block is not rewritten, because it belongs to the teacher, but
/// they are told which one to look at.
pub(crate) fn has_stray_alignment(latex: &str) -> bool {
    let mut depth = 0usize;
    let mut escaped = false;

    for (index, character) in latex.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => {
                let rest = &latex[index..];
                if env_after(rest, "\\begin{").is_some() {
                    depth += 1;
                } else if env_after(rest, "\\end{").is_some() {
                    depth = depth.saturating_sub(1);
                } else {
                    // `\&` and every other escape: the next character is literal.
                    escaped = true;
                }
            }
            '&' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn warn_about_alignment(block: &Block) {
    if has_stray_alignment(&block.latex) {
        crate::logbus::warn(
            "render",
            format!(
                "Le bloc {} aligne sur un « & » hors d'un environnement d'alignement — \
                 le PDF ne compilera pas. Corrigez-le en relecture.",
                block.id
            ),
        );
    }
}

fn warn_about_layout(block: &Block) {
    let found: Vec<&str> = LAYOUT_COMMANDS
        .iter()
        .copied()
        .filter(|needle| block.latex.contains(needle))
        .collect();

    if !found.is_empty() {
        crate::logbus::warn(
            "render",
            format!(
                "Le bloc {} contient de la mise en page ({}) — le PDF peut déborder.",
                block.id,
                found.join(", ")
            ),
        );
    }
}

/// Wraps a body so it sits left, centred or right.
///
/// A wrapper, not a rewrite: the block's LaTeX is never read or altered, so
/// this cannot mangle a formula. `center` is redefined inside the group for the
/// same reason — a figure the recogniser centred would otherwise ignore the
/// choice, and neutralising it by hand would mean editing the content.
///
/// Display maths keeps LaTeX's own centring: `\[...\]` positions itself and no
/// grouping changes that. Only the charte can, by loading `fleqn`.
fn aligned(body: String, align: Option<&str>) -> String {
    let declaration = match align {
        Some("left") => "\\raggedright",
        Some("center") => "\\centering",
        Some("right") => "\\raggedleft",
        _ => return body,
    };

    // `center` centres whatever it holds; left and right have to undo it.
    let neutralise = if align == Some("center") {
        ""
    } else {
        "\\renewenvironment{center}{\\par}{\\par}%\n"
    };

    format!("\\begingroup{declaration}\n{neutralise}{body}\n\\par\\endgroup")
}

fn render_block(template: &Template, block: &Block) -> String {
    // The alignment wraps the body, inside whatever environment holds it: a
    // heading's own label must keep the place the charte gives it.
    let body = aligned(escape_nothing(block.latex.trim()).to_string(), block.align.as_deref());
    let body = body.as_str();
    let mapping = template.blocks.get(&block.kind);

    let Some(mapping) = mapping else {
        // Loud on purpose: a missing mapping produces a PDF that looks fine but
        // has lost its heading or its environment.
        crate::logbus::warn(
            "render",
            format!(
                "Aucune correspondance LaTeX pour un bloc « {} » — rendu en texte brut.",
                block.kind
            ),
        );
        return body.to_string();
    };

    match mapping.mode.as_str() {
        // Headings carry their text in `title`; `latex` may repeat it with the
        // handwritten numbering ("I - Vecteurs du plan"), which would collide
        // with the numbering the template already produces.
        "command" => {
            let heading = block
                .title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or(body);
            format!("\\{}{{{}}}", mapping.name, heading)
        }
        // Headings: the number read from the page, then the title. The
        // template decides how to show them; an empty number shows none.
        "numbered" => {
            let heading = block
                .title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or(body);
            let number = block.number.as_deref().map(str::trim).unwrap_or("");
            format!("\\{}{{{}}}{{{}}}", mapping.name, number, heading)
        }
        "environment" => {
            // The recogniser sometimes echoes the environment's own name as a
            // title, which renders as "Exemple (Exemple)". The template already
            // knows what each environment is called, so we can drop it.
            let own_name = template
                .keys
                .iter()
                .find(|k| k.key == format!("label.{}", block.kind))
                .map(|k| k.value.trim().to_lowercase());

            let title = block
                .title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .filter(|t| Some(t.to_lowercase()) != own_name)
                .map(|t| format!("[{t}]"))
                .unwrap_or_default();
            format!(
                "\\begin{{{name}}}{title}\n{body}\n\\end{{{name}}}",
                name = mapping.name
            )
        }
        "centered" => format!("\\begin{{center}}\n{body}\n\\end{{center}}"),
        _ => body.to_string(),
    }
}

fn keeps(block: &Block, audience: &str) -> bool {
    audience == AUDIENCE_ALL
        || block.audience.is_empty()
        || block.audience.iter().any(|a| a == audience)
}

/// Builds the complete `.tex` for one audience.
///
/// `audience` is `all`, `teacher` or `student`. Filtering happens here rather
/// than in LaTeX conditionals so that the student export contains no trace of
/// the teacher-only blocks — a `.tex` handed to a class should not hide answers
/// in a comment.
pub fn render_document(
    root: &Path,
    template: &Template,
    transcript: &Transcript,
    title: &str,
    audience: &str,
) -> std::io::Result<String> {
    let mut out = crate::templates::render_preamble(root, template)?;
    out.push_str("\n\\begin{document}\n\n");

    let mut wrote_chapter = false;
    for page in &transcript.pages {
        for block in page.blocks.iter().filter(|b| keeps(b, audience)) {
            if block.kind == "chapter" {
                wrote_chapter = true;
            }
            warn_about_layout(block);
            warn_about_alignment(block);
            out.push_str(&render_block(template, block));
            out.push_str("\n\n");
        }
    }

    // A course without a recognised chapter heading still deserves a title.
    if !wrote_chapter {
        let heading = format!("\\chapitre{{{title}}}\n\n");
        if let Some(at) = out.find("\\begin{document}\n\n") {
            let at = at + "\\begin{document}\n\n".len();
            out.insert_str(at, &heading);
        }
    }

    out.push_str("\\end{document}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact shape that reached the screen as a literal "&=".
    fn heading(kind: &str, title: &str, number: Option<&str>) -> Block {
        Block {
            id: "p01-b01".into(),
            kind: kind.into(),
            title: Some(title.into()),
            number: number.map(str::to_string),
            latex: String::new(),
            confidence: 1.0,
            doubt: None,
            audience: Vec::new(),
            align: None,
            note: None,
            reviewed: false,
        }
    }

    fn bundled() -> Template {
        serde_json::from_str(crate::templates::BUILTIN_MANIFEST).expect("valid manifest")
    }

    /// The number belongs to the page. A course photographed from the middle of
    /// a notebook opens on "Chapitre 3" and must stay chapter 3.
    #[test]
    fn a_heading_carries_the_number_read_on_the_page() {
        let template = bundled();
        assert_eq!(
            render_block(&template, &heading("chapter", "Vecteurs", Some("3"))),
            "\\chapitre{3}{Vecteurs}"
        );
        assert_eq!(
            render_block(&template, &heading("part", "Notion de vecteurs", Some("II"))),
            "\\partie{II}{Notion de vecteurs}"
        );
    }

    /// And an absent number is passed as absent, never replaced by a guess.
    #[test]
    fn an_unnumbered_heading_gets_no_invented_number() {
        let template = bundled();
        assert_eq!(
            render_block(&template, &heading("chapter", "Vecteurs", None)),
            "\\chapitre{}{Vecteurs}"
        );
        assert_eq!(
            render_block(&template, &heading("subpart", "Définition", Some("  "))),
            "\\souspartie{}{Définition}",
            "blank is the same as absent"
        );
    }

    /// The twin of `hasLayout` in the preview: the same list, checked here so
    /// the console and the screen cannot drift apart again.
    #[test]
    fn every_layout_construct_is_reported() {
        for latex in [
            "\\begin{minipage}[t]{0.48\\textwidth}a\\end{minipage}",
            "\\begin{multicols}{2}a\\end{multicols}",
            "\\begin{tabular}{cc}a & b\\end{tabular}",
            "\\hfill\\rule{0.4pt}{6cm}\\hfill",
            "avant\\newpage après",
            "\\vspace{1cm}",
        ] {
            assert!(
                LAYOUT_COMMANDS.iter().any(|needle| latex.contains(needle)),
                "not reported: {latex}"
            );
        }

        for latex in [
            "Soient $\\vec{u}$ et $\\vec{v}$ deux vecteurs.",
            "\\begin{center}\\begin{tikzpicture}\\end{tikzpicture}\\end{center}",
        ] {
            assert!(
                !LAYOUT_COMMANDS.iter().any(|needle| latex.contains(needle)),
                "wrongly reported: {latex}"
            );
        }
    }

    #[test]
    fn alignment_wraps_without_touching_the_content() {
        let formula = "$\\dfrac{a}{c} + \\dfrac{b}{c}$";

        for (choice, declaration) in [
            ("left", "\\raggedright"),
            ("center", "\\centering"),
            ("right", "\\raggedleft"),
        ] {
            let out = aligned(formula.to_string(), Some(choice));
            assert!(out.contains(declaration), "{choice} must use {declaration}");
            assert!(out.contains(formula), "the content is passed through untouched");
            assert!(out.starts_with("\\begingroup"), "the change must stay local");
            assert!(out.ends_with("\\par\\endgroup"));
        }

        // Left and right have to undo a `center` the recogniser emitted; the
        // centred choice has nothing to undo.
        assert!(aligned(formula.into(), Some("left")).contains("\\renewenvironment{center}"));
        assert!(aligned(formula.into(), Some("right")).contains("\\renewenvironment{center}"));
        assert!(!aligned(formula.into(), Some("center")).contains("\\renewenvironment"));
    }

    #[test]
    fn no_alignment_leaves_the_body_exactly_as_it_was() {
        let body = "\\begin{center}\\begin{tikzpicture}\\end{tikzpicture}\\end{center}";
        assert_eq!(aligned(body.to_string(), None), body);
        assert_eq!(aligned(body.to_string(), Some("")), body);
        assert_eq!(aligned(body.to_string(), Some("justified")), body);
    }

    #[test]
    fn a_bare_alignment_tab_is_caught() {
        assert!(has_stray_alignment(
            r"A = \frac{3}{5} + \frac{7}{15} &= \frac{9}{15} \\ &= \frac{16}{15}"
        ));
        assert!(has_stray_alignment(r"$A &= 1 \\ &= 2$"), "dollars alone do not help");
    }

    #[test]
    fn a_tab_inside_its_environment_is_fine() {
        assert!(!has_stray_alignment(r"$\begin{aligned}[t] A &= 1 \\ &= 2 \end{aligned}$"));
        assert!(!has_stray_alignment(r"\begin{align*} A &= 1 \\ &= 2 \end{align*}"));
        assert!(!has_stray_alignment(r"\begin{cases} a & x > 0 \\ b & x < 0 \end{cases}"));
        // Nested: the inner environment must not close the outer one early.
        assert!(!has_stray_alignment(
            r"\begin{align*} \begin{cases} a & b \end{cases} &= c \end{align*}"
        ));
    }

    #[test]
    fn an_escaped_ampersand_is_not_an_alignment_tab() {
        assert!(!has_stray_alignment(r"Pierre \& Marie Curie"));
        assert!(!has_stray_alignment("Rien à signaler ici."));
    }
}
