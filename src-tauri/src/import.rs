//! Reading a document written somewhere other than on paper.
//!
//! Plume's whole design already turns on one idea: recognition emits typed
//! blocks, and a deterministic renderer turns those into the teacher's house
//! style. Nothing in that arrangement cares where the blocks came from. A
//! model asked for an exercise sheet can emit the same shape a photograph
//! produces, and everything downstream — review, split, audience, the taught
//! boundary, the charte, the PDF — works on it unchanged.
//!
//! So this module is not a second pipeline. It is a second door onto the
//! first one, and its whole job is to refuse anything that would not have come
//! out of the recogniser.
//!
//! **Why the refusals are loud.** The JSON arrives from outside — pasted from
//! a conversation, saved from who knows where — and a document quietly missing
//! half its exercises is worse than one that would not import. Every message
//! names the passage it is about, because "invalid JSON" tells a teacher
//! nothing they can act on.

use crate::ir::{Block, Page, Transcript, BLOCK_KINDS};
use serde::{Deserialize, Serialize};

/// Kinds that carry their content in a title rather than a body.
const HEADINGS: &[&str] = &["chapter", "part", "subpart", "paragraph"];

/// Audiences a block may be restricted to.
const AUDIENCES: &[&str] = &["teacher", "student"];

/// A document read from JSON and ready to become one.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    /// The title the file proposes. Empty when it names none — the teacher is
    /// asked for one rather than having "Sans titre" chosen for them.
    pub title: String,
    /// What kind of document it is — « exercices », « DS » — as the file
    /// proposes. The teacher has the last word on the screen.
    pub tags: Vec<String>,
    pub blocks: Vec<Block>,
}

/// The shape accepted on the wire, kept separate from `Block` on purpose.
///
/// `Block` carries fields that belong to Plume and not to whoever wrote the
/// file — the id, how sure a reading was, whether a human has looked. Letting
/// the file set those would mean a document could arrive pre-marked as reviewed,
/// or claim a boundary the teacher never placed.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    #[serde(default)]
    title: String,
    #[serde(default)]
    tags: Vec<String>,
    blocks: Vec<WireBlock>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireBlock {
    kind: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    number: Option<String>,
    #[serde(default)]
    latex: String,
    #[serde(default)]
    audience: Vec<String>,
}

/// Reads a document from JSON, or says exactly what is wrong with it.
pub fn parse(json: &str) -> Result<Import, String> {
    let json = json.trim();
    if json.is_empty() {
        return Err("Collez le JSON du document, ou choisissez un fichier.".into());
    }

    let wire: Wire = serde_json::from_str(json).map_err(|error| {
        format!(
            "Ce n'est pas le format attendu (ligne {}) : {}. Le fichier doit être un objet JSON \
             avec une liste « blocks ».",
            error.line(),
            error
        )
    })?;

    if wire.blocks.is_empty() {
        return Err("Ce document ne contient aucun passage.".into());
    }

    let mut blocks = Vec::with_capacity(wire.blocks.len());
    for (index, raw) in wire.blocks.into_iter().enumerate() {
        blocks.push(convert(raw, index + 1)?);
    }

    Ok(Import {
        title: wire.title.trim().to_string(),
        tags: crate::workspace::normalise_tags(&wire.tags),
        blocks,
    })
}

/// One wire block into one IR block, or the reason it cannot be.
fn convert(raw: WireBlock, position: usize) -> Result<Block, String> {
    let kind = raw.kind.trim().to_lowercase();
    if !BLOCK_KINDS.contains(&kind.as_str()) {
        return Err(format!(
            "Passage {position} : « {} » n'est pas un type de passage. Types acceptés : {}.",
            raw.kind,
            BLOCK_KINDS.join(", ")
        ));
    }

    let title = raw.title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let latex = raw.latex.trim().to_string();

    // A heading is its title; everything else is its body. A block with
    // neither renders as an empty box the teacher then has to hunt down —
    // exactly the "Exercices : null" that had to be chased out of readings.
    let empty = if HEADINGS.contains(&kind.as_str()) {
        title.is_none() && latex.is_empty()
    } else {
        latex.is_empty()
    };
    if empty {
        return Err(format!(
            "Passage {position} ({kind}) : il est vide. Un titre ne suffit que pour un titre de \
             partie ; les autres passages ont besoin de « latex »."
        ));
    }

    for audience in &raw.audience {
        if !AUDIENCES.contains(&audience.as_str()) {
            return Err(format!(
                "Passage {position} : « {audience} » n'est pas un public. Utilisez « teacher », \
                 « student », ou laissez la liste vide pour les deux."
            ));
        }
    }

    Ok(Block {
        // Assigned when the transcript is laid out, never taken from the file.
        id: String::new(),
        kind,
        title,
        number: raw.number.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
        latex,
        // Nothing was read, so there is nothing to be unsure about. Confidence
        // measures how well handwriting was made out; it does not measure
        // whether the mathematics is right, and pretending otherwise would put
        // a green tick on a sheet nobody has checked. `reviewed` stays false:
        // unread is exactly what this is.
        confidence: 1.0,
        doubt: None,
        audience: raw.audience,
        align: None,
        note: None,
        taught_end: false,
        reviewed: false,
    })
}

/// Lays the blocks out as a transcript.
///
/// One page, because pages exist to match photographs and there are none. Ids
/// still encode position, so everything that edits a transcript — inserting,
/// splitting, reordering — works here exactly as it does on a reading.
pub fn transcript_of(blocks: Vec<Block>) -> Transcript {
    let mut blocks = blocks;
    for (index, block) in blocks.iter_mut().enumerate() {
        block.id = format!("p01-b{:02}", index + 1);
    }
    Transcript { version: 1, pages: vec![Page { number: 1, blocks, session_id: None }] }
}

/// Reads a document from JSON and writes it into the workbook.
///
/// Shared by the interface and the MCP server rather than written twice: the
/// interesting part is not the happy path but the rollback, and a document folder
/// that exists with no passages in it shows up in the list as something to open
/// with nothing inside.
///
/// The text is parsed here rather than taken as blocks, so that what is written
/// can only ever be something `parse` accepted.
pub fn create(
    json: &str,
    title: &str,
    template_id: &str,
    tags: &[String],
) -> Result<crate::workspace::Document, String> {
    let import = parse(json)?;
    let title = if title.trim().is_empty() { import.title.as_str() } else { title };
    // The screen's choice wins over the file's proposal; an empty choice means
    // the teacher left the proposal alone.
    let tags = if tags.iter().all(|t| t.trim().is_empty()) { &import.tags } else { tags };

    let document = crate::workspace::create_written(title, template_id, tags)?;
    let transcript = transcript_of(import.blocks);

    let written = serde_json::to_string_pretty(&transcript)
        .map_err(|e| e.to_string())
        .and_then(|raw| {
            std::fs::write(
                crate::workspace::document_dir(&document.id).join("transcript.json"),
                raw,
            )
            .map_err(|e| format!("Écriture de la transcription : {e}"))
        });

    if let Err(error) = written {
        // Removed rather than binned, as a failed creation is: the teacher
        // never had this document, so there is nothing to restore.
        let _ = std::fs::remove_dir_all(crate::workspace::document_dir(&document.id));
        return Err(error);
    }

    crate::logbus::info(
        "workspace",
        format!(
            "Document « {} » importé — {} passage(s)",
            document.title,
            transcript.pages.iter().map(|p| p.blocks.len()).sum::<usize>()
        ),
    );
    Ok(document)
}

/// The same contract as a JSON Schema, for the callers that validate rather
/// than read — the MCP server hands it to the model as a tool signature.
pub fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "array",
        "minItems": 1,
        "description": "Les passages du document, dans l'ordre de lecture.",
        "items": {
            "type": "object",
            "required": ["kind"],
            "additionalProperties": false,
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": BLOCK_KINDS,
                    "description": "Le type de passage."
                },
                "title": {
                    "type": "string",
                    "description": "Titre facultatif. Pour un titre de partie, c'est son texte."
                },
                "number": {
                    "type": "string",
                    "description": "Numéro écrit d'un titre de partie : « 3 », « II », « a »."
                },
                "latex": {
                    "type": "string",
                    "description": "Le CONTENU du passage : ni préambule, ni \\begin{document}, \\
ni \\section, ni \\begin{definition}. La charte décide de tout cela."
                },
                "audience": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["teacher", "student"] },
                    "description": "Vide (les deux) par défaut. [\"teacher\"] pour ce qui ne doit \\
pas partir aux élèves."
                }
            }
        }
    })
}

/// What to hand a model so that what comes back will import.
///
/// Written as instructions rather than as a bare schema because that is how it
/// gets used: copied into a conversation, above a request for an exercise
/// sheet. The schema alone leaves the interesting half unsaid — that the body
/// is content and never a document, and that nobody but the teacher decides
/// what the class does not get to see.
pub fn instructions() -> String {
    format!(
        r#"Tu écris un document pour Plume. Réponds UNIQUEMENT par un objet JSON de cette forme,
sans texte autour et sans bloc de code :

{{
  "title": "Fiche d'exercices — Vecteurs",
  "tags": ["exercices"],
  "blocks": [
    {{ "kind": "part", "number": "I", "title": "Colinéarité" }},
    {{ "kind": "application", "title": "Exercice 1",
       "latex": "Montrer que $\\vec{{u}}(2;3)$ et $\\vec{{v}}(4;6)$ sont colinéaires." }},
    {{ "kind": "proof", "latex": "On calcule $2\\times 6-3\\times 4=0$.",
       "audience": ["teacher"] }}
  ]
}}

Règles :

1. « kind » est l'un de : {kinds}.
2. « latex » est le CONTENU du passage, jamais un document : pas de préambule,
   pas de \begin{{document}}, pas de \section, pas de \begin{{definition}} — la
   charte du professeur décide de tout cela. Écris les mathématiques en LaTeX
   ($x^2$, \frac, \vec), le reste en texte ordinaire.
3. N'invente aucune mise en page : ni minipage, ni multicols, ni tabular pour
   placer deux choses côte à côte, ni \vspace, ni \newpage.
4. Les titres de partie (chapter, part, subpart, paragraph) portent leur texte
   dans « title », et leur numéro écrit dans « number » — « 3 », « II », « a ».
   Les autres passages portent leur contenu dans « latex ».
5. « audience » est optionnel et vaut les deux publics par défaut. Ne mets
   ["teacher"] que sur ce qui ne doit pas partir aux élèves — une correction,
   une réponse. Dans le doute, laisse vide : c'est au professeur de trancher.
6. Une énumération est un \begin{{enumerate}} ou un \begin{{itemize}}, jamais
   des numéros écrits à la main en début de ligne.
7. « tags » dit ce qu'est le document — « cours », « exercices », « DS »,
   « DM », « interrogation »… — et le professeur trie son classeur avec.
   Reprends l'orthographe d'une étiquette qu'il utilise déjà quand elle
   convient ; sinon, propose-en une.

N'ajoute aucun autre champ : l'identifiant, la confiance et l'état de relecture
appartiennent à Plume."#,
        kinds = BLOCK_KINDS.join(", ")
    )
}

/// Blocks worth flagging before the document is created.
///
/// Not refusals — the passage is the teacher's to keep — but the two things a
/// generated sheet gets wrong often enough to be worth a word: a body that
/// lays out its own page, and an alignment tab with no environment around it,
/// which is a LaTeX error and would sink the whole compile.
pub fn warnings(blocks: &[Block]) -> Vec<String> {
    let mut out = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let position = index + 1;
        if crate::render::has_layout(&block.latex) {
            out.push(format!(
                "Passage {position} : il contient sa propre mise en page, que la charte ne \
                 reproduira pas telle quelle."
            ));
        }
        if crate::render::has_stray_alignment(&block.latex) {
            out.push(format!(
                "Passage {position} : un « & » sans environnement d'alignement autour. Le PDF \
                 ne compilera pas tant qu'il est là."
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(blocks: &str) -> String {
        format!(r#"{{"title":"Fiche","blocks":[{blocks}]}}"#)
    }

    #[test]
    fn reads_a_sheet_and_leaves_plume_s_own_fields_alone() {
        let import = parse(&sheet(
            r#"{"kind":"part","number":"I","title":"Colinéarité"},
               {"kind":"application","title":"Exercice 1","latex":"Montrer que $x=1$."},
               {"kind":"proof","latex":"On calcule.","audience":["teacher"]}"#,
        ))
        .expect("valid sheet");

        assert_eq!(import.title, "Fiche");
        assert_eq!(import.blocks.len(), 3);
        assert_eq!(import.blocks[0].number.as_deref(), Some("I"));
        assert_eq!(import.blocks[2].audience, vec!["teacher".to_string()]);

        // Unread is what this is, and confidence is about handwriting.
        for block in &import.blocks {
            assert!(!block.reviewed, "nobody has read it yet");
            assert_eq!(block.confidence, 1.0);
            assert!(!block.taught_end);
            assert!(block.id.is_empty(), "ids are assigned by the layout");
        }
    }

    /// The file cannot hand itself a clean bill of health.
    #[test]
    fn fields_belonging_to_plume_are_refused_outright() {
        let refused = parse(&sheet(
            r#"{"kind":"text","latex":"Bonjour.","reviewed":true}"#,
        ));
        assert!(refused.is_err(), "an unknown field is a refusal, not a shrug");

        let refused = parse(&sheet(r#"{"kind":"text","latex":"Bonjour.","taughtEnd":true}"#));
        assert!(refused.is_err());
    }

    /// The file says what the document is; the teacher decides on the screen.
    #[test]
    fn tags_travel_with_the_sheet_and_are_normalised() {
        let import = parse(
            r#"{"title":"DS","tags":["  DS ","ds","", "Seconde"],"blocks":[{"kind":"text","latex":"x"}]}"#,
        )
        .unwrap();
        assert_eq!(import.tags, vec!["DS".to_string(), "Seconde".to_string()]);

        let none = parse(r#"{"blocks":[{"kind":"text","latex":"x"}]}"#).unwrap();
        assert!(none.tags.is_empty(), "no proposal is not the same as a default");
    }

    #[test]
    fn an_unknown_kind_names_itself_and_the_alternatives() {
        let error = parse(&sheet(r#"{"kind":"exercice","latex":"..."}"#)).unwrap_err();
        assert!(error.contains("Passage 1"), "{error}");
        assert!(error.contains("exercice"), "{error}");
        assert!(error.contains("application"), "lists what is accepted: {error}");
    }

    /// The "Exercices : null" of readings, arriving through the other door.
    #[test]
    fn an_empty_passage_is_refused_but_a_bare_heading_is_not() {
        let error = parse(&sheet(r#"{"kind":"definition","title":"Vecteur"}"#)).unwrap_err();
        assert!(error.contains("Passage 1"), "{error}");

        parse(&sheet(r#"{"kind":"part","title":"Exercices"}"#))
            .expect("a heading is its title");
    }

    #[test]
    fn an_unknown_audience_is_refused_rather_than_dropped() {
        let error =
            parse(&sheet(r#"{"kind":"text","latex":"x","audience":["eleves"]}"#)).unwrap_err();
        assert!(error.contains("eleves"), "{error}");
    }

    #[test]
    fn an_empty_or_unparseable_file_says_which() {
        assert!(parse("").unwrap_err().contains("Collez"));
        assert!(parse(r#"{"blocks":[]}"#).unwrap_err().contains("aucun passage"));
        assert!(parse("pas du json").is_err());
        assert!(parse(r#"{"title":"Fiche"}"#).is_err(), "blocks is required");
    }

    /// Ids encode position here exactly as they do after a reading, so every
    /// transcript edit works on an imported document without a special case.
    #[test]
    fn the_layout_numbers_the_passages_as_a_reading_would() {
        let import = parse(&sheet(
            r#"{"kind":"text","latex":"un"},{"kind":"text","latex":"deux"}"#,
        ))
        .unwrap();
        let transcript = transcript_of(import.blocks);

        assert_eq!(transcript.pages.len(), 1, "no photographs, one page");
        let ids: Vec<&str> =
            transcript.pages[0].blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["p01-b01", "p01-b02"]);
    }

    #[test]
    fn layout_and_stray_tabs_are_flagged_without_refusing_the_course() {
        let import = parse(&sheet(
            r#"{"kind":"text","latex":"\\begin{minipage}{5cm}x\\end{minipage}"},
               {"kind":"equation","latex":"x &= 1"},
               {"kind":"text","latex":"Tout va bien."}"#,
        ))
        .expect("flagged, not refused");

        let warnings = warnings(&import.blocks);
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("Passage 1"), "{warnings:?}");
        assert!(warnings[1].contains("Passage 2"), "{warnings:?}");
    }

    /// The instructions are handed to a model verbatim, so a kind missing from
    /// them is a kind the teacher will never be offered.
    #[test]
    fn the_instructions_list_every_kind() {
        let instructions = instructions();
        for kind in BLOCK_KINDS {
            assert!(instructions.contains(kind), "{kind} missing from the instructions");
        }
    }

    /// The example in the instructions is what a model will copy, so it had
    /// better be something this parser accepts.
    #[test]
    fn the_example_in_the_instructions_imports() {
        let instructions = instructions();
        let start = instructions.find('{').expect("an example");
        let end = instructions.rfind("}\n\nRègles").map(|at| at + 1).expect("its end");
        parse(&instructions[start..end]).expect("the example we hand out must import");
    }

    /// Everything the parser produces must survive the round trip to disk.
    #[test]
    fn an_imported_course_reloads_as_itself() {
        let import = parse(&sheet(
            r#"{"kind":"application","title":"Exercice 1","latex":"$x=1$","audience":["student"]}"#,
        ))
        .unwrap();
        let transcript = transcript_of(import.blocks);

        let raw = serde_json::to_string(&transcript).unwrap();
        let back: Transcript = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.pages[0].blocks[0].title.as_deref(), Some("Exercice 1"));
        assert_eq!(back.pages[0].blocks[0].audience, vec!["student".to_string()]);
    }
}
