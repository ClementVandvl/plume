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
        .unwrap_or_else(|_| env_check::Environment { tools: Vec::new(), ready: false })
}

#[tauri::command]
fn list_documents() -> Vec<workspace::Document> {
    workspace::list()
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
    *target = block;
    // A manual edit does not discard a pending instruction: the teacher may
    // have fixed the wording and still want the diagram redone.
    target.note = note;
    target.reviewed = true;

    logbus::info("workspace", format!("Bloc {} modifié à la main", target.id));
    write_transcript(&id, &transcript)
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
                Ok(corrected) => {
                    transcript.pages[page_index].blocks[block_index] = corrected;
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

        let rules = settings::combined_rules(&document.reading_rules);
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
                    let outcome = recognizer::transcribe_page(
                        &job,
                        &dir,
                        number,
                        &files[index],
                        &model,
                        &rules,
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

        let mut document = document;
        document.status = "review".into();
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
#[tauri::command]
async fn build_document(id: String, audience: String) -> Result<BuildResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let document = workspace::load(&id)?;
        let root = workspace::root();
        let dir = workspace::document_dir(&id);

        let transcript: ir::Transcript = fs::read_to_string(dir.join(TRANSCRIPT_FILE))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .ok_or("Ce document n'a pas encore été transcrit.")?;

        let template = templates::load(&root, &document.template_id)
            .ok_or("Modèle introuvable.".to_string())?;

        let tex = render::render_document(
            &root,
            &template,
            &transcript,
            &document.title,
            &audience,
        )
        .map_err(|e| format!("Rendu impossible : {e}"))?;

        let name = format!("{}-{}.tex", document.id, audience);
        let tex_path = dir.join(&name);
        fs::write(&tex_path, tex).map_err(|e| format!("Écriture du .tex : {e}"))?;

        match latex::compile(&dir, &name) {
            Ok(pdf) => Ok(BuildResult {
                tex_path: tex_path.to_string_lossy().to_string(),
                pdf_path: Some(pdf.to_string_lossy().to_string()),
                error: None,
            }),
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
            get_document,
            document_pages,
            document_page_paths,
            delete_document,
            rename_document,
            add_pages,
            remove_page,
            set_reading_rules,
            get_settings,
            save_settings,
            save_template,
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
            set_block_note,
            apply_corrections,
            transcribe_document,
            build_document,
            workspace_path,
            reveal_workspace,
            reveal_path,
            open_url,
            updates_configured,
            logs,
            clear_logs,
            log_client
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
