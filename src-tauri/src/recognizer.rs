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
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

/// Replaces Claude Code's default system prompt. Measured at ~8.3k input tokens
/// per call otherwise, for a task that needs none of it.
const SYSTEM_PROMPT: &str = r#"You transcribe one photographed page of a handwritten French maths course into structured blocks.

Rules:
- Read the page top to bottom. Emit one block per logical unit, in reading order.
- `latex` carries body content ONLY. Never emit \begin{...}/\end{...} wrappers for the block itself, never a preamble, never \section or \chapter — the caller wraps each block according to its kind.
- For heading blocks (`chapter`, `part`, `subpart`, `paragraph`): put the heading text ALONE in `title` — without its handwritten number and without the words "Chapitre", "Partie" — and leave `latex` empty. The template renumbers headings itself.
- Emit CONTENT, never page layout. No `minipage`, no `tabular` used for placement, no `multicols`, no `\rule`, no `\hfill`, no `\vspace`, no `\newpage`. Columns and spacing belong to the template, and a layout you invent will not fit the page. If the page shows two things side by side, emit them as two consecutive blocks.
- Set `title` ONLY when the page itself writes a title next to the keyword, e.g. « Définition (vecteurs colinéaires) : ». If the page just says « Définition : », leave `title` empty. Never repeat the environment's own name as its title.
- A diagram that belongs to an example or a proof stays inside that block's `latex`, wrapped in \begin{center}...\end{center}. Use a standalone `figure` block only for a diagram that stands on its own.
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

/// Transcribes `pages/NN.ext` inside `document_dir`.
///
/// `reading_rules` is the user's own natural-language instruction block (their
/// highlighter conventions, teacher-only markers...). It is appended verbatim,
/// in whatever language they wrote it.
pub fn transcribe_page(
    run_id: &str,
    document_dir: &Path,
    page_number: usize,
    image_name: &str,
    model: &str,
    reading_rules: &str,
) -> Result<PageOutcome, String> {
    let mut system = SYSTEM_PROMPT.to_string();
    if !reading_rules.trim().is_empty() {
        system.push_str("\n\nReading conventions defined by the teacher (authoritative):\n");
        system.push_str(reading_rules.trim());
    }

    let prompt = format!(
        "Read the image `pages/{image_name}` and transcribe it. It is page {page_number} of the course."
    );

    logbus::detail(
        "claude",
        format!("Page {page_number} — lecture de pages/{image_name}"),
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
        .arg("json")
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
        logbus::error("claude", format!("Page {page_number} — {}", output.status));
        return Err(format!(
            "Claude Code s'est arrêté ({}). {}",
            output.status,
            if first.is_empty() {
                "Aucun message d'erreur — détail dans la console."
            } else {
                &first
            }
        ));
    }

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Réponse illisible de Claude Code : {e}"))?;

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
) -> Result<ir::Block, String> {
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

    logbus::detail(
        "claude",
        format!("Bloc {} corrigé en {:.0} s", block.id, started.elapsed().as_secs_f32()),
        format!(
            "{:.3} $ · {} tours",
            envelope.get("total_cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0),
            envelope.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0)
        ),
    );

    Ok(corrected)
}
