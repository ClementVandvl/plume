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
const LAYOUT_COMMANDS: &[&str] = &[
    "\\begin{minipage}",
    "\\begin{multicols}",
    "\\rule{",
    "\\hfill",
    "\\newpage",
    "\\vspace",
];

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

fn render_block(template: &Template, block: &Block) -> String {
    let body = escape_nothing(block.latex.trim());
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
