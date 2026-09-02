//! Runs Claude Code headlessly to read one page.
//!
//! Plume never talks to an API: it shells out to the `claude` CLI the user
//! already installed and signed in, so recognition runs on their subscription.
//!
//! Two things measured on the way here shape this module:
//!  - `--allowedTools` is variadic and swallows a trailing positional argument,
//!    so the prompt goes through stdin;
//!  - `--json-schema` puts the validated object in `structured_output`, while
//!    `result` stays empty. No prose parsing.

use crate::ir;
use crate::logbus;
use serde::Serialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::Stdio;

/// Replaces Claude Code's default system prompt. Measured at ~8.3k input tokens
/// per call otherwise, for a task that needs none of it.
const SYSTEM_PROMPT: &str = r#"You transcribe one photographed page of a handwritten French maths course into structured blocks.

Rules:
- Read the page top to bottom. Emit one block per logical unit, in reading order.
- `latex` carries body content ONLY. Never emit \begin{...}/\end{...} wrappers for the block itself, never a preamble, never \section or \chapter — the caller wraps each block according to its kind.
- For heading blocks (`chapter`, `part`, `subpart`, `paragraph`): put the heading text ALONE in `title` — without its number and without the words "Chapitre", "Partie" — and leave `latex` empty. Put the number exactly as the page writes it in `number`: "3", "II", "1", "a". Plume never renumbers, so a course opening on "Chapitre 3" stays chapter 3. If the page shows no number, leave `number` empty rather than inventing one.
- Emit CONTENT, never page layout. No `minipage`, no `tabular` used for placement, no `multicols`, no `\rule`, no `\hfill`, no `\vspace`, no `\newpage`. Columns and spacing belong to the template, and a layout you invent will not fit the page. If the page shows two things side by side, emit them as two consecutive blocks.
- A keyword introducing a passage — « Définition : », « Propriété : », « Exemple : », « Remarque : » — is that block's own label, not a heading. Emit the block itself and nothing else; never a separate `paragraph` or `subpart` repeating the keyword above it. Emit a heading block only for a heading the page itself sets apart as one, with its own number.
- Set `title` ONLY when the page itself writes a title next to the keyword, e.g. « Définition (vecteurs colinéaires) : ». If the page just says « Définition : », leave `title` empty. Never repeat the environment's own name as its title.
- A diagram that belongs to an example or a proof stays inside that block's `latex`, wrapped in \begin{center}...\end{center}. Use a standalone `figure` block only for a diagram that stands on its own.
- A numbered or bulleted list is `enumerate` or `itemize` with `\item`. Never number the lines by hand: written as « 1)\quad ... », the first item runs on after the environment's own label instead of starting its own line.
- Maths in LaTeX: $...$ inline, \[...\] displayed. Vectors as \overrightarrow{AB} or \vec{u}.
- A drawn diagram is a `figure` block whose `latex` is one complete tikzpicture environment. Redraw the mathematical object cleanly with named \coordinate and \draw[->]; do not trace the handwriting stroke by stroke. Map pen colour to semantic colours already defined by the document: black or blue -> mcTexte, red -> mcDef, green -> mcProp. Construction lines are dashed in mcTexte.
- `audience` lists which exports keep the block, and defaults to both: ["teacher", "student"]. Restrict it to ["teacher"] ONLY when the teacher's own reading conventions below describe a visual marker that means "my copy only" AND that marker is present on this block. Never restrict a block because you judge it too hard, too detailed, or answer-like — that decision belongs to the teacher, not to you.
- `confidence` is your honest certainty for that block. Below 0.85 you MUST fill `doubt` with one short French sentence naming exactly what is unclear.
- Never invent content that is not on the page. A block you cannot read is a low-confidence block, not a guess.
- The course text is French: transcribe it verbatim, keeping the author's wording and accents."#;

/// Marks a page that stopped because the teacher cancelled, so the caller can
/// tell it apart from a genuine failure.
pub const CANCELLED: &str = "__plume_cancelled__";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageOutcome {
    pub page: ir::Page,
    pub cost_usd: f64,
    pub duration_ms: u64,
    pub turns: u32,
}

/// Turns a hand-numbered run of lines into a real `enumerate`.
///
/// A property listing four rules came back as « 1)\quad ... » separated by
/// blank lines. LaTeX starts an environment's first paragraph on the label's
/// own line, so the PDF read "Propriété : 1) ..." with the rest beneath, while
/// the review — which stacks — had promised otherwise. An `enumerate` starts on
/// its own line in both.
///
/// Only a run of at least two paragraphs, each opening with `n)` or `n.` in
/// order from one, is converted. Anything else is left as written.
fn enumerate_hand_numbered(latex: &str) -> Option<String> {
    let paragraphs: Vec<&str> = latex.split("\n\n").map(str::trim).collect();
    if paragraphs.len() < 2 {
        return None;
    }

    let mut items: Vec<String> = Vec::new();
    for (index, paragraph) in paragraphs.iter().enumerate() {
        let expected = (index + 1).to_string();
        let rest = paragraph
            .strip_prefix(&expected)
            .and_then(|r| r.strip_prefix(')').or_else(|| r.strip_prefix('.')))?;
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("\\quad").unwrap_or(rest);
        items.push(rest.trim().to_string());
    }

    if items.iter().any(|item| item.is_empty()) {
        return None;
    }
    Some(format!(
        "\\begin{{enumerate}}\n{}\n\\end{{enumerate}}",
        items
            .iter()
            .map(|item| format!("\\item {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

/// Page-layout constructs the prompt forbids, and how many braced arguments
/// each one swallows. An optional `[...]` is always taken first.
const LAYOUT: &[(&str, usize)] = &[
    ("\\begin{minipage}", 1),
    ("\\end{minipage}", 0),
    ("\\begin{multicols}", 1),
    ("\\end{multicols}", 0),
    ("\\rule", 2),
    ("\\vspace*", 1),
    ("\\vspace", 1),
    ("\\hfill", 0),
    ("\\newpage", 0),
    ("\\clearpage", 0),
];

/// Consumes a balanced `[..]` or `{..}` starting at `from`, if one is there.
fn skip_group(chars: &[char], from: usize, open: char, close: char) -> Option<usize> {
    let mut at = from;
    while chars.get(at).is_some_and(|c| *c == ' ' || *c == '\n') {
        at += 1;
    }
    if chars.get(at) != Some(&open) {
        return None;
    }
    let mut depth = 0usize;
    while at < chars.len() {
        if chars[at] == open {
            depth += 1;
        } else if chars[at] == close {
            depth -= 1;
            if depth == 0 {
                return Some(at + 1);
            }
        }
        at += 1;
    }
    None
}

/// Removes the page layout a passage invented for itself.
///
/// The prompt forbids these outright — columns and spacing belong to the
/// charte — but the model reaches for them anyway when a page shows two things
/// side by side. Warning was not enough: two `minipage` separated by a rule
/// came out of the compiler as a blank half-page, a rule hanging in the margin
/// and overlapping lines. Stripped, the same passage stacks, which is what the
/// charte expects and what the review already showed.
///
/// Only the wrappers go; everything they contained stays, in order.
fn strip_layout(latex: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = latex.chars().collect();
    let mut out = String::new();
    let mut removed: Vec<String> = Vec::new();
    let mut at = 0usize;

    'outer: while at < chars.len() {
        if chars[at] == '\\' {
            for (needle, groups) in LAYOUT {
                let end = at + needle.chars().count();
                if end <= chars.len() && chars[at..end].iter().collect::<String>() == **needle {
                    let mut after = end;
                    if let Some(next) = skip_group(&chars, after, '[', ']') {
                        after = next;
                    }
                    for _ in 0..*groups {
                        match skip_group(&chars, after, '{', '}') {
                            Some(next) => after = next,
                            None => break,
                        }
                    }
                    if !removed.iter().any(|r| r == needle) {
                        removed.push((*needle).to_string());
                    }
                    at = after;
                    continue 'outer;
                }
            }
        }
        out.push(chars[at]);
        at += 1;
    }

    // Removing a wrapper leaves the blank lines that surrounded it.
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    (out.trim().to_string(), removed)
}

/// The French name a page uses for each environment, folded for comparison.
///
/// Only what a teacher writes above a box — not every block kind — because the
/// point is to recognise a keyword the model turned into a heading.
fn environment_word(kind: &str) -> Option<&'static str> {
    match kind {
        "definition" => Some("definition"),
        "property" => Some("propriete"),
        "theorem" => Some("theoreme"),
        "method" => Some("methode"),
        "example" => Some("exemple"),
        "application" => Some("application"),
        "remark" => Some("remarque"),
        "proof" => Some("demonstration"),
        _ => None,
    }
}

/// Accents and a trailing plural removed, for comparing a heading to a keyword.
fn folded(text: &str) -> String {
    let mut out: String = text
        .trim()
        .trim_end_matches([':', ' '])
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'û' | 'ü' | 'ù' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect();
    if out.ends_with('s') {
        out.pop();
    }
    out
}

/// Drops a heading that only repeats the keyword of the box beneath it.
///
/// A page reading "Propriété :" above a framed box gave two blocks: a heading
/// titled "Propriétés", then the property itself. Nothing was missing from the
/// transcription — there was one passage too many, and it appeared on every
/// page holding a box.
///
/// The test is deliberately narrow. The heading must carry no number, because
/// this template numbers its real headings and the number now comes from the
/// page; it must have no body of its own; and the very next block must be the
/// environment it names. Anything else is left alone and reported, since a
/// heading the teacher wrote is theirs to keep.
fn drop_echoed_headings(blocks: &mut Vec<ir::Block>) {
    let mut redundant: Vec<usize> = Vec::new();

    for (index, block) in blocks.iter().enumerate() {
        let is_heading = matches!(block.kind.as_str(), "part" | "subpart" | "paragraph");
        let numbered = block.number.as_deref().map(str::trim).is_some_and(|n| !n.is_empty());
        let has_body = !block.latex.trim().is_empty();
        if !is_heading || numbered || has_body {
            continue;
        }

        let Some(title) = block.title.as_deref().map(folded).filter(|t| !t.is_empty()) else {
            continue;
        };
        let follows = blocks.get(index + 1).and_then(|next| environment_word(&next.kind));
        if follows == Some(title.as_str()) {
            redundant.push(index);
        }
    }

    for index in redundant.iter().rev() {
        let dropped = blocks.remove(*index);
        logbus::detail(
            "claude",
            format!(
                "Titre « {} » retiré : il ne fait que répéter l'encadré qui suit",
                dropped.title.unwrap_or_default()
            ),
            "aucun contenu perdu — le mot-clé appartient à l'encadré".to_string(),
        );
    }
}

/// Coarse, user-safe reading of one stream event.
///
/// The stream carries the model's actual words and tool calls; none of that
/// belongs on screen. What the teacher needs is proof of life, so events are
/// flattened to a handful of French labels — and anything unrecognised maps to
/// nothing rather than leaking.
fn classify(event: &serde_json::Value) -> Option<&'static str> {
    match event.get("type")?.as_str()? {
        "system" => Some("Préparation de la lecture…"),
        "user" => Some("Photo chargée…"),
        "assistant" => {
            let blocks = event.pointer("/message/content")?.as_array()?;
            let mut has_text = false;
            for block in blocks {
                match block.get("type").and_then(|v| v.as_str()) {
                    Some("tool_use") => {
                        return match block.get("name").and_then(|v| v.as_str()) {
                            Some("Read") => Some("Examine la photo…"),
                            _ => Some("Met les blocs en forme…"),
                        };
                    }
                    Some("text") => has_text = true,
                    _ => {}
                }
            }
            has_text.then_some("Analyse le contenu…")
        }
        _ => None,
    }
}

/// Transcribes `pages/NN.ext` inside `document_dir`.
///
/// `reading_rules` is the user's own natural-language instruction block (their
/// highlighter conventions, teacher-only markers...). It is appended verbatim,
/// in whatever language they wrote it.
///
/// `on_activity` receives the heartbeat labels from `classify` as the model
/// works: a page takes a minute or two, and a silent minute reads as a crash.
pub fn transcribe_page(
    run_id: &str,
    document_dir: &Path,
    page_number: usize,
    // `image_path` is relative to `document_dir` — `pages/03.jpg`, or an
    // extract added later — and `context` is the one sentence that tells the
    // model what it is looking at.
    image_path: &str,
    context: &str,
    model: &str,
    reading_rules: &str,
    on_activity: &dyn Fn(&'static str),
) -> Result<PageOutcome, String> {
    let mut system = SYSTEM_PROMPT.to_string();
    if !reading_rules.trim().is_empty() {
        system.push_str("\n\nReading conventions defined by the teacher (authoritative):\n");
        system.push_str(reading_rules.trim());
    }

    let prompt = format!("Read the image `{image_path}` and transcribe it. {context}");

    logbus::detail(
        "claude",
        format!("Page {page_number} — lecture de {image_path}"),
        format!("modèle {model} · consigne {} caractères", system.len()),
    );

    let claude = crate::env_check::resolve_tool("claude")
        .ok_or("Claude Code est introuvable. Vérifiez le panneau « État du système ».")?;

    logbus::debug(
        "claude",
        format!("Page {page_number} — commande"),
        format!(
            "{} -p --model {model} --output-format json --json-schema <{} o> --system-prompt <{} o> --allowedTools Read",
            claude.display(),
            ir::page_schema().len(),
            system.len()
        ),
    );

    let started = std::time::Instant::now();
    let mut child = crate::proc::quiet(claude)
        .current_dir(document_dir)
        .arg("-p")
        .arg("--model")
        .arg(model)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--json-schema")
        .arg(ir::page_schema())
        .arg("--system-prompt")
        .arg(&system)
        .arg("--allowedTools")
        .arg("Read")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Lancement de Claude Code impossible : {e}"))?;

    // Watched so a cancellation can kill it: waiting for a page to finish would
    // keep spending quota for another minute or two.
    let pid = child.id();
    crate::runs::watch(run_id, pid);

    child
        .stdin
        .take()
        .ok_or("Entrée standard indisponible.")?
        .write_all(prompt.as_bytes())
        .map_err(|e| format!("Envoi de la consigne : {e}"))?;

    // The stream is read as it arrives: each event becomes a heartbeat, and the
    // final `result` line carries the same envelope the one-shot format did.
    let stdout = child.stdout.take().ok_or("Sortie standard indisponible.")?;
    let mut envelope: Option<serde_json::Value> = None;
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if event.get("type").and_then(|v| v.as_str()) == Some("result") {
            envelope = Some(event);
        } else if let Some(label) = classify(&event) {
            on_activity(label);
        }
    }

    let mut stderr_text = String::new();
    if let Some(mut stream) = child.stderr.take() {
        let _ = stream.read_to_string(&mut stderr_text);
    }
    let status = child
        .wait()
        .map_err(|e| format!("Exécution de Claude Code : {e}"))?;
    crate::runs::unwatch(run_id, pid);

    if crate::runs::is_cancelled(run_id) {
        return Err(CANCELLED.to_string());
    }

    if !status.success() {
        let first = stderr_text.lines().next().unwrap_or("").trim().to_string();
        logbus::error("claude", format!("Page {page_number} — {status}"));
        return Err(format!(
            "Claude Code s'est arrêté ({status}). {}",
            if first.is_empty() {
                "Aucun message d'erreur — détail dans la console."
            } else {
                &first
            }
        ));
    }

    let envelope = envelope.ok_or("Claude Code n'a renvoyé aucun résultat.")?;

    if envelope.get("is_error").and_then(|v| v.as_bool()) == Some(true) {
        return Err(format!(
            "Claude Code a signalé une erreur : {}",
            envelope.get("result").and_then(|v| v.as_str()).unwrap_or("")
        ));
    }

    let structured = envelope
        .get("structured_output")
        .ok_or("Claude Code n'a pas renvoyé de sortie structurée.")?;

    let blocks = structured
        .get("blocks")
        .and_then(|v| v.as_array())
        .ok_or("Sortie structurée sans blocs.")?;

    // Ids are assigned here, not by the model: they must be stable and unique
    // across the document, and review anchors depend on that.
    //
    // Parsing failures are raised, never skipped. Silently dropping a block
    // turns a schema mismatch into "0 blocks" after the quota has been spent.
    let blocks: Vec<ir::Block> = blocks
        .iter()
        .enumerate()
        .map(|(index, raw)| {
            let mut block: ir::Block = serde_json::from_value(raw.clone()).map_err(|e| {
                format!("Bloc {} de la page {page_number} illisible : {e}", index + 1)
            })?;
            block.id = format!("p{page_number:02}-b{:02}", index + 1);
            if block.audience.is_empty() {
                block.audience = vec!["teacher".into(), "student".into()];
            }
            Ok(block)
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut blocks = blocks;
    for block in &mut blocks {
        if let Some(listed) = enumerate_hand_numbered(&block.latex) {
            logbus::detail(
                "claude",
                format!("Liste numérotée à la main convertie en enumerate (page {page_number})"),
                "sinon le premier élément se colle à l'intitulé de l'encadré".to_string(),
            );
            block.latex = listed;
        }
        let (cleaned, removed) = strip_layout(&block.latex);
        if !removed.is_empty() {
            logbus::warn(
                "claude",
                format!(
                    "Mise en page retirée d'un passage de la page {page_number} ({}) — \
                     le contenu est conservé, empilé.",
                    removed.join(", ")
                ),
            );
            block.latex = cleaned;
        }
    }
    drop_echoed_headings(&mut blocks);
    for (index, block) in blocks.iter_mut().enumerate() {
        block.id = format!("p{page_number:02}-b{:02}", index + 1);
    }

    if blocks.is_empty() {
        return Err(format!(
            "La page {page_number} n'a produit aucun bloc. Vérifiez que l'image est lisible."
        ));
    }

    let outcome = PageOutcome {
        page: ir::Page {
            number: page_number,
            blocks,
            // Kept so a correction can resume this exact conversation instead of
            // re-reading the page from scratch.
            session_id: envelope
                .get("session_id")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        },
        cost_usd: envelope
            .get("total_cost_usd")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        duration_ms: envelope
            .get("duration_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        turns: envelope
            .get("num_turns")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    };

    logbus::detail(
        "claude",
        format!(
            "Page {page_number} — {} blocs en {:.0} s",
            outcome.page.blocks.len(),
            started.elapsed().as_secs_f32()
        ),
        format!(
            "{} tours · {:.3} $ · {} ms côté Claude",
            outcome.turns, outcome.cost_usd, outcome.duration_ms
        ),
    );

    Ok(outcome)
}

/// Re-runs one block after the teacher annotated it.
///
/// Resumes the page's original session when there is one: the image and the
/// instruction block are already in that context, so a correction costs a
/// fraction of a fresh page. Without a session it falls back to a full read of
/// the page image, which still works but is not cheap.
pub fn correct_block(
    run_id: &str,
    document_dir: &Path,
    page_number: usize,
    image_name: &str,
    session_id: Option<&str>,
    block: &ir::Block,
    note: &str,
    model: &str,
) -> Result<(ir::Block, f64), String> {
    let current = serde_json::to_string_pretty(&serde_json::json!({
        "kind": block.kind,
        "title": block.title,
        "latex": block.latex,
        "audience": block.audience,
    }))
    .unwrap_or_default();

    let prompt = format!(
        "The teacher reviewed one block of page {page_number} and asked for a change.\n\n         Current block:\n{current}\n\n         Their instruction (authoritative, follow it exactly):\n{note}\n\n         Re-read `pages/{image_name}` if you need to check the source, and return the corrected block.          Change only what the instruction asks for; keep everything else identical."
    );

    let claude = crate::env_check::resolve_tool("claude")
        .ok_or("Claude Code est introuvable. Vérifiez le panneau « État du système ».")?;

    logbus::detail(
        "claude",
        format!("Correction du bloc {}", block.id),
        match session_id {
            Some(id) => format!("reprise de la session {id}"),
            None => "sans session — relecture complète de la page".to_string(),
        },
    );

    let started = std::time::Instant::now();
    let mut command = crate::proc::quiet(claude);
    command.current_dir(document_dir).arg("-p");
    if let Some(id) = session_id {
        command.arg("--resume").arg(id);
    }
    command
        .arg("--model")
        .arg(model)
        .arg("--output-format")
        .arg("json")
        .arg("--json-schema")
        .arg(ir::block_schema())
        .arg("--allowedTools")
        .arg("Read")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("Lancement de Claude Code impossible : {e}"))?;

    let pid = child.id();
    crate::runs::watch(run_id, pid);

    child
        .stdin
        .take()
        .ok_or("Entrée standard indisponible.")?
        .write_all(prompt.as_bytes())
        .map_err(|e| format!("Envoi de la consigne : {e}"))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Exécution de Claude Code : {e}"))?;
    crate::runs::unwatch(run_id, pid);

    if crate::runs::is_cancelled(run_id) {
        return Err(CANCELLED.to_string());
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr.lines().next().unwrap_or("").trim().to_string();
        logbus::error(
            "claude",
            format!("Correction du bloc {} échouée ({}) : {first}", block.id, output.status),
        );
        return Err(format!("La correction a échoué ({}). {first}", output.status));
    }

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Réponse illisible de Claude Code : {e}"))?;

    let structured = envelope
        .get("structured_output")
        .ok_or("Claude Code n'a pas renvoyé de sortie structurée.")?;

    let mut corrected: ir::Block = serde_json::from_value(structured.clone())
        .map_err(|e| format!("Bloc corrigé illisible : {e}"))?;

    // Identity and review state belong to Plume, not to the model.
    corrected.id = block.id.clone();
    corrected.note = None;
    corrected.reviewed = true;
    if corrected.audience.is_empty() {
        corrected.audience = block.audience.clone();
    }

    let cost_usd = envelope.get("total_cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0);
    logbus::detail(
        "claude",
        format!("Bloc {} corrigé en {:.0} s", block.id, started.elapsed().as_secs_f32()),
        format!(
            "{cost_usd:.3} $ · {} tours",
            envelope.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0)
        ),
    );

    Ok((corrected, cost_usd))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property that read "Propriété : 1) ..." in the PDF while the review
    /// stacked all four rules.
    #[test]
    fn a_hand_numbered_run_becomes_a_list() {
        let latex = "1)\\quad $\\dfrac{a}{c} + \\dfrac{b}{c} = \\dfrac{a+b}{c}$\n\n\
                     2)\\quad $\\dfrac{a}{c} - \\dfrac{b}{c} = \\dfrac{a-b}{c}$\n\n\
                     3)\\quad $\\dfrac{a}{b} \\times \\dfrac{c}{d} = \\dfrac{a \\times c}{b \\times d}$";

        let listed = enumerate_hand_numbered(latex).expect("a list");

        assert!(listed.starts_with("\\begin{enumerate}"));
        assert!(listed.ends_with("\\end{enumerate}"));
        assert_eq!(listed.matches("\\item").count(), 3);
        assert!(!listed.contains("\\quad"), "the hand spacing goes with the hand numbering");
        assert!(!listed.contains("1)"));
        assert!(listed.contains("\\dfrac{a+b}{c}"), "the content is untouched");
    }

    #[test]
    fn anything_that_is_not_a_list_is_left_alone() {
        for latex in [
            // A single paragraph, numbered or not.
            "1)\\quad une seule ligne",
            "Un vecteur est défini par une direction, un sens et une longueur.",
            // Numbers out of order, or not starting at one.
            "2) deux\n\n3) trois",
            "1) un\n\n3) trois",
            // Ordinary paragraphs that happen to follow each other.
            "Première phrase.\n\nSeconde phrase.",
            // A run whose items would be empty.
            "1)\n\n2)",
        ] {
            assert!(
                enumerate_hand_numbered(latex).is_none(),
                "wrongly converted: {latex}"
            );
        }
    }

    /// The passage that came out of a real page and broke the PDF: two
    /// columns with a rule between them, from a model told not to.
    #[test]
    fn the_two_column_passage_becomes_a_stack() {
        let latex = "\\begin{minipage}[t]{0.48\\textwidth}\n\
                     \\textbf{Relation de Chasles :}\n\n\
                     \\begin{center}A\\end{center}\n\
                     \\end{minipage}%\n\
                     \\hfill\\rule{0.4pt}{6cm}\\hfill%\n\
                     \\begin{minipage}[t]{0.48\\textwidth}\n\
                     \\textbf{Règle du parallélogramme :}\n\n\
                     \\begin{center}B\\end{center}\n\
                     \\end{minipage}";

        let (cleaned, removed) = strip_layout(latex);

        assert!(!cleaned.contains("minipage"));
        assert!(!cleaned.contains("\\rule"));
        assert!(!cleaned.contains("\\hfill"));
        assert!(!cleaned.contains("0.48"), "the width argument goes with its wrapper");

        // Everything the wrappers held is still there, in order.
        assert!(cleaned.contains("Relation de Chasles"));
        assert!(cleaned.contains("\\begin{center}A\\end{center}"));
        assert!(cleaned.contains("Règle du parallélogramme"));
        assert!(cleaned.contains("\\begin{center}B\\end{center}"));
        assert!(
            cleaned.find("Chasles") < cleaned.find("parallélogramme"),
            "order is preserved"
        );

        assert!(removed.contains(&"\\begin{minipage}".to_string()));
        assert!(removed.contains(&"\\rule".to_string()));
    }

    #[test]
    fn ordinary_content_is_returned_untouched() {
        for latex in [
            "Soient $\\vec{u}$ et $\\vec{v}$ deux vecteurs.",
            "\\begin{center}\n\\begin{tikzpicture}\\draw (0,0) -- (1,1);\\end{tikzpicture}\n\\end{center}",
            "$\\begin{aligned}[t] a &= b \\\\ &= c \\end{aligned}$",
            "\\begin{itemize}\\item une direction\\end{itemize}",
        ] {
            let (cleaned, removed) = strip_layout(latex);
            assert_eq!(cleaned, latex.trim(), "changed: {latex}");
            assert!(removed.is_empty());
        }
    }

    #[test]
    fn spacing_commands_go_with_their_argument() {
        let (cleaned, _) = strip_layout("avant\\vspace{1cm}après");
        assert_eq!(cleaned, "avantaprès");
        let (cleaned, _) = strip_layout("avant\\vspace*{2\\baselineskip}après");
        assert_eq!(cleaned, "avantaprès");
        // A rule takes an optional lift and two lengths.
        let (cleaned, _) = strip_layout("a\\rule[2pt]{0.4pt}{6cm}b");
        assert_eq!(cleaned, "ab");
    }

    fn block(kind: &str, title: Option<&str>, number: Option<&str>, latex: &str) -> ir::Block {
        ir::Block {
            id: String::new(),
            kind: kind.into(),
            title: title.map(str::to_string),
            number: number.map(str::to_string),
            latex: latex.into(),
            confidence: 0.9,
            doubt: None,
            audience: Vec::new(),
            align: None,
            note: None,
            reviewed: false,
        }
    }

    /// The shape read off a real page: "Propriété :" above a framed box came
    /// back as a heading and then the box.
    #[test]
    fn a_heading_that_only_names_the_box_below_is_dropped() {
        let mut blocks = vec![
            block("subpart", Some("Règles de calcul"), Some("2"), ""),
            block("paragraph", Some("Propriétés"), None, ""),
            block("property", None, None, "a + b"),
            block("paragraph", Some("Exemple"), None, ""),
            block("example", None, None, "A = ..."),
        ];

        drop_echoed_headings(&mut blocks);

        let kinds: Vec<&str> = blocks.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(kinds, vec!["subpart", "property", "example"]);
        // The real heading, numbered on the page, stays.
        assert_eq!(blocks[0].title.as_deref(), Some("Règles de calcul"));
    }

    #[test]
    fn a_heading_the_teacher_wrote_is_left_alone() {
        let cases = vec![
            // Numbered on the page: a heading of theirs, whatever it says.
            vec![
                block("paragraph", Some("Propriétés"), Some("a"), ""),
                block("property", None, None, "corps"),
            ],
            // Carries its own text, so it is not a bare echo.
            vec![
                block("paragraph", Some("Propriétés"), None, "Trois règles suivent."),
                block("property", None, None, "corps"),
            ],
            // Names something other than what follows.
            vec![
                block("paragraph", Some("Exercices"), None, ""),
                block("property", None, None, "corps"),
            ],
            // Nothing follows it at all.
            vec![block("paragraph", Some("Propriétés"), None, "")],
        ];

        for mut blocks in cases {
            let before = blocks.len();
            drop_echoed_headings(&mut blocks);
            assert_eq!(blocks.len(), before, "nothing should have been dropped");
        }
    }

    #[test]
    fn the_comparison_ignores_accents_plurals_and_a_trailing_colon() {
        assert_eq!(folded("Propriétés"), "propriete");
        assert_eq!(folded("PROPRIÉTÉ :"), "propriete");
        assert_eq!(folded("  Démonstration  "), "demonstration");
        assert_ne!(folded("Exercices"), "exemple");
    }

    fn event(raw: &str) -> serde_json::Value {
        serde_json::from_str(raw).expect("valid test event")
    }

    /// Shapes captured from a real `--output-format stream-json` run.
    #[test]
    fn classify_covers_the_observed_stream() {
        assert_eq!(
            classify(&event(r#"{"type":"system","subtype":"init"}"#)),
            Some("Préparation de la lecture…")
        );
        assert_eq!(
            classify(&event(
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}}]}}"#
            )),
            Some("Examine la photo…")
        );
        assert_eq!(
            classify(&event(
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"StructuredOutput","input":{}}]}}"#
            )),
            Some("Met les blocs en forme…")
        );
        assert_eq!(
            classify(&event(
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}"#
            )),
            Some("Analyse le contenu…")
        );
        assert_eq!(classify(&event(r#"{"type":"user","message":{}}"#)), Some("Photo chargée…"));
    }

    /// The model's words must never leak through the heartbeat, and unknown
    /// event types must map to silence rather than a guess.
    #[test]
    fn classify_stays_quiet_on_unknown_events() {
        assert_eq!(classify(&event(r#"{"type":"rate_limit_event"}"#)), None);
        assert_eq!(classify(&event(r#"{"type":"result","subtype":"success"}"#)), None);
        assert_eq!(classify(&event(r#"{"no_type":true}"#)), None);
    }
}
