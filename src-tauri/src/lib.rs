mod claude;
pub mod engine;
mod env_check;
mod figures;
pub mod ir;
mod latex;
mod logbus;
mod machine;
pub mod photos;
mod proc;
mod recognizer;
mod render;
mod runs;
mod settings;
mod templates;
mod workspace;

use serde::Serialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::fs;
use std::path::PathBuf;
use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;

// ---------------------------------------------------------------------------
// Environment and workbook
// ---------------------------------------------------------------------------

/// Async because it launches `claude --version` and a LaTeX engine: on the main
/// thread those spawns freeze the window before the first paint.
#[tauri::command]
async fn check_environment() -> env_check::Environment {
    tauri::async_runtime::spawn_blocking(env_check::inspect)
        .await
        .unwrap_or_else(|_| env_check::Environment {
            tools: Vec::new(),
            ready: false,
            auto_pages: 1,
            memory_gb: None,
        })
}

/// A course plus what its transcript says is left to do. Computed at list time
/// rather than stored: the transcript is the single source of truth for review
/// state, and a handful of JSON files is cheap to read.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DocumentSummary {
    #[serde(flatten)]
    document: workspace::Document,
    /// Blocks in the transcript; 0 when the course has not been read yet.
    block_count: usize,
    /// Blocks below the doubt threshold and not yet confirmed by the teacher.
    doubtful_count: usize,
    /// Passages the class has covered, when a boundary is set. What the course
    /// list answers on a Sunday evening: where did we get to?
    taught_count: Option<usize>,
    /// Title of the heading the class stopped under — the teacher's own words,
    /// so "Produit scalaire" rather than a passage number.
    taught_heading: Option<String>,
}

/// Headings, in the order they nest. A boundary is described by the last one
/// standing above it, which is how a teacher names where a class got to.
const HEADINGS: &[&str] = &["chapter", "part", "subpart", "paragraph"];

fn summarise(document: workspace::Document) -> DocumentSummary {
    let transcript = read_transcript(&document.id).ok();
    let (block_count, doubtful_count) = transcript
        .as_ref()
        .map(|transcript| {
            let blocks: Vec<&ir::Block> =
                transcript.pages.iter().flat_map(|p| p.blocks.iter()).collect();
            let doubtful = blocks
                .iter()
                .filter(|b| b.confidence < ir::DOUBT_THRESHOLD && !b.reviewed)
                .count();
            (blocks.len(), doubtful)
        })
        .unwrap_or((0, 0));

    let taught_count = transcript.as_ref().and_then(ir::taught_count);
    let taught_heading = transcript.as_ref().and_then(|transcript| {
        let covered = ir::taught_count(transcript)?;
        transcript
            .pages
            .iter()
            .flat_map(|page| page.blocks.iter())
            .take(covered)
            .filter(|block| HEADINGS.contains(&block.kind.as_str()))
            .filter_map(|block| block.title.as_deref().map(str::trim).filter(|t| !t.is_empty()))
            .last()
            .map(str::to_string)
    });

    DocumentSummary {
        document,
        block_count,
        doubtful_count,
        taught_count,
        taught_heading,
    }
}

#[tauri::command]
fn list_documents() -> Vec<DocumentSummary> {
    workspace::list().into_iter().map(summarise).collect()
}

#[tauri::command]
fn list_trash() -> Vec<workspace::TrashedCourse> {
    workspace::trashed()
}

#[tauri::command]
fn restore_document(folder: String) -> Result<workspace::Document, String> {
    workspace::restore(&folder)
}

#[tauri::command]
fn purge_document(folder: String) -> Result<(), String> {
    workspace::purge(&folder)
}

/// `macos` | `windows` | `linux` — the frontend draws its own window chrome
/// and needs to know which side the buttons belong to.
#[tauri::command]
fn os_platform() -> String {
    std::env::consts::OS.to_string()
}

#[tauri::command]
fn get_document(id: String) -> Result<workspace::Document, String> {
    workspace::load(&id)
}

#[tauri::command]
fn document_pages(id: String) -> Vec<String> {
    workspace::page_files(&id)
}

#[tauri::command]
fn delete_document(id: String) -> Result<(), String> {
    workspace::delete(&id).map(|_| ())
}

#[tauri::command]
fn rename_document(id: String, title: String) -> Result<workspace::Document, String> {
    workspace::rename(&id, &title)
}

#[tauri::command]
async fn add_pages(
    app: AppHandle,
    id: String,
    sources: Vec<String>,
) -> Result<workspace::Document, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths: Vec<PathBuf> = sources.into_iter().map(PathBuf::from).collect();
        workspace::add_pages(&id, &paths, &|done, total| {
            let _ = app.emit("import", ImportProgress { done, total });
        })
    })
    .await
    .map_err(|e| format!("Import interrompu : {e}"))?
}

/// Removes a page and keeps the transcript in step with it.
///
/// Page numbers and block ids encode position, so dropping a page without
/// moving the transcript would leave every later block pointing at the wrong
/// photo — worse than losing the transcript outright.
#[tauri::command]
fn remove_page(id: String, number: usize) -> Result<workspace::Document, String> {
    let document = workspace::remove_page(&id, number)?;

    if let Ok(mut transcript) = read_transcript(&id) {
        transcript.pages.retain(|page| page.number != number);
        for page in &mut transcript.pages {
            if page.number > number {
                page.number -= 1;
            }
            for (index, block) in page.blocks.iter_mut().enumerate() {
                block.id = format!("p{:02}-b{:02}", page.number, index + 1);
            }
        }
        write_transcript(&id, &transcript)?;
        logbus::info(
            "workspace",
            format!("Transcription renumérotée — {} page(s)", transcript.pages.len()),
        );
    }

    Ok(document)
}

/// Moves the transcript to follow a new page order.
///
/// A partially read course has fewer transcript pages than photographs, so the
/// pages that exist are matched by number rather than by position.
fn reorder_transcript(transcript: &mut ir::Transcript, order: &[usize]) {
    let mut moved = Vec::with_capacity(transcript.pages.len());
    for (position, &number) in order.iter().enumerate() {
        if let Some(mut page) = transcript.pages.iter().find(|p| p.number == number).cloned() {
            page.number = position + 1;
            for (index, block) in page.blocks.iter_mut().enumerate() {
                block.id = format!("p{:02}-b{:02}", page.number, index + 1);
            }
            moved.push(page);
        }
    }
    moved.sort_by_key(|page| page.number);
    transcript.pages = moved;
}

/// Reorders the pages and moves the transcript with them.
///
/// Page numbers and block ids encode position, so moving the photographs alone
/// would leave every block pointing at the wrong page.
#[tauri::command]
fn reorder_pages(id: String, order: Vec<usize>) -> Result<workspace::Document, String> {
    let document = workspace::reorder_pages(&id, &order)?;

    if let Ok(mut transcript) = read_transcript(&id) {
        reorder_transcript(&mut transcript, &order);
        write_transcript(&id, &transcript)?;
        logbus::info(
            "workspace",
            format!("Transcription réordonnée — {} page(s)", transcript.pages.len()),
        );
    }

    Ok(document)
}

/// Absolute paths to the page images, for the webview to display as thumbnails.
#[tauri::command]
fn document_page_paths(id: String) -> Vec<String> {
    let pages = workspace::document_dir(&id).join("pages");
    workspace::page_files(&id)
        .into_iter()
        .map(|name| pages.join(name).to_string_lossy().to_string())
        .collect()
}

/// Stores the teacher's reading conventions for this course.
///
/// They are appended verbatim to the recogniser's instructions, so they take
/// effect on the next read or correction — no re-import needed.
#[tauri::command]
fn set_reading_rules(id: String, rules: String) -> Result<(), String> {
    let mut document = workspace::load(&id)?;
    document.reading_rules = rules.trim().to_string();
    document.updated_at = workspace::now_ms();
    logbus::info("workspace", format!("Règles de lecture de « {} » mises à jour", document.title));
    workspace::save(&document)
}

#[tauri::command]
fn list_templates() -> Vec<templates::Template> {
    templates::list(&workspace::root())
}

/// Async because importing decodes, rotates and resamples every photo. A phone
/// picture is 48 megapixels; doing that on the main thread freezes the window
/// for the whole import.
#[tauri::command]
async fn create_document(
    app: AppHandle,
    title: String,
    template_id: String,
    sources: Vec<String>,
) -> Result<workspace::Document, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths: Vec<PathBuf> = sources.into_iter().map(PathBuf::from).collect();
        workspace::create(&title, &template_id, &paths, &|done, total| {
            let _ = app.emit("import", ImportProgress { done, total });
        })
    })
    .await
    .map_err(|e| format!("Import interrompu : {e}"))?
}

/// Downloads and runs the official Claude Code installer.
///
/// Asking a teacher to open PowerShell and paste a command is the obstacle this
/// application exists to remove.
#[tauri::command]
async fn install_claude(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        claude::install(&|step| {
            let _ = app.emit("provision", step.to_string());
        })
        .map(|path| path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Installation interrompue : {e}"))?
}

/// Opens a terminal running `claude`, for the sign-in step.
#[tauri::command]
fn open_claude_login() -> Result<(), String> {
    claude::open_login()
}

/// Downloads and installs the LaTeX engine.
///
/// Async and reporting: it is a network download, and a silent button would
/// look broken for the half-minute it takes.
#[tauri::command]
async fn install_engine(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        engine::install(&|step| {
            let _ = app.emit("provision", step.to_string());
        })
        .map(|path| path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Installation interrompue : {e}"))?
}

#[tauri::command]
fn remove_engine() -> Result<(), String> {
    engine::remove()
}

/// Compiles one TikZ block and returns the image path for the preview.
///
/// The review surface is where a wrong diagram gets caught; showing "see the
/// PDF" there defeats the purpose.
#[tauri::command]
async fn render_figure(id: String, tikz: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = workspace::root();
        let document = workspace::load(&id)?;
        let template = templates::load(&root, &document.template_id)
            .ok_or("Modèle introuvable.".to_string())?;
        figures::render(&workspace::document_dir(&id), &root, &template, &tikz)
            .map(|path| path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Rendu du schéma interrompu : {e}"))?
}

/// Preview of the preamble as it will be written at the top of the .tex.
/// Writes back a template whose key values the teacher edited.
///
/// Only values are meant to change; structure comes from the bundled template
/// and is replaced on upgrade.
#[tauri::command]
fn duplicate_template(source_id: String, name: String) -> Result<templates::Template, String> {
    templates::duplicate(&workspace::root(), &source_id, &name)
}

#[tauri::command]
fn delete_template(id: String) -> Result<(), String> {
    templates::delete(&workspace::root(), &id)
}

#[tauri::command]
fn read_template_preamble(id: String) -> Result<String, String> {
    templates::read_preamble(&workspace::root(), &id)
}

#[tauri::command]
fn write_template_preamble(id: String, text: String) -> Result<(), String> {
    templates::write_preamble(&workspace::root(), &id, &text)
}

/// Compiles the template on its own, so an error surfaces in the editor rather
/// than during an export.
#[tauri::command]
async fn check_template(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = workspace::root();
        let template = templates::load(&root, &id).ok_or("Modèle introuvable.")?;
        templates::check(&root, &template)
    })
    .await
    .map_err(|e| format!("Vérification interrompue : {e}"))?
}

#[tauri::command]
fn save_template(template: templates::Template) -> Result<(), String> {
    templates::save(&workspace::root(), &template)
}

#[tauri::command]
fn preview_preamble(template_id: String) -> Result<String, String> {
    let root = workspace::root();
    let template =
        templates::load(&root, &template_id).ok_or("Modèle introuvable.".to_string())?;
    templates::render_preamble(&root, &template).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings() -> settings::Settings {
    settings::load()
}

#[tauri::command]
fn save_settings(settings: settings::Settings) -> Result<(), String> {
    settings::save(&settings)
}

#[tauri::command]
fn workspace_path() -> String {
    workspace::root().to_string_lossy().to_string()
}

/// Opens the workbook in the system file explorer.
#[tauri::command]
fn reveal_workspace(app: AppHandle) -> Result<(), String> {
    let dir = workspace::ensure_root().map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Opens a produced file (the .tex, the PDF) with the system default app.
#[tauri::command]
fn reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Opens the last compiled PDF of a course, straight from the course list —
/// the path is resolved here so the webview never manipulates file paths.
#[tauri::command]
fn open_course_pdf(app: AppHandle, id: String) -> Result<(), String> {
    let document = workspace::load(&id)?;
    let name = document
        .last_pdf
        .ok_or("Aucun PDF n'a encore été fabriqué pour ce cours.")?;
    let path = workspace::document_dir(&id).join(name);
    if !path.is_file() {
        return Err("Le PDF n'est plus dans le dossier du cours. Refabriquez-le.".into());
    }
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Opens an install page in the browser.
///
/// Restricted to HTTPS: the webview must not be able to make the system open a
/// local file or an arbitrary URL scheme.
#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        // User-facing message, hence French.
        return Err("Seules les adresses https sont autorisées.".to_string());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Whether updates can be checked at all.
///
/// The updater needs a public key baked in at build time. Without one, reporting
/// "no update available" would be a lie — nothing was ever checked — so the
/// interface says the feature is not configured instead.
#[tauri::command]
fn updates_configured(app: AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(|key| key.as_str())
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
}

#[tauri::command]
fn logs() -> Vec<logbus::LogEntry> {
    logbus::history()
}

#[tauri::command]
fn clear_logs() {
    logbus::clear();
}

/// Lets the interface report into the same console as the backend.
///
/// An error the user can see but that leaves no trace in the console is the
/// worst of both worlds: they know something broke and have nothing to show.
#[tauri::command]
fn log_client(level: String, scope: String, message: String, detail: Option<String>) {
    logbus::from_client(&level, &scope, message, detail);
}

// ---------------------------------------------------------------------------
// Transcription
// ---------------------------------------------------------------------------

const TRANSCRIPT_FILE: &str = "transcript.json";

/// Import can take a while on a course of a dozen phone photos, so it reports
/// rather than leaving a button spinning.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ImportProgress {
    done: usize,
    total: usize,
}

/// Lifecycle of one page during a read, for the timeline.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PageState {
    document_id: String,
    page: usize,
    /// `reading` | `done` | `failed` | `cancelled`
    state: String,
    blocks: usize,
    message: Option<String>,
}

/// Proof of life while a page is being read. The label is one of a fixed set —
/// never the model's own words.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Heartbeat {
    document_id: String,
    page: usize,
    label: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    document_id: String,
    /// `page` | `done` | `failed`
    phase: String,
    page: usize,
    total: usize,
    blocks: usize,
    cost_usd: f64,
    message: Option<String>,
}

fn read_transcript(id: &str) -> Result<ir::Transcript, String> {
    let raw = fs::read_to_string(workspace::document_dir(id).join(TRANSCRIPT_FILE))
        .map_err(|_| "Ce document n'a pas encore été transcrit.".to_string())?;
    serde_json::from_str(&raw).map_err(|e| format!("Transcription illisible : {e}"))
}

fn write_transcript(id: &str, transcript: &ir::Transcript) -> Result<(), String> {
    let serialised = serde_json::to_string_pretty(transcript).map_err(|e| e.to_string())?;
    fs::write(
        workspace::document_dir(id).join(TRANSCRIPT_FILE),
        serialised,
    )
    .map_err(|e| format!("Écriture de la transcription : {e}"))
}

/// Stops a running read and kills the processes already working.
#[tauri::command]
fn cancel_transcription(id: String) -> usize {
    let stopped = runs::cancel(&runs::reading(&id));
    logbus::warn(
        "claude",
        format!("Lecture annulée — {stopped} processus arrêté(s)"),
    );
    stopped
}

/// Stops a running batch of corrections.
#[tauri::command]
fn cancel_corrections(id: String) -> usize {
    let stopped = runs::cancel(&runs::correcting(&id));
    logbus::warn(
        "claude",
        format!("Corrections annulées — {stopped} processus arrêté(s)"),
    );
    stopped
}

#[tauri::command]
fn load_transcript(id: String) -> Option<ir::Transcript> {
    read_transcript(&id).ok()
}

// ---------------------------------------------------------------------------
// Review
// ---------------------------------------------------------------------------

/// Applies a manual edit to one block.
///
/// The teacher's hand always wins: nothing here calls the model, and the block
/// is marked reviewed so it stops being flagged.
/// Which courses are being read right now.
#[tauri::command]
fn reading_documents() -> Vec<String> {
    runs::active_readings()
}

#[tauri::command]
fn save_block(id: String, block: ir::Block) -> Result<(), String> {
    let mut transcript = read_transcript(&id)?;
    let target = transcript
        .pages
        .iter_mut()
        .flat_map(|p| p.blocks.iter_mut())
        .find(|b| b.id == block.id)
        .ok_or("Bloc introuvable.")?;

    let note = target.note.clone();
    // How far the class has got is not part of what the editor shows, so a
    // round trip through it must not quietly clear the boundary and let a
    // partial export run to the end of the course.
    let taught_end = target.taught_end;
    *target = block;
    // A manual edit does not discard a pending instruction: the teacher may
    // have fixed the wording and still want the diagram redone.
    target.note = note;
    target.taught_end = taught_end;
    target.reviewed = true;

    logbus::info("workspace", format!("Bloc {} modifié à la main", target.id));
    write_transcript(&id, &transcript)
}

/// Replaces one block with two, keeping everything else about it.
///
/// The two bodies come from the interface rather than an offset into the
/// original: a character index means different things in JavaScript and in
/// Rust, and an accent in the wrong place would cut a word in half.
///
/// Returns the id of the new second block.
fn split_in_transcript(
    transcript: &mut ir::Transcript,
    block_id: &str,
    head: &str,
    tail: &str,
) -> Result<String, String> {
    if head.trim().is_empty() || tail.trim().is_empty() {
        return Err("Une des deux moitiés serait vide.".into());
    }

    let page = transcript
        .pages
        .iter_mut()
        .find(|page| page.blocks.iter().any(|block| block.id == block_id))
        .ok_or("Bloc introuvable.")?;
    let at = page
        .blocks
        .iter()
        .position(|block| block.id == block_id)
        .ok_or("Bloc introuvable.")?;

    // The second half inherits what describes the passage — its kind, who it is
    // for, how sure the model was — but not what identifies this one instance:
    // a title, a handwritten number and a pending instruction all belong to the
    // half that keeps them.
    let mut second = page.blocks[at].clone();
    second.latex = tail.trim().to_string();
    second.title = None;
    second.number = None;
    second.note = None;

    page.blocks[at].latex = head.trim().to_string();
    // The class covered the whole passage, so the boundary belongs after both
    // halves — the clone already carries it, the first half must let it go.
    page.blocks[at].taught_end = false;
    page.blocks.insert(at + 1, second);

    // Ids encode the position, so the whole page is renumbered.
    let number = page.number;
    for (index, block) in page.blocks.iter_mut().enumerate() {
        block.id = format!("p{:02}-b{:02}", number, index + 1);
    }
    Ok(page.blocks[at + 1].id.clone())
}

/// Puts new passages after `after_block_id`, or at the very start when it is
/// empty, and renumbers the page.
fn insert_in_transcript(
    transcript: &mut ir::Transcript,
    after_block_id: &str,
    fresh: Vec<ir::Block>,
) -> Result<(), String> {
    if fresh.is_empty() {
        return Err("Rien à insérer.".into());
    }

    let (page_index, at) = if after_block_id.is_empty() {
        (0, 0)
    } else {
        let page_index = transcript
            .pages
            .iter()
            .position(|page| page.blocks.iter().any(|b| b.id == after_block_id))
            .ok_or("Bloc introuvable.")?;
        let at = transcript.pages[page_index]
            .blocks
            .iter()
            .position(|b| b.id == after_block_id)
            .ok_or("Bloc introuvable.")?
            + 1;
        (page_index, at)
    };

    let page = transcript.pages.get_mut(page_index).ok_or("Ce cours n'a aucune page.")?;
    for (offset, block) in fresh.into_iter().enumerate() {
        page.blocks.insert(at + offset, block);
    }

    let number = page.number;
    for (index, block) in page.blocks.iter_mut().enumerate() {
        block.id = format!("p{:02}-b{:02}", number, index + 1);
    }
    Ok(())
}

/// Adds a passage the teacher wrote themselves.
#[tauri::command]
fn insert_block(
    id: String,
    after_block_id: String,
    kind: String,
    title: Option<String>,
    latex: String,
) -> Result<ir::Transcript, String> {
    if latex.trim().is_empty() && title.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err("Ce passage est vide.".into());
    }

    let mut transcript = read_transcript(&id)?;
    let block = ir::Block {
        id: String::new(),
        kind,
        title: title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()),
        number: None,
        latex: latex.trim().to_string(),
        // Written by hand, so there is nothing for the model to be unsure about
        // and nothing left to review.
        confidence: 1.0,
        doubt: None,
        audience: vec!["teacher".into(), "student".into()],
        align: None,
        note: None,
        taught_end: false,
        reviewed: true,
    };
    insert_in_transcript(&mut transcript, &after_block_id, vec![block])?;
    write_transcript(&id, &transcript)?;
    logbus::info("workspace", format!("Passage ajouté après {after_block_id}"));
    Ok(transcript)
}

/// Adds a photograph as a page where the gap is, then reads it.
///
/// The photograph joins the others: it appears in the Photos step and in the
/// course folder, at the position the teacher chose. Pages and transcript move
/// together — everything numbered after it shifts, block ids included — because
/// every part of the app relies on the two staying in step.
#[tauri::command]
async fn insert_from_photo(
    app: AppHandle,
    id: String,
    after_block_id: String,
    source: String,
    model: String,
) -> Result<ir::Transcript, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut transcript = read_transcript(&id).unwrap_or(ir::Transcript {
            version: 1,
            pages: Vec::new(),
        });

        // The new page follows the one holding the passage it was asked for.
        let after_page = if after_block_id.is_empty() {
            0
        } else {
            transcript
                .pages
                .iter()
                .find(|page| page.blocks.iter().any(|b| b.id == after_block_id))
                .map(|page| page.number)
                .ok_or("Bloc introuvable.")?
        };
        let at = after_page + 1;

        workspace::insert_page(&id, std::path::Path::new(&source), at)?;

        // Everything after the new page moves up one, ids included.
        for page in &mut transcript.pages {
            if page.number >= at {
                page.number += 1;
                for (index, block) in page.blocks.iter_mut().enumerate() {
                    block.id = format!("p{:02}-b{:02}", page.number, index + 1);
                }
            }
        }

        let dir = workspace::document_dir(&id);
        let files = workspace::page_files(&id);
        let name = files.get(at - 1).cloned().ok_or("La page insérée est introuvable.")?;

        let document = workspace::load(&id)?;
        let rules = settings::combined_rules(&document.template_id, &document.reading_rules);
        let job = runs::reading(&id);
        runs::begin(&job);

        let _ = app.emit(
            "page-state",
            PageState {
                document_id: id.clone(),
                page: at,
                state: "reading".into(),
                blocks: 0,
                message: None,
            },
        );

        let outcome = recognizer::transcribe_page(
            &job,
            &dir,
            at,
            &format!("pages/{name}"),
            &format!("It is page {at} of the course."),
            &model,
            &rules,
            &|label| {
                let _ = app.emit(
                    "heartbeat",
                    Heartbeat { document_id: id.clone(), page: at, label: label.to_string() },
                );
            },
        );
        runs::finish(&job);

        // The photograph stays whatever the reading did: it is in the course
        // now, and a failed reading is retried from the Lecture step.
        let outcome = outcome?;

        transcript.pages.push(outcome.page);
        transcript.pages.sort_by_key(|page| page.number);
        write_transcript(&id, &transcript)?;

        if let Ok(mut document) = workspace::load(&id) {
            document.cost_usd += outcome.cost_usd;
            document.updated_at = workspace::now_ms();
            let _ = workspace::save(&document);
        }

        let _ = app.emit(
            "page-state",
            PageState {
                document_id: id.clone(),
                page: at,
                state: "done".into(),
                blocks: transcript
                    .pages
                    .iter()
                    .find(|p| p.number == at)
                    .map(|p| p.blocks.len())
                    .unwrap_or(0),
                message: None,
            },
        );
        Ok(transcript)
    })
    .await
    .map_err(|e| format!("Lecture de la page interrompue : {e}"))?
}

/// Takes one passage out and renumbers the page after it.
fn remove_in_transcript(transcript: &mut ir::Transcript, block_id: &str) -> Result<(), String> {
    let page_index = transcript
        .pages
        .iter()
        .position(|page| page.blocks.iter().any(|block| block.id == block_id))
        .ok_or("Bloc introuvable.")?;
    let at = transcript.pages[page_index]
        .blocks
        .iter()
        .position(|block| block.id == block_id)
        .ok_or("Bloc introuvable.")?;

    // Deleting the passage the class stopped on would take the boundary with
    // it, and the next export would run to the end of the course without
    // saying so. The lesson still ended where it ended: the mark steps back.
    let carried = transcript.pages[page_index].blocks[at].taught_end;

    let page = &mut transcript.pages[page_index];
    page.blocks.remove(at);

    let number = page.number;
    for (index, block) in page.blocks.iter_mut().enumerate() {
        block.id = format!("p{:02}-b{:02}", number, index + 1);
    }

    if carried {
        if at > 0 {
            transcript.pages[page_index].blocks[at - 1].taught_end = true;
        } else if let Some(previous) = transcript.pages[..page_index]
            .iter_mut()
            .rev()
            .find_map(|page| page.blocks.last_mut())
        {
            // First block of its page: the lesson ended on the page before.
            previous.taught_end = true;
        }
        // Nothing precedes it at all, so the class covered nothing: no mark.
    }
    Ok(())
}

/// Removes a passage from the transcript.
///
/// A reading sometimes produces a heading with nothing under it, and a manual
/// edit can leave a passage empty. The photograph is untouched: only the
/// transcription loses it, and a fresh reading brings it back.
#[tauri::command]
fn delete_block(id: String, block_id: String) -> Result<ir::Transcript, String> {
    let mut transcript = read_transcript(&id)?;
    remove_in_transcript(&mut transcript, &block_id)?;
    write_transcript(&id, &transcript)?;
    logbus::info("workspace", format!("Passage {block_id} supprimé"));
    Ok(transcript)
}

/// Splits one passage in two, so its halves can be treated separately.
///
/// A worked example whose statement and answer were read as one block cannot
/// be given to the students without its answer; split, the second half can be
/// marked teacher-only on its own.
#[tauri::command]
fn split_block(id: String, block_id: String, head: String, tail: String) -> Result<ir::Transcript, String> {
    let mut transcript = read_transcript(&id)?;
    let created = split_in_transcript(&mut transcript, &block_id, &head, &tail)?;
    write_transcript(&id, &transcript)?;
    logbus::info(
        "workspace",
        format!("Passage {block_id} scindé — nouveau bloc {created}"),
    );
    Ok(transcript)
}

/// Marks the passage the class stopped on, or clears the mark with `None`.
///
/// Not an export setting but a fact about the course, which is why it lives in
/// the transcript and is set from the review: it changes once a week, when the
/// lesson ends, and every export afterwards reads it.
#[tauri::command]
fn set_taught_end(id: String, block_id: Option<String>) -> Result<ir::Transcript, String> {
    let mut transcript = read_transcript(&id)?;
    ir::mark_taught_end(&mut transcript, block_id.as_deref())?;
    write_transcript(&id, &transcript)?;

    match block_id {
        Some(block) => logbus::info(
            "workspace",
            format!("Classe arrêtée après {block} — {} passage(s) vus", ir::taught_count(&transcript).unwrap_or(0)),
        ),
        None => logbus::info("workspace", "Point d'arrêt retiré".to_string()),
    }
    Ok(transcript)
}

/// Attaches or clears the teacher's instruction for one block.
#[tauri::command]
fn set_block_note(id: String, block_id: String, note: Option<String>) -> Result<(), String> {
    let mut transcript = read_transcript(&id)?;
    let target = transcript
        .pages
        .iter_mut()
        .flat_map(|p| p.blocks.iter_mut())
        .find(|b| b.id == block_id)
        .ok_or("Bloc introuvable.")?;

    target.note = note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    write_transcript(&id, &transcript)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CorrectionProgress {
    document_id: String,
    /// `block` | `done` | `failed`
    phase: String,
    block_id: String,
    done: usize,
    total: usize,
    message: Option<String>,
}

/// Re-runs every annotated block, then saves.
///
/// Corrections are targeted: each one resumes its page's session, so the cost
/// is a fraction of re-reading the page. Nothing else in the transcript moves.
#[tauri::command]
async fn apply_corrections(
    app: AppHandle,
    id: String,
    model: String,
) -> Result<ir::Transcript, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = workspace::document_dir(&id);
        let files = workspace::page_files(&id);
        let mut transcript = read_transcript(&id)?;

        let pending: Vec<(usize, usize)> = transcript
            .pages
            .iter()
            .enumerate()
            .flat_map(|(page_index, page)| {
                page.blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, block)| block.note.is_some())
                    .map(move |(block_index, _)| (page_index, block_index))
            })
            .collect();

        if pending.is_empty() {
            return Err("Aucun bloc annoté.".to_string());
        }

        let total = pending.len();
        logbus::info("claude", format!("Correction de {total} bloc(s) annoté(s)"));

        let job = runs::correcting(&id);
        runs::begin(&job);

        let mut failures = Vec::new();
        let mut stopped_early = false;
        let mut spent = 0.0_f64;
        for (done, (page_index, block_index)) in pending.into_iter().enumerate() {
            if runs::is_cancelled(&job) {
                stopped_early = true;
                break;
            }
            let page_number = transcript.pages[page_index].number;
            let session = transcript.pages[page_index].session_id.clone();
            let block = transcript.pages[page_index].blocks[block_index].clone();
            let note = block.note.clone().unwrap_or_default();
            let image = files
                .get(page_number.saturating_sub(1))
                .cloned()
                .unwrap_or_default();

            // Announced before the work, not only after it: correcting one
            // passage takes about as long as reading a page, and nothing was
            // emitted until the first one landed.
            let _ = app.emit(
                "correction",
                CorrectionProgress {
                    document_id: id.clone(),
                    phase: "start".into(),
                    block_id: block.id.clone(),
                    done,
                    total,
                    message: None,
                },
            );

            match recognizer::correct_block(
                &job,
                &dir,
                page_number,
                &image,
                session.as_deref(),
                &block,
                &note,
                &model,
            ) {
                Ok((corrected, cost)) => {
                    transcript.pages[page_index].blocks[block_index] = corrected;
                    spent += cost;
                    let _ = app.emit(
                        "correction",
                        CorrectionProgress {
                            document_id: id.clone(),
                            phase: "block".into(),
                            block_id: block.id.clone(),
                            done: done + 1,
                            total,
                            message: None,
                        },
                    );
                }
                Err(error) if error == recognizer::CANCELLED => {
                    stopped_early = true;
                    break;
                }
                Err(error) => {
                    failures.push(format!("{} : {error}", block.id));
                    let _ = app.emit(
                        "correction",
                        CorrectionProgress {
                            document_id: id.clone(),
                            phase: "failed".into(),
                            block_id: block.id.clone(),
                            done: done + 1,
                            total,
                            message: Some(error),
                        },
                    );
                }
            }
        }

        runs::finish(&job);

        // Saved even on partial failure or cancellation: corrections that did
        // land must not be thrown away because a later one broke or was stopped.
        write_transcript(&id, &transcript)?;

        // Corrections cost real quota too; the running total follows.
        if spent > 0.0 {
            if let Ok(mut document) = workspace::load(&id) {
                document.cost_usd += spent;
                document.updated_at = workspace::now_ms();
                let _ = workspace::save(&document);
            }
        }

        let _ = app.emit(
            "correction",
            CorrectionProgress {
                document_id: id.clone(),
                phase: if stopped_early { "cancelled".into() } else { "done".to_string() },
                block_id: String::new(),
                done: total - failures.len(),
                total,
                message: failures.first().cloned(),
            },
        );

        if !stopped_early && !failures.is_empty() && failures.len() == total {
            return Err(failures.join(" · "));
        }
        Ok(transcript)
    })
    .await
    .map_err(|e| format!("Corrections interrompues : {e}"))?
}

/// Reads every page of a document and writes `transcript.json`.
///
/// Pages run one at a time on purpose: the user's subscription has rolling
/// windows, and a burst of parallel calls buys little for a handful of pages
/// while making quota exhaustion much easier to hit.
#[tauri::command]
async fn transcribe_document(
    app: AppHandle,
    id: String,
    model: String,
) -> Result<ir::Transcript, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let document = workspace::load(&id)?;
        let dir = workspace::document_dir(&id);
        let files = workspace::page_files(&id);
        if files.is_empty() {
            return Err("Ce document n'a aucune page.".to_string());
        }

        let rules = settings::combined_rules(&document.template_id, &document.reading_rules);
        let total = files.len();
        let job = runs::reading(&id);
        runs::begin(&job);
        // Pages are independent, so they read concurrently — but each `claude`
        // weighs hundreds of megabytes, and three of them froze an 8 GB
        // machine. The width follows the memory unless the teacher chose one.
        let chosen = settings::load().concurrent_pages;
        let concurrency = if chosen >= 1 {
            (chosen as usize).min(4)
        } else {
            machine::auto_concurrency()
        }
        .min(total);
        logbus::detail(
            "claude",
            format!("Lecture de {total} page(s), {concurrency} en parallèle, modèle {model}"),
            if chosen >= 1 {
                "parallélisme fixé dans les réglages".to_string()
            } else {
                match machine::total_memory_gb() {
                    Some(gb) => format!("automatique — {gb:.0} Go de mémoire détectés"),
                    None => "automatique — mémoire inconnue".to_string(),
                }
            },
        );

        // Timed end to end: the gap between the last page's banner and the
        // review screen appearing was reported as almost a minute, and guessing
        // where it goes from the code alone was not possible.
        let started = std::time::Instant::now();

        let next = AtomicUsize::new(0);
        let done = AtomicUsize::new(0);
        let spent = Mutex::new(0.0f64);
        let slots: Vec<Mutex<Option<Result<recognizer::PageOutcome, String>>>> =
            (0..total).map(|_| Mutex::new(None)).collect();

        std::thread::scope(|scope| {
            for _ in 0..concurrency {
                scope.spawn(|| loop {
                    if runs::is_cancelled(&job) {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= total {
                        break;
                    }
                    let number = index + 1;
                    let _ = app.emit(
                        "page-state",
                        PageState {
                            document_id: id.clone(),
                            page: number,
                            state: "reading".into(),
                            blocks: 0,
                            message: None,
                        },
                    );

                    let outcome = recognizer::transcribe_page(
                        &job,
                        &dir,
                        number,
                        &format!("pages/{}", files[index]),
                        &format!("It is page {number} of the course."),
                        &model,
                        &rules,
                        &|label| {
                            let _ = app.emit(
                                "heartbeat",
                                Heartbeat {
                                    document_id: id.clone(),
                                    page: number,
                                    label: label.to_string(),
                                },
                            );
                        },
                    );

                    let (phase, blocks, message) = match &outcome {
                        Ok(page) => {
                            *spent.lock().unwrap() += page.cost_usd;
                            ("page", page.page.blocks.len(), None)
                        }
                        Err(error) if error == recognizer::CANCELLED => ("cancelled", 0, None),
                        Err(error) => {
                            logbus::error("claude", format!("Page {number} : {error}"));
                            ("failed", 0, Some(error.clone()))
                        }
                    };

                    let _ = app.emit(
                        "page-state",
                        PageState {
                            document_id: id.clone(),
                            page: number,
                            state: match phase {
                                "page" => "done".into(),
                                "cancelled" => "cancelled".into(),
                                _ => "failed".into(),
                            },
                            blocks,
                            message: message.clone(),
                        },
                    );

                    *slots[index].lock().unwrap() = Some(outcome);
                    let completed = done.fetch_add(1, Ordering::SeqCst) + 1;

                    let _ = app.emit(
                        "transcription",
                        Progress {
                            document_id: id.clone(),
                            phase: phase.into(),
                            page: completed,
                            total,
                            blocks,
                            cost_usd: *spent.lock().unwrap(),
                            message,
                        },
                    );
                });
            }
        });

        let cancelled = runs::is_cancelled(&job);
        runs::finish(&job);
        let pages_read = started.elapsed();

        // Pages already read are kept even when cancelled: they cost real
        // quota, and each page stands on its own.
        let mut transcript = ir::Transcript { version: 1, pages: Vec::new() };
        for slot in slots {
            match slot.into_inner().unwrap() {
                Some(Ok(outcome)) => transcript.pages.push(outcome.page),
                Some(Err(error)) if error == recognizer::CANCELLED => {}
                Some(Err(error)) if cancelled => {
                    logbus::debug("claude", "Erreur ignorée après annulation", error);
                }
                Some(Err(error)) => return Err(error),
                None if cancelled => {}
                None => return Err("Une page n'a pas été traitée.".to_string()),
            }
        }
        transcript.pages.sort_by_key(|page| page.number);
        let spent = *spent.lock().unwrap_or_else(|e| e.into_inner());

        if transcript.pages.is_empty() {
            return Err("Lecture annulée avant la première page.".to_string());
        }

        // Written before returning: a transcription that cost real quota must
        // survive the app being closed a second later.
        let serialised =
            serde_json::to_string_pretty(&transcript).map_err(|e| e.to_string())?;
        fs::write(dir.join(TRANSCRIPT_FILE), serialised)
            .map_err(|e| format!("Écriture de la transcription : {e}"))?;
        logbus::detail(
            "workspace",
            "Transcription enregistrée",
            dir.join(TRANSCRIPT_FILE).to_string_lossy().to_string(),
        );
        logbus::detail(
            "claude",
            format!(
                "Lecture terminée en {:.1} s",
                started.elapsed().as_secs_f64()
            ),
            format!(
                "{:.1} s de lecture des pages, {:.1} s d'écriture",
                pages_read.as_secs_f64(),
                (started.elapsed() - pages_read).as_secs_f64()
            ),
        );

        let mut document = document;
        document.status = "review".into();
        document.cost_usd += spent;
        document.updated_at = workspace::now_ms();
        workspace::save(&document)?;

        let _ = app.emit(
            "transcription",
            Progress {
                document_id: id.clone(),
                phase: if cancelled { "cancelled".into() } else { "done".to_string() },
                page: total,
                total,
                blocks: transcript.pages.iter().map(|p| p.blocks.len()).sum(),
                cost_usd: spent,
                message: None,
            },
        );

        Ok(transcript)
    })
    .await
    .map_err(|e| format!("Transcription interrompue : {e}"))?
}

// ---------------------------------------------------------------------------
// Rendering and compilation
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildResult {
    tex_path: String,
    pdf_path: Option<String>,
    error: Option<String>,
}

/// Renders the `.tex` for one audience, then compiles it.
///
/// A compile failure is not an error of this command: the `.tex` still exists
/// and is the thing the user will fix, so it is returned alongside the message.
///
/// `taught_only` stops the document after the passage the class reached — the
/// handout sent the evening of the lesson. Such a build is a copy taken along
/// the way, not the course: it writes its own file so it cannot overwrite the
/// complete PDF, and leaves the course's own state alone.
#[tauri::command]
async fn build_document(
    id: String,
    audience: String,
    taught_only: bool,
) -> Result<BuildResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let document = workspace::load(&id)?;
        let root = workspace::root();
        let dir = workspace::document_dir(&id);

        let transcript: ir::Transcript = fs::read_to_string(dir.join(TRANSCRIPT_FILE))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .ok_or("Ce document n'a pas encore été transcrit.")?;

        // Rendering would happily treat an unmarked course as "all of it", and
        // that is the one wrong answer here: the teacher asked for the part
        // already taught, and a mail cannot be recalled.
        if taught_only && ir::taught_end(&transcript).is_none() {
            return Err(
                "Ce cours n'a pas de point d'arrêt : marquez d'abord où la classe s'est arrêtée."
                    .into(),
            );
        }

        let template = templates::load(&root, &document.template_id)
            .ok_or("Modèle introuvable.".to_string())?;

        let tex = render::render_document(
            &root,
            &template,
            &transcript,
            &document.title,
            &audience,
            taught_only,
        )
        .map_err(|e| format!("Rendu impossible : {e}"))?;

        let suffix = if taught_only { "-partiel" } else { "" };
        let name = format!("{}-{}{}.tex", document.id, audience, suffix);
        let tex_path = dir.join(&name);
        fs::write(&tex_path, tex).map_err(|e| format!("Écriture du .tex : {e}"))?;

        match latex::compile(&dir, &name) {
            Ok(pdf) => {
                // A compiled PDF is what "ready" means to the course list, and
                // remembering the file lets "Ouvrir le PDF" skip a rebuild.
                // Only a complete one: a partial build would make the course
                // read as finished and point "Ouvrir le PDF" at a document
                // that stops halfway through.
                if !taught_only {
                    let mut document = document;
                    document.status = "ready".into();
                    document.last_pdf = pdf
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string());
                    document.updated_at = workspace::now_ms();
                    let _ = workspace::save(&document);
                }

                Ok(BuildResult {
                    tex_path: tex_path.to_string_lossy().to_string(),
                    pdf_path: Some(pdf.to_string_lossy().to_string()),
                    error: None,
                })
            }
            Err(error) => Ok(BuildResult {
                tex_path: tex_path.to_string_lossy().to_string(),
                pdf_path: None,
                error: Some(error),
            }),
        }
    })
    .await
    .map_err(|e| format!("Construction interrompue : {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            logbus::init(app.handle().clone());

            // Workbook, layout migration and bundled template must all be
            // settled before the first render.
            if let Ok(root) = workspace::ensure_root() {
                match workspace::migrate_layout() {
                    Ok(0) => {}
                    Ok(moved) => logbus::info(
                        "workspace",
                        format!("{moved} cours déplacé(s) dans Courses/"),
                    ),
                    Err(error) => {
                        logbus::error("workspace", format!("Migration impossible : {error}"))
                    }
                }
                let _ = workspace::ensure_courses_dir();
                let _ = templates::seed(&root);
                logbus::detail("app", "Classeur prêt", root.to_string_lossy().to_string());
            }

            let console = MenuItem::with_id(
                app,
                "toggle-console",
                "Console",
                true,
                Some("CmdOrCtrl+Alt+C"),
            )?;
            let tools = Submenu::with_items(app, "Outils", true, &[&console])?;
            let menu = Menu::default(app.handle())?;
            menu.append(&tools)?;
            app.set_menu(menu)?;

            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == "toggle-console" {
                let _ = app.emit("toggle-console", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            check_environment,
            list_documents,
            list_trash,
            restore_document,
            purge_document,
            os_platform,
            get_document,
            document_pages,
            document_page_paths,
            delete_document,
            rename_document,
            add_pages,
            remove_page,
            reorder_pages,
            set_reading_rules,
            get_settings,
            save_settings,
            save_template,
            duplicate_template,
            delete_template,
            read_template_preamble,
            write_template_preamble,
            check_template,
            list_templates,
            create_document,
            preview_preamble,
            render_figure,
            install_engine,
            install_claude,
            open_claude_login,
            remove_engine,
            load_transcript,
            cancel_transcription,
            cancel_corrections,
            save_block,
            split_block,
            delete_block,
            insert_block,
            insert_from_photo,
            reading_documents,
            set_block_note,
            set_taught_end,
            apply_corrections,
            transcribe_document,
            build_document,
            workspace_path,
            reveal_workspace,
            reveal_path,
            open_course_pdf,
            open_url,
            updates_configured,
            logs,
            clear_logs,
            log_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_of(kind: &str, latex: &str) -> ir::Block {
        ir::Block {
            id: String::new(),
            kind: kind.into(),
            title: None,
            number: None,
            latex: latex.into(),
            confidence: 1.0,
            doubt: None,
            audience: Vec::new(),
            align: None,
            note: None,
            taught_end: false,
            reviewed: true,
        }
    }

    fn page(number: usize, blocks: usize) -> ir::Page {
        ir::Page {
            number,
            session_id: None,
            blocks: (1..=blocks)
                .map(|index| ir::Block {
                    id: format!("p{number:02}-b{index:02}"),
                    kind: "text".into(),
                    title: None,
                    number: None,
                    latex: format!("page {number} bloc {index}"),
                    confidence: 1.0,
                    doubt: None,
                    audience: Vec::new(),
                    align: None,
                    note: None,
                    taught_end: false,
                    reviewed: false,
                })
                .collect(),
        }
    }

    /// A worked example read as one block: split, the answer can be kept for
    /// the teacher's copy while the statement goes to everyone.
    #[test]
    fn splitting_keeps_what_describes_the_passage_and_drops_what_names_it() {
        let mut transcript = ir::Transcript { version: 1, pages: vec![page(1, 3)] };
        transcript.pages[0].blocks[1].kind = "example".into();
        transcript.pages[0].blocks[1].title = Some("Chasles".into());
        transcript.pages[0].blocks[1].number = Some("2".into());
        transcript.pages[0].blocks[1].note = Some("à revoir".into());
        transcript.pages[0].blocks[1].audience = vec!["student".into()];
        transcript.pages[0].blocks[1].confidence = 0.42;
        transcript.pages[0].blocks[1].latex = "Énoncé.\n\nCorrection.".into();

        let created =
            split_in_transcript(&mut transcript, "p01-b02", "Énoncé.", "Correction.").unwrap();

        let blocks = &transcript.pages[0].blocks;
        assert_eq!(blocks.len(), 4, "one block became two");
        assert_eq!(created, "p01-b03");

        assert_eq!(blocks[1].latex, "Énoncé.");
        assert_eq!(blocks[2].latex, "Correction.");

        // What describes the passage is inherited.
        assert_eq!(blocks[2].kind, "example");
        assert_eq!(blocks[2].audience, vec!["student".to_string()]);
        assert_eq!(blocks[2].confidence, 0.42);

        // What names this one instance is not.
        assert_eq!(blocks[1].title.as_deref(), Some("Chasles"));
        assert_eq!(blocks[2].title, None);
        assert_eq!(blocks[2].number, None);
        assert_eq!(blocks[2].note, None);

        // Ids follow the new positions, including the untouched blocks after.
        let ids: Vec<&str> = blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["p01-b01", "p01-b02", "p01-b03", "p01-b04"]);
    }

    /// The boundary is the one piece of state a wrong answer cannot take back:
    /// a handout is mailed to a class. Deleting the passage it sits on must not
    /// silently leave the course unbounded.
    #[test]
    fn deleting_the_passage_the_class_stopped_on_steps_the_mark_back() {
        let mut transcript = ir::Transcript { version: 1, pages: vec![page(1, 3)] };
        ir::mark_taught_end(&mut transcript, Some("p01-b02")).unwrap();

        remove_in_transcript(&mut transcript, "p01-b02").unwrap();

        assert_eq!(
            ir::taught_end(&transcript).map(|b| b.latex.as_str()),
            Some("page 1 bloc 1"),
            "the lesson still ended where it ended"
        );
        assert_eq!(ir::taught_count(&transcript), Some(1));
    }

    /// The first block of a page: the lesson ended on the page before.
    #[test]
    fn the_mark_crosses_back_over_a_page_boundary() {
        let mut transcript =
            ir::Transcript { version: 1, pages: vec![page(1, 2), page(2, 2)] };
        ir::mark_taught_end(&mut transcript, Some("p02-b01")).unwrap();

        remove_in_transcript(&mut transcript, "p02-b01").unwrap();

        assert_eq!(
            ir::taught_end(&transcript).map(|b| b.id.as_str()),
            Some("p01-b02")
        );
    }

    /// Nothing precedes it, so the class covered nothing — and an empty mark
    /// beats a mark on someone else's passage.
    #[test]
    fn deleting_the_only_covered_passage_clears_the_mark() {
        let mut transcript = ir::Transcript { version: 1, pages: vec![page(1, 3)] };
        ir::mark_taught_end(&mut transcript, Some("p01-b01")).unwrap();

        remove_in_transcript(&mut transcript, "p01-b01").unwrap();

        assert!(ir::taught_end(&transcript).is_none());
    }

    /// The whole passage was taught, so the boundary belongs after both halves
    /// — and on exactly one of them.
    #[test]
    fn splitting_the_last_taught_passage_moves_the_mark_to_its_second_half() {
        let mut transcript = ir::Transcript { version: 1, pages: vec![page(1, 3)] };
        transcript.pages[0].blocks[1].latex = "Énoncé.\n\nCorrection.".into();
        ir::mark_taught_end(&mut transcript, Some("p01-b02")).unwrap();

        split_in_transcript(&mut transcript, "p01-b02", "Énoncé.", "Correction.").unwrap();

        let marked: Vec<&str> = transcript.pages[0]
            .blocks
            .iter()
            .filter(|b| b.taught_end)
            .map(|b| b.latex.as_str())
            .collect();
        assert_eq!(marked, vec!["Correction."], "one mark, on the second half");
    }

    /// Ids encode position, so a mark stored as an id would follow the position
    /// rather than the passage. This is the regression the whole design avoids.
    #[test]
    fn the_mark_follows_the_passage_through_an_insertion_before_it() {
        let mut transcript = ir::Transcript { version: 1, pages: vec![page(1, 3)] };
        ir::mark_taught_end(&mut transcript, Some("p01-b02")).unwrap();

        insert_in_transcript(&mut transcript, "p01-b01", vec![block_of("text", "ajouté")])
            .unwrap();

        let marked = ir::taught_end(&transcript).expect("still marked");
        assert_eq!(marked.latex, "page 1 bloc 2", "same passage");
        assert_eq!(marked.id, "p01-b03", "new position");
        assert_eq!(
            ir::taught_count(&transcript),
            Some(3),
            "the inserted passage falls inside what was taught"
        );
    }

    /// A course reordered by hand carries the mark with the pages.
    #[test]
    fn the_mark_survives_reordering_the_pages() {
        let mut transcript =
            ir::Transcript { version: 1, pages: vec![page(1, 2), page(2, 2)] };
        ir::mark_taught_end(&mut transcript, Some("p02-b02")).unwrap();

        reorder_transcript(&mut transcript, &[2, 1]);

        let marked = ir::taught_end(&transcript).expect("still marked");
        assert_eq!(marked.latex, "page 2 bloc 2");
        assert_eq!(marked.id, "p01-b02");
    }

    /// Ids encode the position, so what follows a removal has to shift.
    #[test]
    fn a_written_passage_lands_where_it_was_asked_for() {
        let mut transcript = ir::Transcript { version: 1, pages: vec![page(1, 3)] };
        let fresh = || vec![block_of("text", "ajouté")];

        insert_in_transcript(&mut transcript, "p01-b02", fresh()).unwrap();

        let bodies: Vec<&str> =
            transcript.pages[0].blocks.iter().map(|b| b.latex.as_str()).collect();
        assert_eq!(
            bodies,
            vec!["page 1 bloc 1", "page 1 bloc 2", "ajouté", "page 1 bloc 3"]
        );
        let ids: Vec<&str> = transcript.pages[0].blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["p01-b01", "p01-b02", "p01-b03", "p01-b04"]);

        // An empty anchor puts it at the very beginning.
        insert_in_transcript(&mut transcript, "", vec![block_of("text", "en tête")]).unwrap();
        assert_eq!(transcript.pages[0].blocks[0].latex, "en tête");
        assert_eq!(transcript.pages[0].blocks[0].id, "p01-b01");

        assert!(insert_in_transcript(&mut transcript, "p09-b01", fresh()).is_err());
        assert!(insert_in_transcript(&mut transcript, "p01-b01", Vec::new()).is_err());
    }

    #[test]
    fn removing_a_passage_renumbers_the_rest_of_its_page() {
        let mut transcript = ir::Transcript {
            version: 1,
            pages: vec![page(1, 3), page(2, 2)],
        };
        transcript.pages[0].blocks[1].latex = "à supprimer".into();

        remove_in_transcript(&mut transcript, "p01-b02").expect("remove");

        let first: Vec<&str> = transcript.pages[0].blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(first, vec!["p01-b01", "p01-b02"]);
        assert_eq!(transcript.pages[0].blocks[1].latex, "page 1 bloc 3");

        // The other page is untouched.
        let second: Vec<&str> = transcript.pages[1].blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(second, vec!["p02-b01", "p02-b02"]);

        assert!(remove_in_transcript(&mut transcript, "p09-b01").is_err());
    }

    #[test]
    fn splitting_refuses_to_produce_an_empty_half() {
        let mut transcript = ir::Transcript { version: 1, pages: vec![page(1, 1)] };
        assert!(split_in_transcript(&mut transcript, "p01-b01", "  ", "reste").is_err());
        assert!(split_in_transcript(&mut transcript, "p01-b01", "début", "\n \n").is_err());
        assert!(split_in_transcript(&mut transcript, "p09-b01", "a", "b").is_err());
        assert_eq!(transcript.pages[0].blocks.len(), 1, "nothing moved");
    }

    /// Block ids encode the page, so moving photographs without moving the
    /// transcript would leave every block pointing at the wrong one.
    #[test]
    fn reordering_moves_the_transcript_and_its_block_ids() {
        let mut transcript = ir::Transcript {
            version: 1,
            pages: vec![page(1, 2), page(2, 1), page(3, 2)],
        };

        // The third photograph becomes the first.
        reorder_transcript(&mut transcript, &[3, 1, 2]);

        let numbers: Vec<usize> = transcript.pages.iter().map(|p| p.number).collect();
        assert_eq!(numbers, vec![1, 2, 3]);
        assert_eq!(transcript.pages[0].blocks[0].latex, "page 3 bloc 1");
        assert_eq!(transcript.pages[0].blocks[0].id, "p01-b01");
        assert_eq!(transcript.pages[1].blocks[0].latex, "page 1 bloc 1");
        assert_eq!(transcript.pages[2].blocks[0].latex, "page 2 bloc 1");
        assert_eq!(transcript.pages[2].blocks[0].id, "p03-b01");
    }

    /// A course read only in part has fewer transcript pages than photographs.
    #[test]
    fn a_partial_transcript_follows_the_pages_it_has() {
        let mut transcript = ir::Transcript {
            version: 1,
            pages: vec![page(1, 1), page(3, 1)],
        };

        reorder_transcript(&mut transcript, &[3, 2, 1]);

        assert_eq!(transcript.pages.len(), 2, "no page is invented");
        assert_eq!(transcript.pages[0].number, 1);
        assert_eq!(transcript.pages[0].blocks[0].latex, "page 3 bloc 1");
        assert_eq!(transcript.pages[1].number, 3);
        assert_eq!(transcript.pages[1].blocks[0].latex, "page 1 bloc 1");
    }
}
