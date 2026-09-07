//! An MCP server over stdio, so a conversation can put a document in the workbook.
//!
//! Run as `plume mcp`. Not a second binary: this is the one already installed,
//! already signed, at a path that does not move — which is the whole of what a
//! client needs to spawn it. The window is never built; `main` branches before
//! Tauri starts, and what is left is an ordinary process reading lines.
//!
//! **Nothing may be printed.** stdout carries the protocol and only the
//! protocol; a stray `println!` corrupts the stream and the client's error will
//! name a parse failure rather than the print. `logbus` is safe here because it
//! drops everything until an app handle is set, which never happens in this
//! process — stderr stays free for anything that must be said.
//!
//! **What this server may do.** It writes documents into the workbook, which is
//! the point, and that is a capability handed to any conversation the teacher
//! has this server connected to — including one where the model has just read a
//! web page. The client asks before every call, and that confirmation is the
//! real gate. On this side the guard is narrowness: five tools, no path ever
//! taken from an argument, and a document arriving unread rather than approved.

use crate::{import, templates, workspace};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// The version this server speaks when a client does not ask for one.
const PROTOCOL: &str = "2025-06-18";

/// Reads messages until stdin closes.
pub fn serve() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }

        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle(&message),
            // No id to answer with, so this is all that can be said.
            Err(error) => Some(failure(Value::Null, -32700, format!("JSON illisible : {error}"))),
        };

        if let Some(reply) = reply {
            // One message per line, and flushed: a client blocked waiting for a
            // reply that is sitting in a buffer looks exactly like a hang.
            let _ = writeln!(stdout, "{reply}");
            let _ = stdout.flush();
        }
    }
}

/// One message in, at most one out.
///
/// Pure, because the transport is three lines and the protocol is everything
/// that can go wrong: a whole conversation can then be played through in a test
/// without spawning anything.
pub fn handle(message: &Value) -> Option<Value> {
    let method = message.get("method").and_then(Value::as_str).unwrap_or_default();

    // A notification has no id and must never be answered — replying to
    // `notifications/initialized` is enough to make a strict client hang up.
    let id = message.get("id")?.clone();

    match method {
        "initialize" => Some(success(id, initialize(message))),
        "ping" => Some(success(id, json!({}))),
        "tools/list" => Some(success(id, json!({ "tools": tools() }))),
        "tools/call" => Some(call(id, message)),
        _ => Some(failure(id, -32601, format!("Méthode inconnue : {method}"))),
    }
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn failure(id: Value, code: i32, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// What the model reads back from a tool.
///
/// A refusal comes back as a *result* carrying `isError`, not as a protocol
/// error: the difference is that the model sees the text and can act on it. So
/// "Passage 3 : « exercice » n'est pas un type de passage" becomes a correction
/// it can make on its own, instead of a failure it can only report.
fn output(text: String, failed: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": failed,
    })
}

fn initialize(message: &Value) -> Value {
    // Echo the version asked for. This server is five tools with no optional
    // features, so every version that has existed describes it identically, and
    // refusing one over a date string helps nobody.
    let version = message
        .get("params")
        .and_then(|p| p.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL);

    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "plume", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn tools() -> Value {
    json!([
        {
            "name": "list_chartes",
            "description":
                "Liste les chartes (mises en page) du classeur Plume. À appeler avant \
                 create_document quand le professeur n'a pas dit laquelle utiliser.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "list_documents",
            "description":
                "Liste les documents du classeur : identifiant, titre, nombre de passages. \
                 Utile pour retrouver un document dont le professeur donne le nom.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "list_tags",
            "description":
                "Liste les étiquettes en usage dans le classeur — « cours », « exercices », \
                 « DS »… — avec le nombre de documents pour chacune. À appeler avant \
                 create_document, pour reprendre l'orthographe d'une étiquette existante.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "read_document",
            "description":
                "Lit les passages d'un document existant, pour écrire des exercices qui \
                 portent sur cette leçon et en reprennent les notations.",
            "inputSchema": {
                "type": "object",
                "required": ["id"],
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string", "description": "Identifiant donné par list_documents." }
                }
            }
        },
        {
            "name": "create_document",
            "description":
                "Ajoute un document au classeur Plume, écrit en passages typés plutôt qu'en \
                 document LaTeX : le professeur pourra ensuite le relire passage par \
                 passage, réserver les corrections à sa version, et lui appliquer sa \
                 charte. Le document arrive non relu, à vérifier par le professeur.",
            "inputSchema": {
                "type": "object",
                "required": ["title", "blocks"],
                "additionalProperties": false,
                "properties": {
                    "title": { "type": "string", "description": "Titre du document." },
                    "charte": {
                        "type": "string",
                        "description":
                            "Identifiant de charte donné par list_chartes. Par défaut, \
                             la première du classeur."
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description":
                            "Ce qu'est le document : « exercices », « DS », « DM »… Le \
                             professeur trie son classeur avec. Reprends une étiquette \
                             existante (list_tags) quand elle convient ; une nouvelle est \
                             créée en l'utilisant."
                    },
                    "blocks": import::schema()
                }
            }
        }
    ])
}

fn call(id: Value, message: &Value) -> Value {
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    // An unknown tool is the client's mistake, not the model's: it can only
    // have come from a stale tool list, and no wording will help the model fix
    // it. Everything a model can act on comes back as `isError` instead.
    let work = match name {
        "list_chartes" => list_chartes(),
        "list_documents" => list_documents(),
        "list_tags" => list_tags(),
        "read_document" => read_document(&arguments),
        "create_document" => create_document(&arguments),
        _ => return failure(id, -32602, format!("Outil inconnu : {name}")),
    };

    match work {
        Ok(text) => success(id, output(text, false)),
        Err(text) => success(id, output(text, true)),
    }
}

fn list_chartes() -> Result<String, String> {
    let root = workspace::root();
    let chartes: Vec<Value> = templates::list(&root)
        .into_iter()
        .map(|template| {
            json!({
                "id": template.id,
                "name": template.name,
                "description": template.description,
            })
        })
        .collect();

    Ok(pretty(&json!({ "chartes": chartes })))
}

fn list_documents() -> Result<String, String> {
    let documents: Vec<Value> = workspace::list()
        .into_iter()
        .map(|document| {
            json!({
                "id": document.id,
                "title": document.title,
                "charte": document.template_id,
                "tags": document.tags,
                "passages": count_blocks(&document.id),
                "origin": document.origin,
            })
        })
        .collect();

    Ok(pretty(&json!({ "documents": documents })))
}

fn list_tags() -> Result<String, String> {
    let tags: Vec<Value> = workspace::all_tags()
        .into_iter()
        .map(|(tag, count)| json!({ "tag": tag, "count": count }))
        .collect();
    Ok(pretty(&json!({ "tags": tags })))
}

fn count_blocks(id: &str) -> usize {
    read_document_transcript(id)
        .map(|transcript| transcript.pages.iter().map(|p| p.blocks.len()).sum())
        .unwrap_or(0)
}

fn read_document_transcript(id: &str) -> Option<crate::ir::Transcript> {
    let raw =
        std::fs::read_to_string(workspace::document_dir(id).join("transcript.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

fn read_document(arguments: &Value) -> Result<String, String> {
    let id = text(arguments, "id")?;

    // The id names a folder, so it must not be able to name any other one.
    // Nothing in the workbook has a separator or a dot in its name.
    if id.contains(['/', '\\']) || id.contains("..") {
        return Err(format!("« {id} » n'est pas un identifiant de document."));
    }

    let document = workspace::load(&id)
        .map_err(|_| format!("Aucun document « {id} ». Appelez list_documents pour les voir."))?;
    let transcript = read_document_transcript(&id)
        .ok_or_else(|| format!("Le document « {id} » n'a pas encore été transcrit."))?;

    let passages: Vec<Value> = transcript
        .pages
        .iter()
        .flat_map(|page| page.blocks.iter())
        .map(|block| {
            json!({
                "kind": block.kind,
                "title": block.title,
                "number": block.number,
                "latex": block.latex,
                "audience": block.audience,
            })
        })
        .collect();

    Ok(pretty(&json!({
        "id": document.id,
        "title": document.title,
        "charte": document.template_id,
        "passages": passages,
    })))
}

fn create_document(arguments: &Value) -> Result<String, String> {
    let title = text(arguments, "title")?;
    let blocks = arguments
        .get("blocks")
        .ok_or("Il manque « blocks » : les passages du document.")?;

    // Back through the same parser the interface uses, from the same text
    // shape, so a document arriving this way cannot be one the paste path would
    // have refused.
    let source = json!({ "title": title, "blocks": blocks }).to_string();

    let root = workspace::root();
    let chartes = templates::list(&root);
    let charte = match arguments.get("charte").and_then(Value::as_str) {
        Some(asked) => chartes
            .iter()
            .find(|template| template.id == asked)
            .ok_or_else(|| {
                format!(
                    "Aucune charte « {asked} ». Disponibles : {}.",
                    chartes.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(", ")
                )
            })?
            .id
            .clone(),
        None => chartes
            .first()
            .ok_or("Ce classeur n'a aucune charte.")?
            .id
            .clone(),
    };

    let tags: Vec<String> = arguments
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| tags.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();

    let document = import::create(&source, &title, &charte, &tags)?;
    let passages = count_blocks(&document.id);

    Ok(pretty(&json!({
        "id": document.id,
        "title": document.title,
        "tags": document.tags,
        "passages": passages,
        "charte": charte,
        "message": format!(
            "« {} » est dans le classeur, {passages} passage(s), à relire dans Plume.",
            document.title
        ),
    })))
}

/// A required string argument, or the reason it is not usable.
fn text(arguments: &Value, key: &str) -> Result<String, String> {
    let value = arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Il manque « {key} »."))?;
    Ok(value.to_string())
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// The manifest of a bundle Claude Desktop installs by opening it.
///
/// A `.mcpb` is a zip with this file at its root. Nothing else goes in: the
/// server is the Plume already on this machine, so the bundle only has to say
/// where. That keeps it a few hundred bytes and, more to the point, keeps one
/// copy of Plume — a binary zipped into a bundle would stop updating the day
/// it was made.
pub fn bundle_manifest() -> Result<Value, String> {
    let binary = std::env::current_exe()
        .map_err(|e| format!("Chemin de Plume introuvable : {e}"))?
        .to_string_lossy()
        .to_string();

    let tools: Vec<Value> = tools()
        .as_array()
        .into_iter()
        .flatten()
        .map(|tool| json!({ "name": tool["name"], "description": tool["description"] }))
        .collect();

    Ok(json!({
        "manifest_version": "0.3",
        "name": "plume",
        "display_name": "Plume",
        "version": env!("CARGO_PKG_VERSION"),
        "description":
            "Envoie un document dans le classeur Plume, en \
             passages que le professeur relit un par un.",
        "author": { "name": "Plume" },
        "server": {
            "type": "binary",
            "entry_point": binary,
            "mcp_config": { "command": binary, "args": ["mcp"] }
        },
        "tools": tools,
    }))
}

/// Writes the bundle to the temporary directory and says where.
pub fn bundle() -> Result<std::path::PathBuf, String> {
    bundle_to(&std::env::temp_dir().join("plume.mcpb"))
}

/// Writes the bundle at `path`.
///
/// Where it lands matters more than it looks: on a Mac, opening the file is the
/// installation, but the Microsoft Store build of Claude registers no file type,
/// and there the teacher installs it from Claude's own settings by picking the
/// file — so it has to be somewhere they chose and can find again.
pub fn bundle_to(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    use std::io::Write as _;

    let manifest = bundle_manifest()?;
    let path = path.to_path_buf();
    let file = std::fs::File::create(&path)
        .map_err(|e| format!("Écriture de {} : {e}", path.display()))?;

    let mut zip = zip::ZipWriter::new(file);
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    zip.start_file("manifest.json", stored)
        .and_then(|()| zip.write_all(pretty(&manifest).as_bytes()).map_err(Into::into))
        .and_then(|()| zip.finish().map(|_| ()))
        .map_err(|e| format!("Archive : {e}"))?;

    Ok(path)
}

/// The block a teacher pastes into their MCP client's configuration.
///
/// Built from `current_exe` rather than a written-down path: in a bundle that
/// is `/Applications/Plume.app/Contents/MacOS/plume`, in development it is the
/// binary being run, and either way it is the one that will answer. A path
/// typed by hand is the step where this stops working after an update.
pub fn client_config() -> String {
    let binary = std::env::current_exe()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| "plume".into());

    pretty(&json!({
        "mcpServers": {
            "plume": { "command": binary, "args": ["mcp"] }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: u32, method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    #[test]
    fn a_handshake_answers_with_tools_and_the_version_asked_for() {
        let reply = handle(&request(
            1,
            "initialize",
            json!({ "protocolVersion": "2024-11-05", "capabilities": {} }),
        ))
        .expect("initialize is a request");

        assert_eq!(reply["jsonrpc"], "2.0");
        assert_eq!(reply["id"], 1);
        assert_eq!(reply["result"]["protocolVersion"], "2024-11-05");
        assert!(reply["result"]["capabilities"]["tools"].is_object());
        assert_eq!(reply["result"]["serverInfo"]["name"], "plume");
    }

    /// Answering a notification is enough to make a strict client hang up, and
    /// `notifications/initialized` arrives in every single session.
    #[test]
    fn notifications_are_never_answered() {
        let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(handle(&notification).is_none());

        let cancelled = json!({ "jsonrpc": "2.0", "method": "notifications/cancelled",
                                "params": { "requestId": 1 } });
        assert!(handle(&cancelled).is_none());
    }

    #[test]
    fn every_tool_is_listed_with_a_usable_schema() {
        let reply = handle(&request(2, "tools/list", json!({}))).expect("a request");
        let tools = reply["result"]["tools"].as_array().expect("a list");

        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(
            names,
            vec!["list_chartes", "list_documents", "list_tags", "read_document", "create_document"]
        );

        for tool in tools {
            assert!(
                tool["description"].as_str().is_some_and(|d| d.len() > 40),
                "{} needs a description a model can act on",
                tool["name"]
            );
            assert_eq!(tool["inputSchema"]["type"], "object");
        }

        // The passages carry the same contract the paste path validates.
        let blocks = &tools[4]["inputSchema"]["properties"]["blocks"];
        let kinds = blocks["items"]["properties"]["kind"]["enum"]
            .as_array()
            .expect("the kinds");
        assert_eq!(kinds.len(), crate::ir::BLOCK_KINDS.len());
    }

    #[test]
    fn an_unknown_method_is_a_protocol_error() {
        let reply = handle(&request(3, "resources/list", json!({}))).expect("a request");
        assert_eq!(reply["error"]["code"], -32601);
        assert!(reply.get("result").is_none());
    }

    #[test]
    fn an_unknown_tool_is_a_protocol_error() {
        let reply = handle(&request(
            4,
            "tools/call",
            json!({ "name": "delete_everything", "arguments": {} }),
        ))
        .expect("a request");
        assert_eq!(reply["error"]["code"], -32602);
    }

    /// The distinction that makes the server usable: a refusal the model can
    /// act on comes back as content, not as a protocol error it can only
    /// report. The wording is the parser's, so the model is told which passage.
    #[test]
    fn a_refusal_reaches_the_model_as_text_it_can_act_on() {
        let reply = handle(&request(
            5,
            "tools/call",
            json!({
                "name": "create_document",
                "arguments": {
                    "title": "Fiche",
                    "blocks": [{ "kind": "exercice", "latex": "..." }]
                }
            }),
        ))
        .expect("a request");

        assert!(reply.get("error").is_none(), "not a protocol error");
        assert_eq!(reply["result"]["isError"], true);

        let text = reply["result"]["content"][0]["text"].as_str().expect("text");
        assert!(text.contains("Passage 1"), "{text}");
        assert!(text.contains("exercice"), "{text}");
        assert!(text.contains("application"), "lists what is accepted: {text}");
    }

    #[test]
    fn a_missing_argument_says_which_one() {
        let reply = handle(&request(
            6,
            "tools/call",
            json!({ "name": "create_document", "arguments": { "title": "Fiche" } }),
        ))
        .expect("a request");

        assert_eq!(reply["result"]["isError"], true);
        assert!(reply["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("blocks")));
    }

    /// The id names a folder. Nothing in the workbook is called `..`.
    #[test]
    fn a_course_id_cannot_walk_out_of_the_workbook() {
        for id in ["../../etc", "..", "a/b"] {
            let reply = handle(&request(
                7,
                "tools/call",
                json!({ "name": "read_document", "arguments": { "id": id } }),
            ))
            .expect("a request");
            assert_eq!(reply["result"]["isError"], true, "{id} must be refused");
        }
    }

    /// The teacher pastes this into a file that must stay valid JSON, and the
    /// command has to be the binary that will actually answer.
    #[test]
    fn the_client_configuration_is_json_naming_this_binary() {
        let config: Value =
            serde_json::from_str(&client_config()).expect("valid JSON to paste");
        let server = &config["mcpServers"]["plume"];

        assert_eq!(server["args"], json!(["mcp"]));
        let command = server["command"].as_str().expect("a command");
        assert!(
            std::path::Path::new(command).is_absolute(),
            "a relative command would depend on where the client was started: {command}"
        );
    }

    /// A bundle is a zip with the manifest at its root, and Claude Desktop
    /// reads exactly these fields from it.
    #[test]
    fn the_bundle_manifest_names_this_binary_and_every_tool() {
        let manifest = bundle_manifest().expect("a manifest");
        assert_eq!(manifest["manifest_version"], "0.3");
        assert_eq!(manifest["name"], "plume");
        assert_eq!(manifest["server"]["type"], "binary");
        assert_eq!(manifest["server"]["mcp_config"]["args"], json!(["mcp"]));

        let command = manifest["server"]["mcp_config"]["command"].as_str().expect("a command");
        assert!(std::path::Path::new(command).is_absolute(), "{command}");
        assert_eq!(manifest["server"]["entry_point"], command);

        let listed = manifest["tools"].as_array().expect("tools").len();
        assert_eq!(listed, tools().as_array().unwrap().len());
    }

    /// The zip has to open, and hold the manifest under the name the client
    /// looks for.
    #[test]
    fn the_bundle_is_a_zip_holding_the_manifest() {
        let path = bundle().expect("a bundle");
        let bytes = std::fs::read(&path).expect("readable");
        let holds = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert_eq!(&bytes[..2], b"PK", "a zip starts with its signature");
        assert!(holds(b"manifest.json"), "the entry must be named manifest.json");
        // Stored, not deflated, so the manifest is readable in the archive as
        // is — and so a client with no inflater still opens it.
        assert!(holds(b"\"manifest_version\""));
    }

    /// A missing id is not a crash, and says what to call instead.
    #[test]
    fn an_unknown_course_points_at_the_way_to_find_one() {
        let reply = handle(&request(
            8,
            "tools/call",
            json!({ "name": "read_document", "arguments": { "id": "pas-un-cours-abcdef" } }),
        ))
        .expect("a request");

        assert_eq!(reply["result"]["isError"], true);
        assert!(reply["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("list_documents")));
    }
}
