//! Templates: the user's LaTeX house style, parameterised by keys.
//!
//! A template is a `.tex` preamble in which every adjustable value is replaced
//! by a `{{key}}` marker, plus the list of keys and their values. Rendering a
//! document is a *substitution*, never a generation — what compiles today keeps
//! compiling, because the LaTeX skeleton stays literally the user's own.
//!
//! In v1 the key list is fixed (it ships with the template). User-defined keys
//! come later; the format already allows for them.
//!
//! Note on language: `key` and `id` are technical and stay English, while
//! `label`, `group`, `name` and `description` are rendered in the UI and are
//! therefore authored in French.

use crate::logbus;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const BUILTIN_ID: &str = "charte-maths";
const BUILTIN_MANIFEST: &str = include_str!("../resources/templates/charte-maths/template.json");
const BUILTIN_PREAMBLE: &str =
    include_str!("../resources/templates/charte-maths/preamble.tex.tmpl");

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TemplateKey {
    pub key: String,
    pub label: String,
    pub group: String,
    /// `color` | `text` | `length` | `choice:a|b|c`
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
}

/// How one IR block kind is written in this template's LaTeX.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockMapping {
    /// `command` | `environment` | `raw` | `centered`
    pub mode: String,
    /// Command or environment name; empty for `raw` and `centered`.
    #[serde(default)]
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    /// Bumped whenever the bundled template gains structure (a new block
    /// mapping, a new key). Drives the upgrade in `seed`.
    #[serde(default = "one")]
    pub version: u32,
    pub name: String,
    pub description: String,
    /// Expected compiler (`pdflatex`, `xelatex`, ...).
    pub engine: String,
    pub keys: Vec<TemplateKey>,
    /// IR block kind -> LaTeX form. Missing kinds fall back to raw output.
    #[serde(default)]
    pub blocks: std::collections::HashMap<String, BlockMapping>,
}

fn one() -> u32 {
    1
}

pub fn dir(root: &Path) -> PathBuf {
    root.join("Templates")
}

/// Installs the bundled template, or upgrades an older installed copy.
///
/// Install-only seeding was a real bug: the app gained a `blocks` table that
/// never reached installs made before it, and rendering silently fell back to
/// raw output — a plausible-looking PDF with no headings and no environments.
///
/// The upgrade replaces the *structure* (block mappings, key definitions,
/// preamble) and preserves the *user's values* for every key that still exists,
/// so a colour they changed survives.
pub fn seed(root: &Path) -> io::Result<()> {
    let target = dir(root).join(BUILTIN_ID);
    let manifest_path = target.join("template.json");

    let bundled: Template = serde_json::from_str(BUILTIN_MANIFEST)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let installed: Option<Template> = fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());

    match installed {
        Some(installed) if installed.version >= bundled.version => return Ok(()),
        Some(installed) => {
            let kept: std::collections::HashMap<&str, &str> = installed
                .keys
                .iter()
                .map(|k| (k.key.as_str(), k.value.as_str()))
                .collect();

            let mut upgraded = bundled.clone();
            let mut preserved = 0usize;
            for key in &mut upgraded.keys {
                if let Some(value) = kept.get(key.key.as_str()) {
                    if *value != key.value {
                        preserved += 1;
                    }
                    key.value = (*value).to_string();
                }
            }

            fs::create_dir_all(&target)?;
            fs::write(
                &manifest_path,
                serde_json::to_string_pretty(&upgraded)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            )?;
            fs::write(target.join("preamble.tex.tmpl"), BUILTIN_PREAMBLE)?;

            logbus::detail(
                "template",
                format!(
                    "Modèle « {} » mis à jour (v{} vers v{})",
                    upgraded.name, installed.version, upgraded.version
                ),
                format!("{preserved} réglage(s) personnalisé(s) conservé(s)"),
            );
            return Ok(());
        }
        None => {}
    }

    fs::create_dir_all(&target)?;
    fs::write(&manifest_path, BUILTIN_MANIFEST)?;
    fs::write(target.join("preamble.tex.tmpl"), BUILTIN_PREAMBLE)?;
    logbus::detail("template", "Modèle livré installé", target.to_string_lossy().to_string());
    Ok(())
}

pub fn list(root: &Path) -> Vec<Template> {
    let _ = seed(root);
    let Ok(entries) = fs::read_dir(dir(root)) else {
        return Vec::new();
    };

    let mut templates: Vec<Template> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| fs::read_to_string(e.path().join("template.json")).ok())
        .filter_map(|raw| serde_json::from_str::<Template>(&raw).ok())
        .collect();

    templates.sort_by(|a, b| a.name.cmp(&b.name));
    templates
}

pub fn load(root: &Path, id: &str) -> Option<Template> {
    let raw = fs::read_to_string(dir(root).join(id).join("template.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Writes a template back to disk.
pub fn save(root: &Path, template: &Template) -> Result<(), String> {
    let target = dir(root).join(&template.id);
    fs::create_dir_all(&target).map_err(|e| format!("Dossier du modèle : {e}"))?;
    let manifest =
        serde_json::to_string_pretty(template).map_err(|e| format!("Sérialisation : {e}"))?;
    fs::write(target.join("template.json"), manifest)
        .map_err(|e| format!("Écriture du modèle : {e}"))?;
    logbus::info("template", format!("Modèle « {} » enregistré", template.name));
    Ok(())
}

/// Substitutes every `{{key}}` in the preamble with the template's values.
///
/// Colours are stored as `#A93226` for the UI, but `\definecolor` expects
/// `A93226`: the hash is dropped during substitution.
pub fn render_preamble(root: &Path, template: &Template) -> io::Result<String> {
    let mut preamble =
        fs::read_to_string(dir(root).join(&template.id).join("preamble.tex.tmpl"))?;

    for key in &template.keys {
        let value = if key.kind == "color" {
            key.value.trim_start_matches('#').to_string()
        } else {
            key.value.clone()
        };
        preamble = preamble.replace(&format!("{{{{{}}}}}", key.key), &value);
    }
    Ok(preamble)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind the recogniser may emit must have a LaTeX form here, or the
    /// renderer falls back to raw output and the block loses its environment.
    #[test]
    fn bundled_template_maps_every_block_kind() {
        let bundled: Template = serde_json::from_str(BUILTIN_MANIFEST).expect("valid manifest");
        for kind in crate::ir::BLOCK_KINDS {
            assert!(
                bundled.blocks.contains_key(*kind),
                "no LaTeX mapping for block kind `{kind}`"
            );
        }
    }

    /// Every `{{key}}` in the preamble must exist, and every key must be used.
    #[test]
    fn bundled_placeholders_match_bundled_keys() {
        let bundled: Template = serde_json::from_str(BUILTIN_MANIFEST).expect("valid manifest");
        for key in &bundled.keys {
            assert!(
                BUILTIN_PREAMBLE.contains(&format!("{{{{{}}}}}", key.key)),
                "key `{}` is never used in the preamble",
                key.key
            );
        }
        // A placeholder sits inside LaTeX braces — `\definecolor{mc}{HTML}{{{key}}}`
        // — so a naive scan for `{{` lands one brace early. Trim the surplus.
        let leftovers: Vec<&str> = BUILTIN_PREAMBLE
            .match_indices("{{")
            .filter_map(|(at, _)| {
                let rest = &BUILTIN_PREAMBLE[at + 2..];
                rest.find("}}").map(|end| rest[..end].trim_start_matches('{'))
            })
            .filter(|name| !name.is_empty())
            .filter(|name| !bundled.keys.iter().any(|k| k.key == *name))
            .collect();
        assert!(leftovers.is_empty(), "unknown placeholders: {leftovers:?}");
    }
}
