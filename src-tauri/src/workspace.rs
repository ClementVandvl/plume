//! The workbook: the on-disk folder where Plume documents live.
//!
//! A document is a plain sub-folder, readable and movable by hand:
//!
//! ```text
//! Plume/
//!   Courses/
//!     vecteurs/
//!       document.json      metadata
//!       pages/01.jpg ...   the photos, upright and size-capped
//!   Templates/
//!     charte-maths/
//! ```
//!
//! Pages are Plume's own copies — the originals in the photo library are never
//! touched — and they are normalised on the way in: see .
//!
//! No database: if Plume goes away, the work stays.

use crate::logbus;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Extensions accepted on import.
const ACCEPTED: &[&str] = &["jpg", "jpeg", "png", "heic", "heif", "webp", "tif", "tiff"];

/// Formats neither the recogniser nor the webview can open.
///
/// Measured, not assumed: Claude Code's file reader refuses a `.heic` and asks
/// to shell out to a converter, and the webview will not display one either.
/// Since HEIC is what an iPhone produces by default, converting once at import
/// is the only place that fixes every downstream step at the same time.
const NEEDS_CONVERSION: &[&str] = &["heic", "heif"];

/// One way of turning a HEIC into a JPEG.
struct Converter {
    tool: &'static str,
    /// Argument templates; `{in}` and `{out}` are substituted.
    args: &'static [&'static str],
    /// Arguments are a PowerShell script, so paths need PowerShell quoting
    /// rather than being handed straight to `exec`.
    powershell: bool,
}

/// Reads the first frame with the Windows Imaging Component and re-encodes it.
///
/// WIC gains HEIF support from the codec Windows 11 ships by default, so most
/// machines need nothing installed — which matters, because the target user
/// will not go and fetch ImageMagick.
#[cfg(target_os = "windows")]
const WIC_SCRIPT: &str = "Add-Type -AssemblyName PresentationCore; \
$in=[System.IO.File]::OpenRead('{in}'); \
$dec=[System.Windows.Media.Imaging.BitmapDecoder]::Create($in,'None','OnLoad'); \
$enc=New-Object System.Windows.Media.Imaging.JpegBitmapEncoder; \
$enc.QualityLevel=92; \
$enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($dec.Frames[0])); \
$out=[System.IO.File]::Open('{out}','Create'); \
$enc.Save($out); $out.Close(); $in.Close()";

/// Converters, in order of preference, per platform. The first entry of each
/// list is the one that needs nothing installed.
#[cfg(target_os = "macos")]
const CONVERTERS: &[Converter] = &[
    // Ships with macOS.
    Converter { tool: "sips", args: &["-s", "format", "jpeg", "{in}", "--out", "{out}"], powershell: false },
    Converter { tool: "heif-convert", args: &["-q", "92", "{in}", "{out}"], powershell: false },
    Converter { tool: "magick", args: &["{in}", "-quality", "92", "{out}"], powershell: false },
];

#[cfg(target_os = "windows")]
const CONVERTERS: &[Converter] = &[
    Converter { tool: "magick", args: &["{in}", "-quality", "92", "{out}"], powershell: false },
    Converter { tool: "ffmpeg", args: &["-y", "-i", "{in}", "-q:v", "3", "{out}"], powershell: false },
    // Last because it is the slowest, first in practice because it is the only
    // one already present on a stock machine.
    Converter { tool: "powershell", args: &["-NoProfile", "-Command", WIC_SCRIPT], powershell: true },
    Converter { tool: "pwsh", args: &["-NoProfile", "-Command", WIC_SCRIPT], powershell: true },
];

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const CONVERTERS: &[Converter] = &[
    Converter { tool: "heif-convert", args: &["-q", "92", "{in}", "{out}"], powershell: false },
    Converter { tool: "magick", args: &["{in}", "-quality", "92", "{out}"], powershell: false },
    Converter { tool: "ffmpeg", args: &["-y", "-i", "{in}", "-q:v", "3", "{out}"], powershell: false },
];

/// What to tell the teacher when nothing on the machine can do it.
#[cfg(target_os = "windows")]
const NO_CONVERTER_HINT: &str =
    "Installez ImageMagick, ou activez « Extensions d'image HEIF » depuis le Microsoft Store, \
     ou exportez vos photos en JPEG depuis votre iPhone.";
#[cfg(not(target_os = "windows"))]
const NO_CONVERTER_HINT: &str =
    "Installez ImageMagick, ou exportez vos photos en JPEG depuis votre iPhone.";

/// Brings one photo into a course: HEIC conversion if needed, then rotation and
/// resizing, so every stored page is an upright JPEG of a sane size.
///
/// One entry point for both import paths, because a page added later must go
/// through exactly the same treatment as a page added at creation.
fn import_one(source: &Path, destination: &Path) -> Result<(), String> {
    let extension = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if !ACCEPTED.contains(&extension.as_str()) {
        return Err(format!(
            "« {} » n'est pas une image reconnue.",
            source.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    if NEEDS_CONVERSION.contains(&extension.as_str()) {
        // Two steps: the converter produces a JPEG, then the usual pass rotates
        // and resizes it like any other photo.
        let staged = destination.with_extension("converting.jpg");
        convert_to_jpeg(source, &staged)?;
        let outcome = crate::photos::normalise(&staged, destination).map(|_| ());
        let _ = fs::remove_file(&staged);
        return outcome;
    }

    crate::photos::normalise(source, destination).map(|_| ())
}

/// Escapes a path for a single-quoted PowerShell string.
///
/// A file name may legitimately contain an apostrophe, and in PowerShell that
/// would close the string and let the rest of the name run as code.
fn powershell_quote(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn convert_to_jpeg(source: &Path, destination: &Path) -> Result<(), String> {
    for converter in CONVERTERS {
        let Some(binary) = crate::env_check::resolve_tool(converter.tool) else {
            continue;
        };

        let args: Vec<String> = converter
            .args
            .iter()
            .map(|part| {
                if converter.powershell {
                    part.replace("{in}", &powershell_quote(source))
                        .replace("{out}", &powershell_quote(destination))
                } else {
                    match *part {
                        "{in}" => source.to_string_lossy().to_string(),
                        "{out}" => destination.to_string_lossy().to_string(),
                        other => other.to_string(),
                    }
                }
            })
            .collect();

        let ok = std::process::Command::new(&binary)
            .args(&args)
            .output()
            .map(|out| out.status.success() && destination.is_file())
            .unwrap_or(false);

        if ok {
            logbus::detail(
                "workspace",
                format!("Photo convertie en JPEG avec {}", converter.tool),
                destination.to_string_lossy().to_string(),
            );
            return Ok(());
        }
    }

    Err(format!(
        "« {} » est au format HEIC et n'a pas pu être converti. {NO_CONVERTER_HINT}",
        source.file_name().unwrap_or_default().to_string_lossy()
    ))
}

/// `~/Documents/Plume` on macOS and Linux, `%USERPROFILE%\Documents\Plume`
/// on Windows.
pub fn root() -> PathBuf {
    crate::env_check::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("Plume")
}

pub fn ensure_root() -> io::Result<PathBuf> {
    let dir = root();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Where courses live, kept apart from `Templates` so the workbook stays
/// readable as it fills up.
pub fn courses_dir() -> PathBuf {
    root().join("Courses")
}

pub fn ensure_courses_dir() -> io::Result<PathBuf> {
    let dir = courses_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Moves courses that predate the `Courses/` folder into it.
///
/// Only folders holding a `document.json` are touched, so `Templates` and any
/// stray folder of the user's own are left exactly where they are. A course
/// whose name is already taken is left in place rather than overwritten.
pub fn migrate_layout() -> io::Result<usize> {
    let root = ensure_root()?;
    let courses = courses_dir();

    let candidates: Vec<PathBuf> = fs::read_dir(&root)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path != &courses)
        .filter(|path| path.join("document.json").is_file())
        .collect();

    if candidates.is_empty() {
        return Ok(0);
    }
    fs::create_dir_all(&courses)?;

    let mut moved = 0;
    for source in candidates {
        let Some(name) = source.file_name() else { continue };
        let target = courses.join(name);
        if target.exists() {
            logbus::warn(
                "workspace",
                format!(
                    "« {} » existe déjà dans Courses — laissé en place.",
                    name.to_string_lossy()
                ),
            );
            continue;
        }
        match fs::rename(&source, &target) {
            Ok(()) => {
                moved += 1;
                logbus::detail(
                    "workspace",
                    format!("Cours « {} » déplacé", name.to_string_lossy()),
                    target.to_string_lossy().to_string(),
                );
            }
            Err(error) => logbus::error(
                "workspace",
                format!("Déplacement de « {} » impossible : {error}", name.to_string_lossy()),
            ),
        }
    }
    Ok(moved)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub title: String,
    pub template_id: String,
    /// Milliseconds since epoch — formatting into a readable date is the UI's
    /// job, since it knows the user's locale.
    pub created_at: u64,
    pub updated_at: u64,
    pub page_count: usize,
    /// `draft` | `review` | `ready`
    pub status: String,
    /// The teacher's own reading conventions, in their words, appended to the
    /// recogniser's instructions. Empty until they define any.
    #[serde(default)]
    pub reading_rules: String,
}

pub fn document_dir(id: &str) -> PathBuf {
    courses_dir().join(id)
}

pub fn load(id: &str) -> Result<Document, String> {
    let raw = fs::read_to_string(document_dir(id).join("document.json"))
        .map_err(|_| "Document introuvable.".to_string())?;
    serde_json::from_str(&raw).map_err(|e| format!("Manifeste illisible : {e}"))
}

pub fn save(document: &Document) -> Result<(), String> {
    let manifest = serde_json::to_string_pretty(document).map_err(|e| e.to_string())?;
    fs::write(document_dir(&document.id).join("document.json"), manifest)
        .map_err(|e| format!("Écriture du manifeste : {e}"))
}

/// Page image file names, in page order.
pub fn page_files(id: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(document_dir(id).join("pages")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Lists courses, most recently updated first.
///
/// A sub-folder without a readable `document.json` is skipped silently: a stray
/// file must not break the dashboard.
pub fn list() -> Vec<Document> {
    let Ok(dir) = ensure_courses_dir() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut documents: Vec<Document> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| fs::read_to_string(e.path().join("document.json")).ok())
        .filter_map(|raw| serde_json::from_str::<Document>(&raw).ok())
        .collect();

    documents.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    documents
}

/// Safe folder name derived from the title: accents flattened, lowercase,
/// dashes.
fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = true;

    for c in title.trim().chars() {
        let plain = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            other => other,
        };
        let plain = plain.to_ascii_lowercase();

        if plain.is_ascii_alphanumeric() {
            slug.push(plain);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() { "document".to_string() } else { slug }
}

/// Appends a numeric suffix while the folder already exists.
fn unique_id(root: &Path, base: &str) -> String {
    if !root.join(base).exists() {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !root.join(candidate).exists())
        .unwrap_or_else(|| format!("{base}-{}", now_ms()))
}

/// Creates a document and copies the photos into it, in the given order.
///
/// Originals are never moved: Plume works on copies so that a failure here
/// cannot touch the user's photo library.
///
/// Error strings are surfaced verbatim in the UI, hence French.
pub fn create(
    title: &str,
    template_id: &str,
    sources: &[PathBuf],
    on_progress: &dyn Fn(usize, usize),
) -> Result<Document, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Donnez un titre au document.".into());
    }
    if sources.is_empty() {
        return Err("Ajoutez au moins une photo.".into());
    }

    let courses = ensure_courses_dir().map_err(|e| format!("Classeur inaccessible : {e}"))?;
    let id = unique_id(&courses, &slugify(title));
    let dir = courses.join(&id);
    let pages = dir.join("pages");
    fs::create_dir_all(&pages).map_err(|e| format!("Création du dossier : {e}"))?;

    let mut copied = 0usize;
    for (index, source) in sources.iter().enumerate() {
        let extension = source
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if !ACCEPTED.contains(&extension.as_str()) {
            fs::remove_dir_all(&dir).ok();
            return Err(format!(
                "« {} » n'est pas une image reconnue.",
                source.file_name().unwrap_or_default().to_string_lossy()
            ));
        }

        let destination = pages.join(format!("{:02}.jpg", index + 1));
        if let Err(error) = import_one(source, &destination) {
            fs::remove_dir_all(&dir).ok();
            return Err(error);
        }
        copied += 1;
        on_progress(copied, sources.len());
    }

    let now = now_ms();
    let document = Document {
        id: id.clone(),
        title: title.to_string(),
        template_id: template_id.to_string(),
        created_at: now,
        updated_at: now,
        page_count: copied,
        status: "draft".into(),
        reading_rules: String::new(),
    };

    let manifest = serde_json::to_string_pretty(&document)
        .map_err(|e| format!("Sérialisation : {e}"))?;
    if let Err(e) = fs::write(dir.join("document.json"), manifest) {
        fs::remove_dir_all(&dir).ok();
        return Err(format!("Écriture du manifeste : {e}"));
    }

    logbus::detail(
        "workspace",
        format!("Document « {title} » créé — {copied} page(s)"),
        dir.to_string_lossy().to_string(),
    );

    Ok(document)
}

/// Where deleted courses go.
///
/// Never a hard delete: a course is weeks of handwriting plus real quota spent
/// reading it. The folder is moved, and the teacher empties the bin themselves
/// from the Finder if they mean it.
pub fn bin_dir() -> PathBuf {
    root().join("Corbeille")
}

pub fn delete(id: &str) -> Result<PathBuf, String> {
    let source = document_dir(id);
    if !source.is_dir() {
        return Err("Document introuvable.".into());
    }

    let bin = bin_dir();
    fs::create_dir_all(&bin).map_err(|e| format!("Corbeille inaccessible : {e}"))?;

    let target = (0..)
        .map(|n| if n == 0 { bin.join(id) } else { bin.join(format!("{id}-{n}")) })
        .find(|candidate| !candidate.exists())
        .ok_or("Nom libre introuvable dans la corbeille.")?;

    fs::rename(&source, &target).map_err(|e| format!("Mise à la corbeille : {e}"))?;
    logbus::detail(
        "workspace",
        format!("Cours « {id} » mis à la corbeille"),
        target.to_string_lossy().to_string(),
    );
    Ok(target)
}

/// Renames the course for display. The folder keeps its identifier, because
/// the transcript, the figure cache and the produced files all hang off it.
pub fn rename(id: &str, title: &str) -> Result<Document, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Donnez un titre au document.".into());
    }
    let mut document = load(id)?;
    document.title = title.to_string();
    document.updated_at = now_ms();
    save(&document)?;
    Ok(document)
}

/// Appends pages at the end, so existing numbering — and the transcript that
/// depends on it — stays valid.
pub fn add_pages(
    id: &str,
    sources: &[PathBuf],
    on_progress: &dyn Fn(usize, usize),
) -> Result<Document, String> {
    if sources.is_empty() {
        return Err("Aucune photo à ajouter.".into());
    }
    let dir = document_dir(id);
    let pages = dir.join("pages");
    fs::create_dir_all(&pages).map_err(|e| format!("Dossier des pages : {e}"))?;

    let mut next = page_files(id).len();
    for (index, source) in sources.iter().enumerate() {
        next += 1;
        import_one(source, &pages.join(format!("{next:02}.jpg")))?;
        on_progress(index + 1, sources.len());
    }

    let mut document = load(id)?;
    document.page_count = page_files(id).len();
    document.updated_at = now_ms();
    save(&document)?;
    logbus::info("workspace", format!("{} page(s) ajoutée(s) à « {id} »", sources.len()));
    Ok(document)
}

/// Removes one page and closes the gap, renumbering the files.
///
/// The caller is responsible for the transcript: page numbers and block ids
/// encode the position, so they have to move with the files.
pub fn remove_page(id: &str, number: usize) -> Result<Document, String> {
    let names = page_files(id);
    if number == 0 || number > names.len() {
        return Err("Cette page n'existe pas.".into());
    }
    let pages = document_dir(id).join("pages");
    fs::remove_file(pages.join(&names[number - 1]))
        .map_err(|e| format!("Suppression de la page : {e}"))?;

    // Two passes through a temporary name, so renumbering never collides with
    // a file it has not moved yet.
    let remaining: Vec<String> = names
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != number - 1)
        .map(|(_, name)| name.clone())
        .collect();

    for (index, name) in remaining.iter().enumerate() {
        let extension = Path::new(name)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "jpg".into());
        let _ = fs::rename(
            pages.join(name),
            pages.join(format!("tmp-{:02}.{extension}", index + 1)),
        );
    }
    for (index, name) in remaining.iter().enumerate() {
        let extension = Path::new(name)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "jpg".into());
        let _ = fs::rename(
            pages.join(format!("tmp-{:02}.{extension}", index + 1)),
            pages.join(format!("{:02}.{extension}", index + 1)),
        );
    }

    let mut document = load(id)?;
    document.page_count = page_files(id).len();
    document.updated_at = now_ms();
    save(&document)?;
    logbus::info("workspace", format!("Page {number} retirée de « {id} »"));
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_flattens_accents_and_punctuation() {
        assert_eq!(slugify("Vecteurs"), "vecteurs");
        assert_eq!(slugify("Théorème de Pythagore !"), "theoreme-de-pythagore");
        assert_eq!(slugify("  Suites (récurrence)  "), "suites-recurrence");
        assert_eq!(slugify("???"), "document");
    }

    /// HEIC is what an iPhone produces, and neither the recogniser nor the
    /// webview can open one — so the import must hand back a JPEG.
    #[test]
    fn a_converter_is_listed_for_this_platform() {
        assert!(!CONVERTERS.is_empty());
        for converter in CONVERTERS {
            let joined = converter.args.join(" ");
            assert!(joined.contains("{in}"), "{} has no input", converter.tool);
            assert!(joined.contains("{out}"), "{} has no output", converter.tool);
        }
    }

    #[test]
    fn powershell_quoting_neutralises_an_apostrophe() {
        assert_eq!(powershell_quote(Path::new("/tmp/l'ete.heic")), "/tmp/l''ete.heic");

        // The property that matters: every run of quotes is even, so none of
        // them can close the surrounding single-quoted string.
        for name in ["plain.heic", "l'ete.heic", "'''odd.heic", "a''b'c.heic"] {
            let quoted = powershell_quote(Path::new(name));
            let mut run = 0usize;
            for character in quoted.chars() {
                if character == '\'' {
                    run += 1;
                } else {
                    assert_eq!(run % 2, 0, "odd run of quotes in {quoted}");
                    run = 0;
                }
            }
            assert_eq!(run % 2, 0, "odd trailing run of quotes in {quoted}");
        }
    }

    #[test]
    fn heic_is_converted_to_jpeg_on_import() {
        let converter = CONVERTERS
            .iter()
            .any(|c| crate::env_check::resolve_tool(c.tool).is_some());
        if !converter {
            eprintln!("no HEIC converter on this machine, skipping");
            return;
        }

        let dir = std::env::temp_dir().join(format!("plume-heic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // A real HEIC is needed; build one from a generated PNG when possible.
        let png = dir.join("source.png");
        let heic = dir.join("source.heic");
        let made = std::process::Command::new("magick")
            .args(["-size", "64x64", "xc:white", png.to_str().unwrap()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
            && std::process::Command::new("magick")
                .args([png.to_str().unwrap(), heic.to_str().unwrap()])
                .output()
                .map(|o| o.status.success() && heic.is_file())
                .unwrap_or(false);

        if !made {
            eprintln!("cannot build a HEIC fixture here, skipping");
            let _ = fs::remove_dir_all(&dir);
            return;
        }

        let jpeg = dir.join("out.jpg");
        convert_to_jpeg(&heic, &jpeg).expect("HEIC must convert");
        assert!(jpeg.is_file());
        assert!(fs::metadata(&jpeg).unwrap().len() > 0);

        let _ = fs::remove_dir_all(&dir);
    }
}
