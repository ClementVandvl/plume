//! A conversation with Claude about the whole document.
//!
//! A note on one passage asks for one passage back. Some requests are about the
//! document as a whole — put the four definitions side by side in a grid,
//! number the parts A, B, C instead of I, II, III, reserve every answer for the
//! teacher's copy — and would take a note on every passage concerned, or could
//! not be said passage by passage at all.
//!
//! The model reads the whole transcript and answers with **changes**, never
//! with a new document: a passage it does not name stays byte for byte what it
//! was. Rewriting forty passages to renumber four headings would cost minutes
//! of output and let anything drift on the way through.
//!
//! Plume keeps the last word on structure, exactly as for a reading: ids are
//! Plume's, the mark of where the class stopped and a passage set aside survive
//! an edit, and a change naming a passage that is not there — or that the
//! teacher edited while Claude was thinking — refuses the whole answer rather
//! than half of it.

use crate::ir::{self, Block, Transcript};
use crate::templates::Template;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

const LOG_FILE: &str = "chat.json";

/// Messages sent back with each request, so « et les propriétés aussi » means
/// something. Few on purpose: every one is paid for again on each turn, and the
/// document itself is always sent whole and current.
const REMEMBERED: usize = 6;

/// Messages kept on disk. The conversation is a working tool, not an archive.
const KEPT_MESSAGES: usize = 60;

const HEADINGS: &[&str] = &["chapter", "part", "subpart", "paragraph"];

/// What one answer did to the document, for the line under Claude's reply.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tally {
    pub edited: usize,
    pub added: usize,
    pub removed: usize,
}

impl Tally {
    pub fn is_empty(&self) -> bool {
        self.edited + self.added + self.removed == 0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// `teacher` | `claude`
    pub role: String,
    pub text: String,
    pub at: u64,
    /// On Claude's replies: what changed in the document.
    #[serde(default)]
    pub tally: Option<Tally>,
    /// A reply that could not be obtained or applied: shown, never sent back.
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub cost_usd: f64,
}

impl Message {
    pub fn teacher(text: &str) -> Self {
        Message {
            role: "teacher".into(),
            text: text.trim().to_string(),
            at: crate::workspace::now_ms(),
            tally: None,
            failed: false,
            cost_usd: 0.0,
        }
    }

    pub fn claude(text: &str, tally: Option<Tally>, cost_usd: f64) -> Self {
        Message {
            role: "claude".into(),
            text: text.trim().to_string(),
            at: crate::workspace::now_ms(),
            tally,
            failed: false,
            cost_usd,
        }
    }

    pub fn failure(text: &str) -> Self {
        Message { failed: true, ..Message::claude(text, None, 0.0) }
    }
}

pub fn log(document_dir: &Path) -> Vec<Message> {
    fs::read_to_string(document_dir.join(LOG_FILE))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn append(document_dir: &Path, message: Message) -> Result<Vec<Message>, String> {
    let mut messages = log(document_dir);
    messages.push(message);
    let excess = messages.len().saturating_sub(KEPT_MESSAGES);
    messages.drain(..excess);
    let serialised = serde_json::to_string_pretty(&messages).map_err(|e| e.to_string())?;
    fs::write(document_dir.join(LOG_FILE), serialised)
        .map_err(|e| format!("Écriture de la conversation : {e}"))?;
    Ok(messages)
}

pub fn clear(document_dir: &Path) -> Result<(), String> {
    match fs::remove_file(document_dir.join(LOG_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Effacement de la conversation : {error}")),
    }
}

// ---------------------------------------------------------------------------
// What the model is told, and what it answers
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT: &str = r#"You edit a French maths course inside Plume. The document is a sequence of typed passages ("blocks"); Plume renders each one through the teacher's LaTeX house style (the "charte"). The teacher talks to you about the whole document, and you answer with a short reply and the list of changes to make to its passages.

How to answer:
- `reply`: in French, addressed to the teacher as « vous », one to four sentences. Say what you changed, or answer their question. If the request is ambiguous or cannot be done, say so, ask what they mean, and return no changes.
- `changes`: ONLY the passages that change. A passage you do not list stays exactly as it is. Never list a passage you leave unchanged, never rewrite the whole document to change part of it.
  - `replace`: `targets` lists the ids of the passages replaced; `blocks` is what takes their place, at the position of the first target. One target and one block edits a passage. Several targets and one block merges them. An empty `blocks` deletes the targets.
  - `insert_after` / `insert_before`: exactly one target; the new blocks go right after / right before it.
  - Use only ids given in the document, never invent one, and never name the same passage in two `replace` changes.

Every block you write follows the rules of the rest of the document:
- `latex` is body content only: no \begin/\end wrapper for the block itself, no \section, no preamble. The charte wraps each block according to its `kind`.
- Headings (`chapter`, `part`, `subpart`, `paragraph`) carry their text in `title` and their number in `number`, exactly as it should print: "3", "II", "a", "A". Their `latex` stays empty. Plume never numbers anything by itself, so renumbering headings means rewriting each heading's `number`.
- `title` on any other kind only when the passage has a title of its own; never repeat the environment's name (« Définition », « Exemple »).
- Maths: $...$ inline, \[...\] displayed, a continued calculation in an `aligned` environment. Lists are `enumerate` / `itemize` with \item, never numbered by hand.
- `audience`: ["teacher", "student"] for both copies, ["teacher"] for the teacher's copy only, ["student"] for the pupils' only. Leave it out to keep the passage's current audience.
- `align`: "left", "center", "right", or "default" for the charte's own placement. Leave it out to keep the current one.
- `hidden`: true sets a passage aside — it stays in the document but leaves every PDF; false brings it back. Leave it out to keep.
- Keep the teacher's wording and mathematics. Change only what the request asks for, and never add content they did not ask for.

Page layout:
- Ordinary passages carry no layout — no minipage, multicols, \vspace, \hfill, \newpage: spacing and columns belong to the charte.
- The one exception is a layout the teacher explicitly asks for: side by side, in columns, in a grid, in a table. Then replace the passages concerned with ONE block of kind `text` whose `latex` lays them out, for instance in rows of `minipage`:
  \noindent\begin{minipage}[t]{0.48\linewidth}
  ...
  \end{minipage}\hfill
  \begin{minipage}[t]{0.48\linewidth}
  ...
  \end{minipage}
  Put a blank line between two rows. Inside, each former passage keeps its own environment, written the way this charte writes it (listed below), with its title as the optional argument. Keep the widths of a row under \linewidth, and never draw a \rule.
- Diagrams are one tikzpicture; the charte already loads every TikZ library it needs, never load one."#;

/// How this charte writes each kind of passage, for the one case the model
/// has to write the wrapper itself: passages laid out side by side, in a single
/// block that the charte does not wrap.
fn wrappers(template: &Template) -> String {
    let mut lines = Vec::new();
    for kind in ir::BLOCK_KINDS {
        let Some(mapping) = template.blocks.get(*kind) else { continue };
        let form = match mapping.mode.as_str() {
            "environment" => format!(
                "\\begin{{{name}}}[title, optional] ... \\end{{{name}}}",
                name = mapping.name
            ),
            "numbered" => format!("\\{}{{number}}{{title}}", mapping.name),
            "command" => format!("\\{}{{title}}", mapping.name),
            "centered" => "\\begin{center} ... \\end{center}".to_string(),
            _ => "the body as it is, with no wrapper".to_string(),
        };
        lines.push(format!("- {kind}: {form}"));
    }
    lines.join("\n")
}

pub fn system_prompt(template: Option<&Template>, rules: &str, has_photos: bool) -> String {
    let mut system = SYSTEM_PROMPT.to_string();
    if let Some(template) = template {
        system.push_str("\n\nHow this charte writes each kind of passage:\n");
        system.push_str(&wrappers(template));
    }
    if has_photos {
        system.push_str(
            "\n\nThe document was read from photographs, at pages/01.jpg, pages/02.jpg… in \
             the working directory; each passage says which page it came from. Read one only \
             when the request needs the source, for instance to check a passage against the page.",
        );
    }
    if !rules.trim().is_empty() {
        system.push_str("\n\nThe teacher's own conventions (authoritative):\n");
        system.push_str(rules.trim());
    }
    system
}

/// The document as the model sees it: one passage per line, in reading order.
///
/// Only what a request can be about. Review state — confidence, a pending
/// note, whether the teacher has read it — is Plume's, and the model has no
/// business reading it or answering about it.
fn describe(transcript: &Transcript) -> String {
    transcript
        .pages
        .iter()
        .flat_map(|page| page.blocks.iter().map(move |block| (page.number, block)))
        .map(|(page, block)| {
            let mut line = serde_json::json!({
                "id": block.id,
                "page": page,
                "kind": block.kind,
                "number": block.number,
                "title": block.title,
                "latex": block.latex,
                "audience": if block.audience.is_empty() {
                    vec!["teacher".to_string(), "student".to_string()]
                } else {
                    block.audience.clone()
                },
                "align": block.align.clone().unwrap_or_else(|| "default".into()),
                "hidden": block.hidden,
            });
            if block.taught_end {
                line["lastTaught"] = serde_json::Value::Bool(true);
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn prompt(transcript: &Transcript, earlier: &[Message], request: &str) -> String {
    let mut prompt = String::new();

    let remembered: Vec<&Message> = earlier.iter().filter(|m| !m.failed).collect();
    let from = remembered.len().saturating_sub(REMEMBERED);
    if from < remembered.len() {
        prompt.push_str("Earlier in this conversation, most recent last:\n");
        for message in &remembered[from..] {
            let who = if message.role == "teacher" { "Teacher" } else { "You" };
            prompt.push_str(&format!("{who}: {}\n", message.text));
        }
        prompt.push_str(
            "The document below is its current state, after those changes and any the \
             teacher made by hand since.\n\n",
        );
    }

    prompt.push_str("The document, one passage per line, in reading order. `lastTaught` marks the last passage the class has covered.\n");
    prompt.push_str(&describe(transcript));
    prompt.push_str("\n\nThe teacher's request:\n");
    prompt.push_str(request.trim());
    prompt
}

pub fn schema() -> String {
    let kinds = ir::BLOCK_KINDS
        .iter()
        .map(|k| format!("\"{k}\""))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        r#"{{
  "type": "object",
  "required": ["reply", "changes"],
  "additionalProperties": false,
  "properties": {{
    "reply": {{ "type": "string" }},
    "changes": {{
      "type": "array",
      "items": {{
        "type": "object",
        "required": ["action", "targets", "blocks"],
        "additionalProperties": false,
        "properties": {{
          "action": {{ "type": "string", "enum": ["replace", "insert_after", "insert_before"] }},
          "targets": {{ "type": "array", "minItems": 1, "items": {{ "type": "string" }} }},
          "blocks": {{
            "type": "array",
            "items": {{
              "type": "object",
              "required": ["kind", "latex"],
              "additionalProperties": false,
              "properties": {{
                "kind": {{ "type": "string", "enum": [{kinds}] }},
                "title": {{ "type": ["string", "null"] }},
                "number": {{ "type": ["string", "null"] }},
                "latex": {{ "type": "string" }},
                "audience": {{
                  "type": "array",
                  "items": {{ "type": "string", "enum": ["teacher", "student"] }}
                }},
                "align": {{ "type": "string", "enum": ["left", "center", "right", "default"] }},
                "hidden": {{ "type": "boolean" }}
              }}
            }}
          }}
        }}
      }}
    }}
  }}
}}"#
    )
}

/// The model's answer, as the schema shapes it.
#[derive(Deserialize, Debug)]
pub struct Answer {
    pub reply: String,
    #[serde(default)]
    pub changes: Vec<Change>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Change {
    pub action: String,
    pub targets: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<Proposed>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Proposed {
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub latex: String,
    #[serde(default)]
    pub audience: Option<Vec<String>>,
    #[serde(default)]
    pub align: Option<String>,
    #[serde(default)]
    pub hidden: Option<bool>,
}

/// The heartbeat shown while Claude works. The stream carries the model's own
/// words, none of which belong on screen: only what it is doing.
pub fn activity(event: &serde_json::Value) -> Option<&'static str> {
    match event.get("type")?.as_str()? {
        "system" => Some("Ouvre le document…"),
        "assistant" => {
            let blocks = event.pointer("/message/content")?.as_array()?;
            let mut thinking = false;
            for block in blocks {
                match block.get("type").and_then(|v| v.as_str()) {
                    Some("tool_use") => {
                        return match block.get("name").and_then(|v| v.as_str()) {
                            Some("Read") => Some("Regarde une photo…"),
                            _ => Some("Rédige les modifications…"),
                        };
                    }
                    Some("text") | Some("thinking") => thinking = true,
                    _ => {}
                }
            }
            thinking.then_some("Réfléchit à votre demande…")
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Applying an answer
// ---------------------------------------------------------------------------

pub struct Applied {
    pub transcript: Transcript,
    pub tally: Tally,
    /// Ids, in the new transcript, of every passage the answer wrote.
    pub changed: Vec<String>,
}

struct Entry {
    page: usize,
    block: Block,
    changed: bool,
}

fn clean(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// What the model saw of a passage, and could have based a change on.
fn same_text(a: &Block, b: &Block) -> bool {
    a.kind == b.kind && a.title == b.title && a.number == b.number && a.latex == b.latex
}

fn check(proposed: &Proposed, change: usize) -> Result<(), String> {
    let at = format!("Modification {change}");
    if !ir::BLOCK_KINDS.contains(&proposed.kind.as_str()) {
        return Err(format!("{at} : « {} » n'est pas un type de passage.", proposed.kind));
    }
    let titled = proposed.title.as_deref().is_some_and(|t| !t.trim().is_empty());
    let bodied = !proposed.latex.trim().is_empty();
    if HEADINGS.contains(&proposed.kind.as_str()) {
        if !titled && !bodied {
            return Err(format!("{at} : un titre vide."));
        }
    } else if !bodied {
        return Err(format!("{at} : un passage vide."));
    }
    if let Some(audience) = &proposed.audience {
        if audience.iter().any(|who| who != "teacher" && who != "student") {
            return Err(format!("{at} : un public inconnu."));
        }
    }
    if let Some(align) = &proposed.align {
        if !["left", "center", "right", "default"].contains(&align.as_str()) {
            return Err(format!("{at} : un alignement inconnu."));
        }
    }
    Ok(())
}

/// A new block from what the model wrote.
///
/// `edits` is the passage it replaces one for one, if that is what the change
/// does. Such a passage keeps everything the model was not asked about — how
/// sure the reading was, whether the teacher has read it, a pending note —
/// because renumbering a heading says nothing about how well its handwriting
/// was made out. Anything else is new: nobody has read it yet.
fn written(proposed: Proposed, edits: Option<&Block>, inherits: Option<&Block>) -> Block {
    let mut block = match edits {
        Some(original) => original.clone(),
        None => Block {
            id: String::new(),
            kind: String::new(),
            title: None,
            number: None,
            latex: String::new(),
            confidence: 1.0,
            doubt: None,
            audience: inherits.map(|b| b.audience.clone()).unwrap_or_default(),
            align: None,
            note: None,
            taught_end: false,
            hidden: false,
            reviewed: false,
        },
    };
    block.kind = proposed.kind;
    block.title = clean(proposed.title);
    block.number = clean(proposed.number);
    let latex = proposed.latex.trim().to_string();
    block.latex = crate::recognizer::enumerate_hand_numbered(&latex).unwrap_or(latex);
    if let Some(audience) = proposed.audience.filter(|a| !a.is_empty()) {
        block.audience = audience;
    }
    if block.audience.is_empty() {
        block.audience = vec!["teacher".into(), "student".into()];
    }
    if let Some(align) = proposed.align {
        block.align = (align != "default").then_some(align);
    }
    if let Some(hidden) = proposed.hidden {
        block.hidden = hidden;
    }
    if HEADINGS.contains(&block.kind.as_str()) && block.title.is_none() {
        // A heading's text lives in its title; one written in the body would
        // print twice once the charte adds its own.
        block.title = clean(Some(std::mem::take(&mut block.latex)));
    }
    block
}

/// Whether two passages would print the same.
fn unchanged(a: &Block, b: &Block) -> bool {
    same_text(a, b) && a.audience == b.audience && a.align == b.align && a.hidden == b.hidden
}

/// Applies `changes` to `current`.
///
/// `asked_on` is the transcript the model was shown. The changes name its ids,
/// and every passage they name must still read the same in `current`: a
/// passage edited, or moved by an insertion, while Claude was working would
/// otherwise have the change land on the wrong text. One such passage refuses
/// the whole answer — half a restructuring is worse than none.
pub fn apply(
    current: &Transcript,
    asked_on: &Transcript,
    changes: Vec<Change>,
) -> Result<Applied, String> {
    let seen: HashMap<&str, &Block> = asked_on
        .pages
        .iter()
        .flat_map(|page| page.blocks.iter())
        .map(|block| (block.id.as_str(), block))
        .collect();

    let mut entries: Vec<Entry> = current
        .pages
        .iter()
        .flat_map(|page| {
            page.blocks.iter().map(move |block| Entry {
                page: page.number,
                block: block.clone(),
                changed: false,
            })
        })
        .collect();

    // Everything is checked before anything moves.
    let mut replaced: HashSet<&str> = HashSet::new();
    for (index, change) in changes.iter().enumerate() {
        let number = index + 1;
        match change.action.as_str() {
            "replace" => {}
            "insert_after" | "insert_before" => {
                if change.targets.len() != 1 {
                    return Err(format!("Modification {number} : une insertion vise un seul passage."));
                }
                if change.blocks.is_empty() {
                    return Err(format!("Modification {number} : rien à insérer."));
                }
            }
            other => return Err(format!("Modification {number} : action inconnue « {other} ».")),
        }
        if change.targets.is_empty() {
            return Err(format!("Modification {number} : elle ne vise aucun passage."));
        }
        for target in &change.targets {
            let Some(was) = seen.get(target.as_str()) else {
                return Err(format!("Claude a visé un passage qui n'existe pas ({target})."));
            };
            let still = entries.iter().any(|e| e.block.id == *target && same_text(&e.block, was));
            if !still {
                return Err(
                    "Le document a changé pendant que Claude travaillait : rien n'a été appliqué. \
                     Renvoyez votre demande."
                        .into(),
                );
            }
            if change.action == "replace" && !replaced.insert(target.as_str()) {
                return Err(format!("Claude a modifié deux fois le même passage ({target})."));
            }
        }
        for proposed in &change.blocks {
            check(proposed, number)?;
        }
    }

    let position = |entries: &[Entry], id: &str| entries.iter().position(|e| e.block.id == id);
    let mut tally = Tally::default();

    // Insertions first, while every anchor is still where the model saw it: an
    // answer may well replace a passage and insert after it in the same breath.
    let mut after: HashMap<String, usize> = HashMap::new();
    for change in changes.iter().filter(|c| c.action != "replace") {
        let target = &change.targets[0];
        let at = position(&entries, target).expect("checked above");
        let page = entries[at].page;
        let mut at = if change.action == "insert_after" {
            // Two insertions after the same passage keep the order they came in.
            let count = after.entry(target.clone()).or_insert(0);
            at + 1 + *count
        } else {
            at
        };
        for proposed in change.blocks.clone() {
            let block = written(proposed, None, None);
            if change.action == "insert_after" {
                *after.get_mut(target).expect("inserted above") += 1;
            }
            entries.insert(at, Entry { page, block, changed: true });
            at += 1;
            tally.added += 1;
        }
    }

    for change in changes.into_iter().filter(|c| c.action == "replace") {
        let mut places: Vec<usize> = change
            .targets
            .iter()
            .map(|target| position(&entries, target).expect("checked above"))
            .collect();
        places.sort_unstable();
        let first = places[0];
        let page = entries[first].page;
        let originals: Vec<Block> = places.iter().map(|&at| entries[at].block.clone()).collect();
        let carried = originals.iter().any(|b| b.taught_end);

        for &at in places.iter().rev() {
            entries.remove(at);
        }

        let one_for_one = originals.len() == 1 && change.blocks.len() == 1;
        let fresh: Vec<Entry> = change
            .blocks
            .into_iter()
            .map(|proposed| {
                let edits = one_for_one.then(|| &originals[0]);
                let mut block = written(proposed, edits, originals.first());
                block.taught_end = false;
                let changed = !edits.is_some_and(|original| unchanged(original, &block));
                Entry { page, block, changed }
            })
            .collect();

        let kept = fresh.len();
        let edited = originals.len().min(kept);
        tally.edited += if one_for_one { usize::from(fresh[0].changed) } else { edited };
        tally.removed += originals.len().saturating_sub(kept);
        tally.added += kept.saturating_sub(originals.len());

        for (offset, entry) in fresh.into_iter().enumerate() {
            entries.insert(first + offset, entry);
        }

        // The lesson still ended where it ended. Merged or split, the passage
        // the class stopped on is covered by what replaced it, so the mark goes
        // to the last of those; deleted, it steps back, as a deletion by hand
        // does.
        if carried {
            if kept > 0 {
                entries[first + kept - 1].block.taught_end = true;
            } else if first > 0 {
                entries[first - 1].block.taught_end = true;
            }
        }
    }

    // At most one mark: the earlier one, the side that sends the class less.
    let mut marked = false;
    for entry in &mut entries {
        if entry.block.taught_end {
            entry.block.taught_end = !marked;
            marked = true;
        }
    }

    let mut transcript = Transcript { version: current.version, pages: Vec::new() };
    let mut changed = Vec::new();
    for page in &current.pages {
        let mut blocks = Vec::new();
        for entry in entries.iter().filter(|e| e.page == page.number) {
            let mut block = entry.block.clone();
            block.id = format!("p{:02}-b{:02}", page.number, blocks.len() + 1);
            if entry.changed {
                changed.push(block.id.clone());
            }
            blocks.push(block);
        }
        transcript.pages.push(ir::Page {
            number: page.number,
            blocks,
            session_id: page.session_id.clone(),
        });
    }

    Ok(Applied { transcript, tally, changed })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(id: &str, kind: &str, number: Option<&str>, title: Option<&str>, latex: &str) -> Block {
        Block {
            id: id.into(),
            kind: kind.into(),
            title: title.map(str::to_string),
            number: number.map(str::to_string),
            latex: latex.into(),
            confidence: 0.7,
            doubt: Some("un symbole".into()),
            audience: vec!["teacher".into(), "student".into()],
            align: None,
            note: None,
            taught_end: false,
            hidden: false,
            reviewed: false,
        }
    }

    fn document() -> Transcript {
        Transcript {
            version: 1,
            pages: vec![
                ir::Page {
                    number: 1,
                    session_id: Some("s1".into()),
                    blocks: vec![
                        block("p01-b01", "part", Some("I"), Some("Vecteurs"), ""),
                        block("p01-b02", "definition", None, None, "Un vecteur."),
                        block("p01-b03", "definition", None, None, "Deux vecteurs égaux."),
                    ],
                },
                ir::Page {
                    number: 2,
                    session_id: None,
                    blocks: vec![
                        block("p02-b01", "part", Some("II"), Some("Somme"), ""),
                        block("p02-b02", "example", None, None, "Un exemple."),
                    ],
                },
            ],
        }
    }

    fn proposed(kind: &str, latex: &str) -> Proposed {
        Proposed {
            kind: kind.into(),
            title: None,
            number: None,
            latex: latex.into(),
            audience: None,
            align: None,
            hidden: None,
        }
    }

    fn heading(number: &str, title: &str) -> Proposed {
        Proposed {
            number: Some(number.into()),
            title: Some(title.into()),
            ..proposed("part", "")
        }
    }

    fn replace(targets: &[&str], blocks: Vec<Proposed>) -> Change {
        Change {
            action: "replace".into(),
            targets: targets.iter().map(|t| t.to_string()).collect(),
            blocks,
        }
    }

    fn ids(transcript: &Transcript) -> Vec<String> {
        transcript.pages.iter().flat_map(|p| p.blocks.iter().map(|b| b.id.clone())).collect()
    }

    #[test]
    fn renumbering_headings_touches_only_their_number() {
        let doc = document();
        let applied = apply(
            &doc,
            &doc,
            vec![
                replace(&["p01-b01"], vec![heading("A", "Vecteurs")]),
                replace(&["p02-b01"], vec![heading("B", "Somme")]),
            ],
        )
        .unwrap();

        let first = &applied.transcript.pages[0].blocks[0];
        assert_eq!(first.number.as_deref(), Some("A"));
        // Edited, not rewritten: what the review knew about it stays.
        assert_eq!(first.confidence, 0.7);
        assert_eq!(first.doubt.as_deref(), Some("un symbole"));
        assert_eq!(applied.tally, Tally { edited: 2, added: 0, removed: 0 });
        assert_eq!(applied.changed, vec!["p01-b01", "p02-b01"]);
        assert_eq!(applied.transcript.pages[0].session_id.as_deref(), Some("s1"));
    }

    #[test]
    fn a_grid_replaces_its_passages_where_the_first_one_was() {
        let doc = document();
        let grid = "\\begin{minipage}{0.48\\linewidth}\\begin{definition}Un vecteur.\\end{definition}\\end{minipage}";
        let applied =
            apply(&doc, &doc, vec![replace(&["p01-b02", "p01-b03"], vec![proposed("text", grid)])])
                .unwrap();

        let page = &applied.transcript.pages[0];
        assert_eq!(page.blocks.len(), 2);
        assert_eq!(page.blocks[1].id, "p01-b02");
        assert_eq!(page.blocks[1].latex, grid);
        // New text: nobody has read it yet.
        assert!(!page.blocks[1].reviewed);
        assert_eq!(page.blocks[1].confidence, 1.0);
        assert_eq!(applied.tally, Tally { edited: 1, added: 0, removed: 1 });
    }

    #[test]
    fn a_passage_left_out_of_the_answer_is_untouched() {
        let doc = document();
        let applied =
            apply(&doc, &doc, vec![replace(&["p02-b02"], vec![proposed("example", "Autre.")])])
                .unwrap();
        let untouched = &applied.transcript.pages[0].blocks[1];
        assert_eq!(untouched.latex, "Un vecteur.");
        assert_eq!(applied.changed, vec!["p02-b02"]);
    }

    #[test]
    fn an_insertion_after_a_replaced_passage_lands_after_its_replacement() {
        let doc = document();
        let applied = apply(
            &doc,
            &doc,
            vec![
                replace(&["p01-b02"], vec![proposed("definition", "Un vecteur, reformulé.")]),
                Change {
                    action: "insert_after".into(),
                    targets: vec!["p01-b02".into()],
                    blocks: vec![proposed("remark", "Première."), proposed("remark", "Seconde.")],
                },
            ],
        )
        .unwrap();
        let latex: Vec<&str> =
            applied.transcript.pages[0].blocks.iter().map(|b| b.latex.as_str()).collect();
        assert_eq!(
            latex,
            vec!["", "Un vecteur, reformulé.", "Première.", "Seconde.", "Deux vecteurs égaux."]
        );
        assert_eq!(ids(&applied.transcript)[..5], ["p01-b01", "p01-b02", "p01-b03", "p01-b04", "p01-b05"]);
        assert_eq!(applied.tally, Tally { edited: 1, added: 2, removed: 0 });
    }

    #[test]
    fn the_mark_follows_a_merge_and_steps_back_over_a_deletion() {
        let mut doc = document();
        doc.pages[0].blocks[2].taught_end = true;

        let merged =
            apply(&doc, &doc, vec![replace(&["p01-b02", "p01-b03"], vec![proposed("text", "Grille.")])])
                .unwrap();
        assert!(merged.transcript.pages[0].blocks[1].taught_end);

        let deleted = apply(&doc, &doc, vec![replace(&["p01-b03"], vec![])]).unwrap();
        assert!(deleted.transcript.pages[0].blocks[1].taught_end);
        assert_eq!(deleted.tally, Tally { edited: 0, added: 0, removed: 1 });
    }

    #[test]
    fn what_the_review_decided_survives_an_edit() {
        let mut doc = document();
        doc.pages[0].blocks[1].hidden = true;
        doc.pages[0].blocks[1].align = Some("center".into());
        doc.pages[0].blocks[1].note = Some("refaire le schéma".into());

        let applied =
            apply(&doc, &doc, vec![replace(&["p01-b02"], vec![proposed("definition", "Autre.")])])
                .unwrap();
        let edited = &applied.transcript.pages[0].blocks[1];
        assert!(edited.hidden);
        assert_eq!(edited.align.as_deref(), Some("center"));
        assert_eq!(edited.note.as_deref(), Some("refaire le schéma"));
    }

    #[test]
    fn the_model_can_set_aside_and_realign_when_asked() {
        let doc = document();
        let mut aside = proposed("example", "Un exemple.");
        aside.hidden = Some(true);
        aside.audience = Some(vec!["teacher".into()]);
        aside.align = Some("right".into());
        let applied = apply(&doc, &doc, vec![replace(&["p02-b02"], vec![aside])]).unwrap();
        let block = &applied.transcript.pages[1].blocks[1];
        assert!(block.hidden);
        assert_eq!(block.audience, vec!["teacher"]);
        assert_eq!(block.align.as_deref(), Some("right"));
    }

    #[test]
    fn an_edit_that_changes_nothing_is_not_counted() {
        let doc = document();
        let applied = apply(
            &doc,
            &doc,
            vec![replace(&["p01-b02"], vec![proposed("definition", "Un vecteur.")])],
        )
        .unwrap();
        assert!(applied.tally.is_empty());
        assert!(applied.changed.is_empty());
    }

    #[test]
    fn an_unknown_passage_refuses_the_whole_answer() {
        let doc = document();
        let error = apply(
            &doc,
            &doc,
            vec![
                replace(&["p01-b02"], vec![proposed("definition", "Autre.")]),
                replace(&["p09-b01"], vec![]),
            ],
        )
        .err()
        .unwrap();
        assert!(error.contains("p09-b01"));
    }

    #[test]
    fn a_passage_edited_meanwhile_refuses_the_answer() {
        let asked_on = document();
        let mut current = document();
        current.pages[0].blocks[1].latex = "Corrigé à la main.".into();
        let error = apply(
            &current,
            &asked_on,
            vec![replace(&["p01-b02"], vec![proposed("definition", "Autre.")])],
        )
        .err()
        .unwrap();
        assert!(error.contains("a changé"));
    }

    #[test]
    fn the_same_passage_cannot_be_replaced_twice() {
        let doc = document();
        assert!(apply(
            &doc,
            &doc,
            vec![replace(&["p01-b02"], vec![]), replace(&["p01-b02", "p01-b03"], vec![])],
        )
        .is_err());
    }

    #[test]
    fn an_empty_passage_is_refused() {
        let doc = document();
        assert!(apply(&doc, &doc, vec![replace(&["p01-b02"], vec![proposed("definition", "  ")])])
            .is_err());
    }

    #[test]
    fn a_heading_written_in_the_body_moves_to_its_title() {
        let doc = document();
        let applied =
            apply(&doc, &doc, vec![replace(&["p01-b01"], vec![proposed("part", "Vecteurs")])])
                .unwrap();
        let heading = &applied.transcript.pages[0].blocks[0];
        assert_eq!(heading.title.as_deref(), Some("Vecteurs"));
        assert!(heading.latex.is_empty());
    }

    #[test]
    fn the_prompt_carries_the_document_and_recent_turns_only() {
        let doc = document();
        let mut earlier: Vec<Message> =
            (0..10).map(|n| Message::teacher(&format!("demande {n}"))).collect();
        earlier.push(Message::failure("échec à ne pas renvoyer"));
        let text = prompt(&doc, &earlier, "Mets les définitions en grille");
        assert!(text.contains("demande 9"));
        assert!(!text.contains("demande 3"));
        assert!(!text.contains("échec"));
        assert!(text.contains("\"id\":\"p02-b02\""));
        assert!(text.ends_with("Mets les définitions en grille"));
    }

    #[test]
    fn the_schema_is_valid_json() {
        let schema: serde_json::Value = serde_json::from_str(&schema()).unwrap();
        assert_eq!(schema["required"][1], "changes");
    }

    #[test]
    fn the_log_keeps_the_most_recent_messages() {
        let dir = std::env::temp_dir().join(format!("plume-chat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for n in 0..(KEPT_MESSAGES + 5) {
            append(&dir, Message::teacher(&n.to_string())).unwrap();
        }
        let messages = log(&dir);
        assert_eq!(messages.len(), KEPT_MESSAGES);
        assert_eq!(messages[0].text, "5");
        clear(&dir).unwrap();
        assert!(log(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
