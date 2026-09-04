//! The semantic intermediate representation.
//!
//! Recognition never emits LaTeX directly. It emits typed blocks, and a
//! deterministic renderer turns those into the user's house style. That
//! indirection is what makes the rest possible:
//!
//!  - swapping the template re-renders without calling the model again;
//!  - teacher/student variants are a filter on `audience`, not two documents;
//!  - a review action names one block by `id`, not by matching its text;
//!  - low-confidence blocks are a field, not a guess made later.

use serde::{Deserialize, Serialize};

/// Block kinds the recogniser may emit. These map onto template environments in
/// `render.rs`; adding one here means adding a mapping there.
pub const BLOCK_KINDS: &[&str] = &[
    "chapter",
    "part",
    "subpart",
    "paragraph",
    "text",
    "list",
    "equation",
    "definition",
    "property",
    "theorem",
    "method",
    "example",
    "application",
    "remark",
    "proof",
    "figure",
];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// Where the block sits: `p03-b07` is page 3, seventh block.
    ///
    /// Assigned by Plume, never by the model — the schema sent to the CLI does
    /// not contain it, so it must default when parsing the model's output.
    ///
    /// It encodes position, so it names a passage only until that page is next
    /// edited: inserting, splitting, deleting or reordering renumbers every id
    /// on the page. Good enough to anchor one review action; not something to
    /// store elsewhere and read back later. Anything that must outlive an edit
    /// belongs in a field on the block — see `taught_end`.
    #[serde(default)]
    pub id: String,
    pub kind: String,
    /// Optional environment title: `\begin{definition}[title]`.
    #[serde(default)]
    pub title: Option<String>,
    /// For a heading: the number written on the page — "3", "II", "1", "a".
    ///
    /// Plume does not number anything itself. A course photographed from the
    /// middle of a notebook opens on "Chapitre 3", and renumbering it to 1
    /// would contradict every other document the class holds. Empty when the
    /// page shows no number, in which case none is invented.
    #[serde(default)]
    pub number: Option<String>,
    /// LaTeX body — no `\begin{...}`, the renderer wraps it.
    pub latex: String,
    /// 0.0 to 1.0. Anything below `DOUBT_THRESHOLD` is surfaced for review.
    pub confidence: f32,
    /// Why the model was unsure, in French: shown as-is next to the block.
    #[serde(default)]
    pub doubt: Option<String>,
    /// Which exports keep this block. Empty means every audience.
    #[serde(default)]
    pub audience: Vec<String>,
    /// `left` | `center` | `right`, or absent for the template's own choice.
    ///
    /// Set during review, never by the model: it is a decision about the page,
    /// not about what the page says. Kept out of the schema for that reason.
    #[serde(default)]
    pub align: Option<String>,
    /// The teacher's pending instruction for this block, in their words. Set
    /// during review, consumed by a targeted re-run, then cleared.
    #[serde(default)]
    pub note: Option<String>,
    /// True on the last passage the class has actually covered.
    ///
    /// The mark rides on the block rather than sitting in the manifest as an
    /// id, because ids encode position — `p03-b07` — and every insertion,
    /// split, deletion or reordering renumbers the page. An id kept elsewhere
    /// would quietly come to mean a different passage, and a handout meant to
    /// stop at II.3 would run on into next week's lesson without a word. A
    /// field travels with the passage through all of it.
    ///
    /// At most one block carries it; `mark_taught_end` enforces that. Set
    /// during review, never by the model: how far a class has got is not
    /// something a page can be read for.
    #[serde(default)]
    pub taught_end: bool,
    /// True once a human has read or edited this block.
    #[serde(default)]
    pub reviewed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// 1-based, matching `pages/01.jpg`.
    pub number: usize,
    pub blocks: Vec<Block>,
    /// Claude Code session that read this page. Reusing it with `--resume` lets
    /// a correction reuse the already-loaded image and context instead of
    /// paying for the whole page again.
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub version: u32,
    pub pages: Vec<Page>,
}

/// Below this, the UI flags the block as needing a look.
pub const DOUBT_THRESHOLD: f32 = 0.85;

/// The passage the class stopped on, when the course has been marked.
pub fn taught_end(transcript: &Transcript) -> Option<&Block> {
    transcript
        .pages
        .iter()
        .flat_map(|page| page.blocks.iter())
        .find(|block| block.taught_end)
}

/// How many passages the class has covered, counting the marked one.
pub fn taught_count(transcript: &Transcript) -> Option<usize> {
    let mut seen = 0;
    for block in transcript.pages.iter().flat_map(|page| page.blocks.iter()) {
        seen += 1;
        if block.taught_end {
            return Some(seen);
        }
    }
    None
}

/// Moves the mark onto one passage, or clears it entirely with `None`.
///
/// Every block is rewritten rather than just the two that change, because the
/// invariant is what matters: exactly one mark, or none. A course carrying two
/// boundaries has no boundary at all, and the export would honour whichever
/// came first — silently, and differently after every reordering.
pub fn mark_taught_end(
    transcript: &mut Transcript,
    block_id: Option<&str>,
) -> Result<(), String> {
    if let Some(id) = block_id {
        let exists = transcript
            .pages
            .iter()
            .flat_map(|page| page.blocks.iter())
            .any(|block| block.id == id);
        if !exists {
            return Err("Bloc introuvable.".into());
        }
    }

    for block in transcript.pages.iter_mut().flat_map(|page| page.blocks.iter_mut()) {
        block.taught_end = block_id == Some(block.id.as_str());
    }
    Ok(())
}

/// Schema for a single corrected block, used by targeted re-runs.
pub fn block_schema() -> String {
    let kinds = BLOCK_KINDS
        .iter()
        .map(|k| format!("\"{k}\""))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        r#"{{
  "type": "object",
  "required": ["kind", "latex", "confidence"],
  "additionalProperties": false,
  "properties": {{
    "kind": {{ "type": "string", "enum": [{kinds}] }},
    "title": {{ "type": ["string", "null"] }},
    "number": {{ "type": ["string", "null"] }},
    "latex": {{ "type": "string" }},
    "confidence": {{ "type": "number", "minimum": 0, "maximum": 1 }},
    "doubt": {{ "type": ["string", "null"] }},
    "audience": {{
      "type": "array",
      "items": {{ "type": "string", "enum": ["teacher", "student"] }}
    }}
  }}
}}"#
    )
}

/// Schema handed to `claude -p --json-schema`, so the CLI validates the shape
/// and retries on mismatch instead of us parsing loose prose.
pub fn page_schema() -> String {
    let kinds = BLOCK_KINDS
        .iter()
        .map(|k| format!("\"{k}\""))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        r#"{{
  "type": "object",
  "required": ["blocks"],
  "additionalProperties": false,
  "properties": {{
    "blocks": {{
      "type": "array",
      "items": {{
        "type": "object",
        "required": ["kind", "latex", "confidence"],
        "additionalProperties": false,
        "properties": {{
          "kind": {{ "type": "string", "enum": [{kinds}] }},
          "title": {{ "type": ["string", "null"] }},
          "number": {{ "type": ["string", "null"] }},
          "latex": {{ "type": "string" }},
          "confidence": {{ "type": "number", "minimum": 0, "maximum": 1 }},
          "doubt": {{ "type": ["string", "null"] }},
          "audience": {{
            "type": "array",
            "items": {{ "type": "string", "enum": ["teacher", "student"] }}
          }}
        }}
      }}
    }}
  }}
}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exactly the shape `--json-schema` produces: no `id`, since Plume assigns
    /// it afterwards. This is the regression that silently cost a full run.
    #[test]
    fn parses_a_block_without_an_id() {
        let raw = r#"{
            "kind": "definition",
            "title": null,
            "number": null,
            "latex": "Un vecteur $\\overrightarrow{AB}$.",
            "confidence": 0.94,
            "doubt": null
        }"#;

        let block: Block = serde_json::from_str(raw).expect("block must parse");
        assert_eq!(block.kind, "definition");
        assert!(block.id.is_empty(), "id is filled in by the recogniser");
        assert!(block.audience.is_empty(), "audience defaults before widening");
    }

    /// Every kind the schema advertises must round-trip, otherwise the model can
    /// legitimately emit something we then drop.
    #[test]
    fn parses_every_advertised_kind() {
        for kind in BLOCK_KINDS {
            let raw = format!(
                r#"{{"kind":"{kind}","latex":"x","confidence":1.0,"audience":["teacher"]}}"#
            );
            let block: Block =
                serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{kind}: {e}"));
            assert_eq!(block.audience, vec!["teacher".to_string()]);
        }
    }

    #[test]
    fn schema_is_valid_json_and_lists_every_kind() {
        let schema: serde_json::Value =
            serde_json::from_str(&page_schema()).expect("schema must be valid JSON");
        let kinds = schema["properties"]["blocks"]["items"]["properties"]["kind"]["enum"]
            .as_array()
            .expect("kind enum");
        assert_eq!(kinds.len(), BLOCK_KINDS.len());
    }
}
