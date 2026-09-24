//! The last few states of a document, kept so a change made by Claude can be
//! taken back.
//!
//! A version is taken when Claude is about to rewrite the transcript — a
//! conversation about the whole document, a batch of corrections, a fresh
//! reading — and never for an edit made by hand. The teacher's own edits are
//! small and visible; what needs a way back is a model rewriting twenty passages
//! at once, where one wrong turn can be anywhere.
//!
//! Only `KEPT` versions are held, newest first, in `versions.json` beside the
//! transcript. A transcript is a few dozen kilobytes, so three whole copies cost
//! less than any scheme clever enough to store differences.

use crate::ir::Transcript;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// How many versions a document keeps.
pub const KEPT: usize = 3;

const FILE: &str = "versions.json";

/// One photograph, as far as a version needs to know it.
///
/// Name and size are enough to notice a page added, removed or moved: pages
/// are renamed `01.jpg`, `02.jpg`… in order, so reordering them changes which
/// size sits under which name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    pub created_at: u64,
    /// `chat` | `corrections` | `reading` | `restore` — what was about to change
    /// the document when this state was set aside.
    pub kind: String,
    /// What was asked, in the teacher's words when there are some.
    pub label: String,
    /// The photographs the transcript was made against.
    ///
    /// Blocks are tied to pages by number, so a transcript put back over a
    /// different set of photographs would pair each passage with the wrong
    /// page. A version whose photographs no longer match is shown, but refused.
    pub photos: Vec<Photo>,
    pub transcript: Transcript,
}

/// A version as the review lists it: everything but the transcript itself.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub created_at: u64,
    pub kind: String,
    pub label: String,
    pub blocks: usize,
    /// False when the photographs changed since: see `Version::photos`.
    pub restorable: bool,
}

/// The photographs in `pages/`, in order.
pub fn photos(document_dir: &Path) -> Vec<Photo> {
    let Ok(entries) = fs::read_dir(document_dir.join("pages")) else {
        return Vec::new();
    };
    let mut photos: Vec<Photo> = entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| Photo {
            name: entry.file_name().to_string_lossy().to_string(),
            size: entry.metadata().map(|m| m.len()).unwrap_or(0),
        })
        .collect();
    photos.sort_by(|a, b| a.name.cmp(&b.name));
    photos
}

fn read(document_dir: &Path) -> Vec<Version> {
    fs::read_to_string(document_dir.join(FILE))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write(document_dir: &Path, versions: &[Version]) -> Result<(), String> {
    let serialised = serde_json::to_string_pretty(versions).map_err(|e| e.to_string())?;
    fs::write(document_dir.join(FILE), serialised)
        .map_err(|e| format!("Écriture de l'historique : {e}"))
}

/// Puts `versions` newest first and drops what falls past `KEPT`.
fn pushed(mut versions: Vec<Version>, version: Version) -> Vec<Version> {
    versions.insert(0, version);
    versions.truncate(KEPT);
    versions
}

/// A name no other version of this document carries, even two taken within
/// the same millisecond.
fn fresh_id(now: u64) -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("v{now}-{n}")
}

fn blocks_in(transcript: &Transcript) -> usize {
    transcript.pages.iter().map(|page| page.blocks.len()).sum()
}

/// Sets `transcript` aside before Claude changes the document.
///
/// An empty transcript is not worth a place: going back to nothing is what
/// re-reading already offers, and it would push out a version that matters.
pub fn snapshot(
    document_dir: &Path,
    kind: &str,
    label: &str,
    transcript: &Transcript,
) -> Result<(), String> {
    if blocks_in(transcript) == 0 {
        return Ok(());
    }
    let now = crate::workspace::now_ms();
    let version = Version {
        id: fresh_id(now),
        created_at: now,
        kind: kind.to_string(),
        label: label.trim().to_string(),
        photos: photos(document_dir),
        transcript: transcript.clone(),
    };
    write(document_dir, &pushed(read(document_dir), version))?;
    crate::logbus::info(
        "workspace",
        format!("Version mise de côté avant : {}", label.trim()),
    );
    Ok(())
}

pub fn list(document_dir: &Path) -> Vec<Summary> {
    let current = photos(document_dir);
    read(document_dir)
        .into_iter()
        .map(|version| Summary {
            blocks: blocks_in(&version.transcript),
            restorable: version.photos == current,
            id: version.id,
            created_at: version.created_at,
            kind: version.kind,
            label: version.label,
        })
        .collect()
}

/// Swaps the current transcript with one of the versions.
///
/// The version leaves the list and the state it replaces takes a place in it,
/// so going back is itself something that can be taken back: restoring the
/// wrong version loses nothing. The list never grows past `KEPT` for it — the
/// version that left makes room for the one that arrives.
///
/// Returns the transcript to write. Writing it is the caller's, which holds the
/// transcript file.
pub fn restore(
    document_dir: &Path,
    version_id: &str,
    current: Option<&Transcript>,
) -> Result<Transcript, String> {
    let mut versions = read(document_dir);
    let at = versions
        .iter()
        .position(|version| version.id == version_id)
        .ok_or("Cette version n'existe plus.")?;

    if versions[at].photos != photos(document_dir) {
        return Err(
            "Les photos du document ont changé depuis cette version — une page ajoutée, \
             retirée ou déplacée. La remettre associerait des passages aux mauvaises pages."
                .into(),
        );
    }

    let chosen = versions.remove(at);
    if let Some(current) = current.filter(|t| blocks_in(t) > 0) {
        let now = crate::workspace::now_ms();
        versions = pushed(
            versions,
            Version {
                id: fresh_id(now),
                created_at: now,
                kind: "restore".into(),
                label: chosen.label.clone(),
                photos: chosen.photos.clone(),
                transcript: current.clone(),
            },
        );
    }
    write(document_dir, &versions)?;
    Ok(chosen.transcript)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Block, Page};

    fn transcript(latex: &str) -> Transcript {
        Transcript {
            version: 1,
            pages: vec![Page {
                number: 1,
                session_id: None,
                blocks: vec![Block {
                    id: "p01-b01".into(),
                    kind: "text".into(),
                    title: None,
                    number: None,
                    latex: latex.into(),
                    confidence: 1.0,
                    doubt: None,
                    audience: Vec::new(),
                    align: None,
                    note: None,
                    taught_end: false,
                    hidden: false,
                    reviewed: true,
                }],
            }],
        }
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("plume-history-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("pages")).unwrap();
        dir
    }

    #[test]
    fn only_the_last_three_are_kept_newest_first() {
        let dir = scratch("kept");
        for n in 1..=5 {
            snapshot(&dir, "chat", &format!("demande {n}"), &transcript(&n.to_string())).unwrap();
        }
        let listed = list(&dir);
        assert_eq!(listed.len(), KEPT);
        assert_eq!(listed[0].label, "demande 5");
        assert_eq!(listed[2].label, "demande 3");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_document_is_not_worth_a_version() {
        let dir = scratch("empty");
        snapshot(&dir, "reading", "relecture", &Transcript::default()).unwrap();
        assert!(list(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restoring_swaps_the_version_with_the_current_state() {
        let dir = scratch("swap");
        snapshot(&dir, "chat", "avant", &transcript("ancien")).unwrap();
        let id = list(&dir)[0].id.clone();

        let back = restore(&dir, &id, Some(&transcript("récent"))).unwrap();
        assert_eq!(back.pages[0].blocks[0].latex, "ancien");

        // The state that was replaced can itself be brought back.
        let listed = list(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].kind, "restore");
        let again = restore(&dir, &listed[0].id, Some(&back)).unwrap();
        assert_eq!(again.pages[0].blocks[0].latex, "récent");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_version_made_against_other_photos_is_refused() {
        let dir = scratch("photos");
        fs::write(dir.join("pages/01.jpg"), b"premiere").unwrap();
        snapshot(&dir, "chat", "avant", &transcript("x")).unwrap();
        assert!(list(&dir)[0].restorable);

        fs::write(dir.join("pages/02.jpg"), b"ajoutee").unwrap();
        let listed = list(&dir);
        assert!(!listed[0].restorable);
        assert!(restore(&dir, &listed[0].id, None).is_err());
        // Refused, not consumed: it is still there to look at.
        assert_eq!(list(&dir).len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
