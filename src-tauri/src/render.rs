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

/// Page layout the block does for itself, which the charte will not reproduce.
///
/// Shared with the import check rather than duplicated there: this list and the
/// preview's `hasLayout` disagreed once already, and a passage reported in one
/// place and silent in another is how the disagreement stayed hidden.
pub(crate) fn has_layout(latex: &str) -> bool {
    LAYOUT_COMMANDS.iter().any(|needle| latex.contains(needle))
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

fn render_block(template: &Template, block: &Block, blanks: bool) -> String {
    // The alignment wraps the body, inside whatever environment holds it: a
    // heading's own label must keep the place the charte gives it.
    let body = aligned(
        apply_gaps(escape_nothing(block.latex.trim()), blanks),
        block.align.as_deref(),
    );
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

/// The document's title, written the way this charte writes a chapter.
fn fallback_heading(template: &Template, title: &str) -> String {
    match template.blocks.get("chapter") {
        Some(mapping) if mapping.mode == "numbered" => {
            format!("\\{}{{}}{{{title}}}", mapping.name)
        }
        Some(mapping) if mapping.mode == "command" => format!("\\{}{{{title}}}", mapping.name),
        _ => format!("\\section*{{{title}}}"),
    }
}

fn keeps(block: &Block, audience: &str) -> bool {
    audience == AUDIENCE_ALL
        || block.audience.is_empty()
        || block.audience.iter().any(|a| a == audience)
}

/// The mark a teacher leaves on words that must disappear from an adapted
/// copy: `\trou{les mots}`.
///
/// Held in the block's own LaTeX rather than in a field beside it. A field
/// would have to name a span of text by position, and every edit to the
/// passage moves those; inside the body, the mark travels with the words it
/// holds — through a correction, a split, a renumbering.
pub const GAP: &str = "\\trou";

/// How much wider than the printed word its hole is.
///
/// Handwriting is bigger than 11 pt type, and a hole the exact width of the
/// word it replaces is one a pupil cannot write in — which is the whole point
/// of the hole. Half as wide again is about what a secondary pupil's hand
/// needs. The price is that the adapted copy no longer breaks its lines in
/// the same places as the ordinary one; room to write was worth more.
const GAP_WIDTH: &str = "1.5";

/// Resolves every `\trou{…}` in a block's body.
///
/// `blank` produces the adapted copy: each marked word becomes a ruled space
/// half as wide again as the word it hides, for the pupil to write in. Word
/// by word rather than one rule over the whole run, so a long marking still
/// breaks across lines, and the count of holes tells the pupil how many words
/// are missing — the kind of support a PAP exists to give.
///
/// Without `blank` the words come back and the ordinary PDF holds no trace of
/// the marking: one document, two copies, nothing to keep in step by hand.
pub fn apply_gaps(latex: &str, blank: bool) -> String {
    let mut out = String::with_capacity(latex.len());
    let mut rest = latex;

    while let Some(at) = rest.find(GAP) {
        let after = &rest[at + GAP.len()..];
        // `\trouble` opens no group: the command name ends at the brace.
        let Some(inner) = group_at(after) else {
            out.push_str(&rest[..at + GAP.len()]);
            rest = after;
            continue;
        };
        out.push_str(&rest[..at]);
        out.push_str(&if blank { blanked(inner) } else { inner.to_string() });
        rest = &after[inner.len() + 2..];
    }

    out.push_str(rest);
    out
}

/// The contents of the balanced group `text` opens with, if it opens with one.
fn group_at(text: &str) -> Option<&str> {
    if !text.starts_with('{') {
        return None;
    }
    let mut depth = 0i32;
    let mut escaped = false;
    for (at, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[1..at]);
                }
            }
            _ => {}
        }
    }
    None
}

fn blanked(content: &str) -> String {
    words_of(content)
        .into_iter()
        .map(|word| {
            // `\width` inside a `\makebox` width is the natural width of what
            // it holds, so the rule grows with the word rather than by a fixed
            // amount: a long word gets a long hole.
            format!(
                "\\underline{{\\vphantom{{Ag}}\\makebox[{GAP_WIDTH}\\width]{{\\hphantom{{{word}}}}}}}"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The words of a marked run, splitting on spaces that separate words rather
/// than on every space: one inside `$a + b$` or inside a command's argument
/// would cut the LaTeX in half and the copy would not compile.
fn words_of(content: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let (mut depth, mut maths, mut escaped) = (0i32, false, false);
    let mut start: Option<usize> = None;
    let open = |start: &mut Option<usize>, at: usize| {
        if start.is_none() {
            *start = Some(at);
        }
    };

    for (at, ch) in content.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                open(&mut start, at);
                escaped = true;
            }
            '{' => {
                open(&mut start, at);
                depth += 1;
            }
            '}' => {
                open(&mut start, at);
                depth -= 1;
            }
            '$' => {
                open(&mut start, at);
                maths = !maths;
            }
            ch if ch.is_whitespace() && depth == 0 && !maths => {
                if let Some(from) = start.take() {
                    words.push(content[from..at].trim());
                }
            }
            _ => open(&mut start, at),
        }
    }
    if let Some(from) = start {
        words.push(content[from..].trim());
    }
    words.retain(|word| !word.is_empty());
    words
}

/// Blocks in reading order, narrowed to what this export should contain.
///
/// Three filters, independent of each other: what the teacher set aside, who
/// the document is for, and how far the class has got. All are applied here
/// rather than through LaTeX conditionals, so a `.tex` handed to a class holds
/// no trace of what was left out — neither the answers reserved for the
/// teacher, nor a passage set aside, nor next week's lesson commented out at
/// the end of the file.
///
/// `taught_only` on a document nobody has marked keeps everything. The command
/// refuses that combination before reaching this point, because "as far as the
/// class has got" has no answer then, and quietly answering "all of it" is the
/// one mistake that cannot be taken back once the mail is sent.
pub fn kept<'a>(
    transcript: &'a Transcript,
    audience: &str,
    taught_only: bool,
) -> Vec<&'a Block> {
    let mut out = Vec::new();
    for page in &transcript.pages {
        for block in &page.blocks {
            if !block.hidden && keeps(block, audience) {
                out.push(block);
            }
            // Inclusive: the marked passage is the last one taught, not the
            // first one still to come. Checked even when a filter dropped the
            // block, or a teacher-only or set-aside boundary would run on.
            if taught_only && block.taught_end {
                return out;
            }
        }
    }
    out
}

/// Builds the complete `.tex` for one audience.
///
/// `audience` is `all`, `teacher` or `student`; `taught_only` stops the
/// document after the passage the class reached; `blanks` leaves a ruled
/// space where the teacher marked words, for a pupil working under a PAP.
pub fn render_document(
    root: &Path,
    template: &Template,
    transcript: &Transcript,
    title: &str,
    audience: &str,
    taught_only: bool,
    blanks: bool,
) -> std::io::Result<String> {
    let mut out = crate::templates::render_preamble(root, template)?;
    out.push_str("\n\\begin{document}\n\n");

    let mut wrote_chapter = false;
    for block in kept(transcript, audience, taught_only) {
        if block.kind == "chapter" {
            wrote_chapter = true;
        }
        warn_about_layout(block);
        warn_about_alignment(block);
        out.push_str(&render_block(template, block, blanks));
        out.push_str("\n\n");
    }

    // A document without a recognised chapter heading still deserves a title:
    // the document's own, through the same mapping a chapter block would use.
    // It was written as `\chapitre{title}` — one argument to a two-argument
    // command, so the title landed in the number's place and the exercise
    // sheet came out headed « Fiche d'exercices — » with nothing after it.
    if !wrote_chapter {
        let heading = format!("{}\n\n", fallback_heading(template, title));
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

    /// The ordinary copy carries no trace of what was marked, and the adapted
    /// one leaves exactly the room the words took.
    #[test]
    fn a_marked_run_is_printed_or_left_blank() {
        let latex = "Deux vecteurs sont \\trou{colinéaires} lorsque...";
        assert_eq!(
            apply_gaps(latex, false),
            "Deux vecteurs sont colinéaires lorsque...",
            "the everyday PDF must not show the marking"
        );
        assert_eq!(
            apply_gaps(latex, true),
            "Deux vecteurs sont \
             \\underline{\\vphantom{Ag}\\makebox[1.5\\width]{\\hphantom{colinéaires}}} lorsque..."
        );
    }

    /// A run of several words becomes one hole per word: the line can break
    /// inside it, and the pupil sees how many words are missing.
    #[test]
    fn each_word_of_a_run_keeps_its_own_width() {
        let blanked = apply_gaps("On dit \\trou{de même direction} quand...", true);
        assert_eq!(blanked.matches("\\underline").count(), 3);
        assert!(
            blanked.contains("\\hphantom{de}}} \\underline{"),
            "one hole per word, the ordinary space between them: the line may break there"
        );
    }

    /// A space inside maths or inside a command's argument does not separate
    /// two words: splitting there would cut the LaTeX in half.
    #[test]
    fn a_space_inside_maths_does_not_open_a_second_hole() {
        let blanked = apply_gaps("\\trou{$a + b$ et \\textbf{les deux}}", true);
        assert_eq!(blanked.matches("\\underline").count(), 3, "$a + b$, et, \\textbf{{...}}");
        // Wider than the word, so a hand fits: the reason the copies no
        // longer break their lines in the same places.
        assert_eq!(blanked.matches("\\makebox[1.5\\width]").count(), 3);
        assert!(blanked.contains("\\hphantom{$a + b$}"));
        assert!(blanked.contains("\\hphantom{\\textbf{les deux}}"));
    }

    /// `\trou` names a command only when a group follows it.
    #[test]
    fn a_word_that_merely_starts_like_the_mark_is_left_alone() {
        for latex in ["un \\trouble passager", "le trou du milieu"] {
            assert_eq!(apply_gaps(latex, true), latex);
            assert_eq!(apply_gaps(latex, false), latex);
        }
    }

    /// Several marks in one passage, and one holding braces of its own.
    #[test]
    fn marks_are_resolved_one_after_another() {
        let latex = "\\trou{premier} au milieu \\trou{\\emph{second}} fin";
        assert_eq!(apply_gaps(latex, false), "premier au milieu \\emph{second} fin");
        assert_eq!(apply_gaps(latex, true).matches("\\underline").count(), 2);
    }

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
            taught_end: false,
            hidden: false,
            reviewed: false,
        }
    }

    fn passage(id: &str, latex: &str, audience: &[&str]) -> Block {
        Block {
            id: id.into(),
            kind: "text".into(),
            title: None,
            number: None,
            latex: latex.into(),
            confidence: 1.0,
            doubt: None,
            audience: audience.iter().map(|a| a.to_string()).collect(),
            align: None,
            note: None,
            taught_end: false,
            hidden: false,
            reviewed: true,
        }
    }

    fn document(blocks: Vec<Block>) -> Transcript {
        Transcript {
            version: 1,
            pages: vec![crate::ir::Page { number: 1, blocks, session_id: None }],
        }
    }

    /// The marked passage is the last one taught, not the first one to come.
    #[test]
    fn the_boundary_keeps_the_passage_it_sits_on() {
        let mut transcript = document(vec![
            passage("p01-b01", "un", &[]),
            passage("p01-b02", "deux", &[]),
            passage("p01-b03", "trois", &[]),
        ]);
        crate::ir::mark_taught_end(&mut transcript, Some("p01-b02")).unwrap();

        let bodies: Vec<&str> = kept(&transcript, AUDIENCE_ALL, true)
            .iter()
            .map(|b| b.latex.as_str())
            .collect();
        assert_eq!(bodies, vec!["un", "deux"]);

        // The same document, whole, when the teacher asks for all of it.
        assert_eq!(kept(&transcript, AUDIENCE_ALL, false).len(), 3);
    }

    /// The two filters are independent, and the order matters: a boundary
    /// sitting on a teacher-only passage still ends the student handout. Read
    /// the other way round the student version would run on to the end of the
    /// document — the exact mistake the feature exists to prevent.
    #[test]
    fn a_teacher_only_boundary_still_ends_the_student_export() {
        let mut transcript = document(vec![
            passage("p01-b01", "énoncé", &[]),
            passage("p01-b02", "correction", &["teacher"]),
            passage("p01-b03", "semaine suivante", &[]),
        ]);
        crate::ir::mark_taught_end(&mut transcript, Some("p01-b02")).unwrap();

        let student: Vec<&str> = kept(&transcript, "student", true)
            .iter()
            .map(|b| b.latex.as_str())
            .collect();
        assert_eq!(student, vec!["énoncé"], "stops there without showing it");

        let teacher: Vec<&str> = kept(&transcript, "teacher", true)
            .iter()
            .map(|b| b.latex.as_str())
            .collect();
        assert_eq!(teacher, vec!["énoncé", "correction"]);
    }

    /// A passage set aside leaves every export, and still ends the one that
    /// stops where the class did when it carries the mark.
    #[test]
    fn a_passage_set_aside_is_left_out_of_every_export() {
        let mut transcript = document(vec![
            passage("p01-b01", "un", &[]),
            passage("p01-b02", "de côté", &[]),
            passage("p01-b03", "trois", &[]),
        ]);
        transcript.pages[0].blocks[1].hidden = true;

        for audience in [AUDIENCE_ALL, "teacher", "student"] {
            let bodies: Vec<&str> =
                kept(&transcript, audience, false).iter().map(|b| b.latex.as_str()).collect();
            assert_eq!(bodies, vec!["un", "trois"], "{audience}");
        }

        crate::ir::mark_taught_end(&mut transcript, Some("p01-b02")).unwrap();
        let taught: Vec<&str> =
            kept(&transcript, AUDIENCE_ALL, true).iter().map(|b| b.latex.as_str()).collect();
        assert_eq!(taught, vec!["un"], "the boundary holds even on a hidden passage");
    }

    /// The regression: without a chapter block the sheet was headed by the
    /// document's title, passed as the *number* of a two-argument command.
    #[test]
    fn the_fallback_title_takes_the_shape_of_a_chapter() {
        let template = bundled();
        assert_eq!(
            fallback_heading(&template, "Calcul littéral"),
            "\\chapitre{}{Calcul littéral}",
            "an empty number, then the title — exactly what a chapter block gives"
        );
    }

    /// Marking the last passage is a legitimate way to say "we finished".
    #[test]
    fn a_boundary_on_the_last_passage_keeps_everything() {
        let mut transcript =
            document(vec![passage("p01-b01", "un", &[]), passage("p01-b02", "deux", &[])]);
        crate::ir::mark_taught_end(&mut transcript, Some("p01-b02")).unwrap();

        assert_eq!(kept(&transcript, AUDIENCE_ALL, true).len(), 2);
    }

    fn bundled() -> Template {
        serde_json::from_str(crate::templates::BUILTIN_MANIFEST).expect("valid manifest")
    }

    /// The number belongs to the page. A document photographed from the middle of
    /// a notebook opens on "Chapitre 3" and must stay chapter 3.
    #[test]
    fn a_heading_carries_the_number_read_on_the_page() {
        let template = bundled();
        assert_eq!(
            render_block(&template, &heading("chapter", "Vecteurs", Some("3")), false),
            "\\chapitre{3}{Vecteurs}"
        );
        assert_eq!(
            render_block(&template, &heading("part", "Notion de vecteurs", Some("II")), false),
            "\\partie{II}{Notion de vecteurs}"
        );
    }

    /// And an absent number is passed as absent, never replaced by a guess.
    #[test]
    fn an_unnumbered_heading_gets_no_invented_number() {
        let template = bundled();
        assert_eq!(
            render_block(&template, &heading("chapter", "Vecteurs", None), false),
            "\\chapitre{}{Vecteurs}"
        );
        assert_eq!(
            render_block(&template, &heading("subpart", "Définition", Some("  ")), false),
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
    fn a_side_remark_column_is_fine() {
        // The `inline-comment` convention: `&&` opens a column for the remark,
        // which carries maths of its own inside `\text`.
        assert!(!has_stray_alignment(
            r"$\begin{aligned}[t] 5x &= 2 && \text{on met les $x$ d'un côté} \\ x &= 2 \end{aligned}$"
        ));
    }

    #[test]
    fn an_escaped_ampersand_is_not_an_alignment_tab() {
        assert!(!has_stray_alignment(r"Pierre \& Marie Curie"));
        assert!(!has_stray_alignment("Rien à signaler ici."));
    }
}
