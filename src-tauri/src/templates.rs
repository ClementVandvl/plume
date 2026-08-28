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

pub const BUILTIN_ID: &str = "charte-maths";
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
    /// Instructions that belong to this house style rather than to the teacher.
    ///
    /// Marker rules and standing conventions describe how *this teacher* writes,
    /// whatever template they use. These describe how *this template* wants its
    /// LaTeX shaped -- aligning a continued calculation on its equals sign, for
    /// instance -- and follow the template when a course changes style.
    #[serde(default)]
    pub conventions: Vec<crate::settings::Convention>,
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
/// Merges the bundled conventions into what is installed.
///
/// A convention the teacher reworded is theirs and survives. One still carrying
/// Plume's own wording is Plume's to correct: keeping it strands a machine on an
/// instruction known to be wrong.
fn reconcile(
    installed: &[crate::settings::Convention],
    bundled: &[crate::settings::Convention],
) -> Vec<crate::settings::Convention> {
    let mut merged = installed.to_vec();

    for delivered in bundled {
        match merged.iter_mut().find(|c| c.id == delivered.id) {
            Some(existing) => {
                if existing.text == existing.shipped {
                    existing.text = delivered.text.clone();
                    existing.title = delivered.title.clone();
                }
                // Either way the delivered wording moves on, so the next upgrade
                // compares against what shipped this time.
                existing.shipped = delivered.text.clone();
            }
            None => merged.push(delivered.clone()),
        }
    }
    merged
}

pub fn seed(root: &Path) -> io::Result<()> {
    let target = dir(root).join(BUILTIN_ID);
    let manifest_path = target.join("template.json");

    let mut bundled: Template = serde_json::from_str(BUILTIN_MANIFEST)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Remember the wording as delivered, so a later upgrade can tell an entry
    // the teacher reworded from one they never touched.
    for convention in &mut bundled.conventions {
        convention.shipped = convention.text.clone();
    }

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

            upgraded.conventions = reconcile(&installed.conventions, &bundled.conventions);

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
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&bundled)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
    )?;
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
    // Key values survive an upgrade; name, description and block mappings do
    // not. Refusing here beats accepting an edit that a future release erases.
    if is_builtin(&template.id) {
        if let Some(bundled) = load(root, BUILTIN_ID) {
            let restructured = template.name != bundled.name
                || template.description != bundled.description
                || template.blocks.len() != bundled.blocks.len()
                || template.blocks.iter().any(|(kind, mapping)| {
                    bundled.blocks.get(kind).is_none_or(|other| {
                        other.mode != mapping.mode || other.name != mapping.name
                    })
                });
            if restructured {
                return Err(
                    "Le modèle livré avec Plume est remplacé à chaque mise à jour : \
                     seules ses couleurs et ses valeurs sont conservées. \
                     Dupliquez-le pour en changer la structure."
                        .into(),
                );
            }
        }
    }

    let target = dir(root).join(&template.id);
    fs::create_dir_all(&target).map_err(|e| format!("Dossier du modèle : {e}"))?;
    let manifest =
        serde_json::to_string_pretty(template).map_err(|e| format!("Sérialisation : {e}"))?;
    fs::write(target.join("template.json"), manifest)
        .map_err(|e| format!("Écriture du modèle : {e}"))?;
    logbus::info("template", format!("Modèle « {} » enregistré", template.name));
    Ok(())
}

/// The bundled template, which Plume owns and overwrites on upgrade.
///
/// This is the whole reason duplication exists. `seed` rewrites
/// `preamble.tex.tmpl` whenever the bundled version rises, so an edit made
/// there would vanish at the next update — silently, months later, with no way
/// to tell what happened. Key *values* survive, because `seed` carries them
/// over; nothing else does.
pub fn is_builtin(id: &str) -> bool {
    id == BUILTIN_ID
}

/// A file-system-safe id derived from a display name.
fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.to_lowercase().chars() {
        // Accented letters are folded rather than dropped: "Modèle élève"
        // should not become "modle-lve".
        let folded = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' | 'õ' => 'o',
            'û' | 'ü' | 'ù' | 'ú' => 'u',
            'ç' => 'c',
            'ÿ' | 'ý' => 'y',
            'ñ' => 'n',
            other => other,
        };
        if folded.is_ascii_alphanumeric() {
            out.push(folded);
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "modele".to_string()
    } else {
        trimmed
    }
}

/// Copies a template under a new name, so the original stays untouched.
pub fn duplicate(root: &Path, source_id: &str, name: &str) -> Result<Template, String> {
    let source = load(root, source_id).ok_or("Modèle introuvable.")?;

    let name = name.trim();
    if name.is_empty() {
        return Err("Donnez un nom au modèle.".into());
    }

    // Suffix rather than reject: being told "that name is taken" while holding
    // a perfectly good name helps nobody.
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while dir(root).join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }

    let mut copy = source.clone();
    copy.id = id.clone();
    copy.name = name.to_string();
    // Version 1 and a distinct id keep it clear of `seed`, which only ever
    // touches BUILTIN_ID. A personal template is never upgraded under the
    // teacher's feet.
    copy.version = 1;

    let target = dir(root).join(&id);
    fs::create_dir_all(&target).map_err(|e| format!("Création du modèle : {e}"))?;
    fs::write(
        target.join("preamble.tex.tmpl"),
        read_preamble(root, source_id)?,
    )
    .map_err(|e| format!("Écriture du préambule : {e}"))?;
    save(root, &copy)?;

    logbus::detail(
        "template",
        format!("Modèle « {} » créé", copy.name),
        format!("copié depuis « {} »", source.name),
    );
    Ok(copy)
}

/// Moves a template to the workbook's bin. Never the bundled one.
pub fn delete(root: &Path, id: &str) -> Result<(), String> {
    if is_builtin(id) {
        return Err("Le modèle livré avec Plume ne peut pas être supprimé.".into());
    }
    let source = dir(root).join(id);
    if !source.exists() {
        return Err("Modèle introuvable.".into());
    }

    // Kept rather than erased, like a deleted course: a template is hours of
    // work and the teacher may have meant the other one.
    let bin = root.join("Corbeille").join("Modeles");
    fs::create_dir_all(&bin).map_err(|e| format!("Corbeille inaccessible : {e}"))?;

    let mut target = bin.join(id);
    let mut n = 2;
    while target.exists() {
        target = bin.join(format!("{id}-{n}"));
        n += 1;
    }
    fs::rename(&source, &target).map_err(|e| format!("Suppression du modèle : {e}"))?;
    logbus::info("template", format!("Modèle « {id} » déplacé vers la corbeille"));
    Ok(())
}

/// The preamble as written, placeholders intact.
pub fn read_preamble(root: &Path, id: &str) -> Result<String, String> {
    fs::read_to_string(dir(root).join(id).join("preamble.tex.tmpl"))
        .map_err(|e| format!("Lecture du préambule : {e}"))
}

/// Replaces the preamble, refusing placeholders no key defines.
///
/// An unknown `{{key}}` would survive substitution and reach the compiler as
/// literal braces — a LaTeX error far from its cause. Caught here, it names the
/// key instead.
pub fn write_preamble(root: &Path, id: &str, text: &str) -> Result<(), String> {
    if is_builtin(id) {
        return Err(
            "Le modèle livré avec Plume est remplacé à chaque mise à jour.              Dupliquez-le pour modifier sa mise en forme."
                .into(),
        );
    }
    let template = load(root, id).ok_or("Modèle introuvable.")?;
    let known: std::collections::HashSet<&str> =
        template.keys.iter().map(|k| k.key.as_str()).collect();

    let mut unknown: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        // A placeholder usually sits inside a LaTeX group -- `{{{color.body}}}`
        // -- so the extra braces are skipped before reading the name. Anything
        // that is not a bare key between the braces is ordinary LaTeX, such as
        // `{{\bfseries x}}`, and is left alone.
        let after = &rest[start + 2..];
        let inner = after.trim_start_matches('{');
        let name: String = inner
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            .collect();

        if !name.is_empty()
            && inner[name.len()..].starts_with("}}")
            && !known.contains(name.as_str())
            && !unknown.contains(&name)
        {
            unknown.push(name);
        }
        rest = after;
    }
    if !unknown.is_empty() {
        return Err(format!(
            "Clé{} inconnue{} dans le préambule : {}. Utilisez une clé existante ou corrigez la faute de frappe.",
            if unknown.len() > 1 { "s" } else { "" },
            if unknown.len() > 1 { "s" } else { "" },
            unknown.join(", ")
        ));
    }

    fs::write(dir(root).join(id).join("preamble.tex.tmpl"), text)
        .map_err(|e| format!("Écriture du préambule : {e}"))?;
    logbus::info("template", format!("Préambule de « {} » enregistré", template.name));
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

/// Compiles the preamble against a token document.
///
/// A preamble is only ever wrong at compile time, and finding that out during
/// an export — after a reading has been paid for — is the wrong moment. The
/// probe exercises every environment the template declares, so a mistyped
/// `\newtcolorbox` is caught here and not on the one block that used it.
pub fn check(root: &Path, template: &Template) -> Result<(), String> {
    let preamble = render_preamble(root, template).map_err(|e| format!("Préambule : {e}"))?;

    let mut body = String::from("\\begin{document}\n");
    for (kind, mapping) in &template.blocks {
        match mapping.mode.as_str() {
            "environment" => body.push_str(&format!(
                "\\begin{{{name}}}[{kind}]\nTexte de contrôle.\n\\end{{{name}}}\n",
                name = mapping.name
            )),
            "command" => body.push_str(&format!(
                "\\{}{{Texte de contrôle}}\n",
                mapping.name
            )),
            _ => {}
        }
    }
    body.push_str("\\end{document}\n");

    let dir = std::env::temp_dir().join(format!("plume-check-{}", template.id));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("Dossier temporaire : {e}"))?;
    fs::write(dir.join("check.tex"), format!("{preamble}\n{body}"))
        .map_err(|e| format!("Écriture du document de contrôle : {e}"))?;

    let outcome = crate::latex::compile(&dir, "check.tex");
    let _ = fs::remove_dir_all(&dir);
    outcome.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch workbook root, so these never touch the real one.
    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("plume-tpl-test-{name}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch root");
        seed(&root).expect("seed");
        root
    }

    /// The bundled instruction shows the model a literal example. If that
    /// example were itself malformed, every page would inherit the mistake — so
    /// it is held to the same check the renderer applies to real blocks.
    #[test]
    fn the_shipped_example_is_valid_latex_alignment() {
        let bundled: Template = serde_json::from_str(BUILTIN_MANIFEST).expect("valid manifest");
        let rule = bundled
            .conventions
            .iter()
            .find(|c| c.id == "align-equals")
            .expect("the alignment rule ships with the template");

        assert!(
            rule.text.contains("\\begin{aligned}"),
            "the instruction must show the exact environment"
        );
        // Only the example, not the prose around it: the instruction explains
        // what `&` means, so the word appears in running text as well.
        let start = rule.text.find("$\\begin{aligned}").expect("the example is shown");
        let end = rule.text.find("\\end{aligned}$").expect("the example is closed")
            + "\\end{aligned}$".len();
        let example = &rule.text[start..end];

        assert!(
            !crate::render::has_stray_alignment(example),
            "the example handed to the model must not itself misplace a tab"
        );
        assert!(example.contains("&="), "it must show the alignment point");
    }

    #[test]
    fn slugs_fold_accents_rather_than_dropping_them() {
        assert_eq!(slug("Modèle élève"), "modele-eleve");
        assert_eq!(slug("Charte 2nde — maths"), "charte-2nde-maths");
        assert_eq!(slug("   "), "modele");
        assert_eq!(slug("Français"), "francais");
    }

    #[test]
    fn duplicating_gives_an_independent_copy() {
        let root = scratch("duplicate");
        let copy = duplicate(&root, BUILTIN_ID, "Ma charte").expect("duplicate");

        assert_eq!(copy.id, "ma-charte");
        assert!(!is_builtin(&copy.id));
        assert_eq!(copy.version, 1, "a personal template must stay clear of seed");
        assert_eq!(copy.keys.len(), load(&root, BUILTIN_ID).unwrap().keys.len());
        assert!(dir(&root).join("ma-charte").join("preamble.tex.tmpl").is_file());

        // Editing the copy must leave the bundled preamble untouched.
        let mine = read_preamble(&root, &copy.id).unwrap().replace("0.9pt", "2.4pt");
        write_preamble(&root, &copy.id, &mine).expect("write copy");
        assert!(read_preamble(&root, &copy.id).unwrap().contains("2.4pt"));
        assert!(!read_preamble(&root, BUILTIN_ID).unwrap().contains("2.4pt"));

        let _ = fs::remove_dir_all(&root);
    }

    /// The whole point of duplication: `seed` must not reach a personal copy.
    #[test]
    fn upgrading_never_touches_a_personal_template() {
        let root = scratch("upgrade");
        let copy = duplicate(&root, BUILTIN_ID, "Ma charte").expect("duplicate");
        write_preamble(&root, &copy.id, "% entièrement le mien\n").expect("write");

        // Force the upgrade path by ageing the installed bundled manifest.
        let mut installed = load(&root, BUILTIN_ID).unwrap();
        installed.version = 1;
        let target = dir(&root).join(BUILTIN_ID).join("template.json");
        fs::write(&target, serde_json::to_string_pretty(&installed).unwrap()).unwrap();

        seed(&root).expect("re-seed");

        assert_eq!(read_preamble(&root, &copy.id).unwrap(), "% entièrement le mien\n");
        assert!(read_preamble(&root, BUILTIN_ID).unwrap().contains("chapitre"));
        let _ = fs::remove_dir_all(&root);
    }

    /// Conventions are editable precisely because an upgrade keeps them.
    #[test]
    fn an_upgrade_keeps_edited_conventions_and_adds_new_ones() {
        let root = scratch("conventions");

        let mut installed = load(&root, BUILTIN_ID).unwrap();
        assert!(
            installed.conventions.iter().any(|c| c.id == "align-equals"),
            "the bundled template must ship the alignment rule"
        );

        // The teacher reworded one and added their own, then Plume upgrades.
        installed.conventions[0].text = "Ma formulation à moi.".into();
        installed.conventions.push(crate::settings::Convention {
            id: "mine".into(),
            enabled: true,
            title: "La mienne".into(),
            text: "À conserver.".into(),
            shipped: String::new(),
        });
        installed.version = 1;
        fs::write(
            dir(&root).join(BUILTIN_ID).join("template.json"),
            serde_json::to_string_pretty(&installed).unwrap(),
        )
        .unwrap();

        seed(&root).expect("upgrade");

        let after = load(&root, BUILTIN_ID).unwrap();
        let aligned = after.conventions.iter().find(|c| c.id == "align-equals").unwrap();
        assert_eq!(aligned.text, "Ma formulation à moi.", "an edit must survive");
        assert!(after.conventions.iter().any(|c| c.id == "mine"), "additions must survive");

        let _ = fs::remove_dir_all(&root);
    }

    /// The other direction, and the reason `shipped` exists: an instruction the
    /// teacher never touched is Plume's to correct. This machine sat on a
    /// version-5 manifest carrying the version-4 wording — the vague one that
    /// produced bare alignment tabs — with nothing left to try again.
    #[test]
    fn an_upgrade_corrects_wording_the_teacher_never_touched() {
        let root = scratch("reword");

        // Roll the workbook back to an older delivery of the same rule.
        let mut installed = load(&root, BUILTIN_ID).unwrap();
        let old = "Ancienne formulation, trop vague.".to_string();
        installed.conventions[0].text = old.clone();
        installed.conventions[0].shipped = old.clone();
        installed.conventions[0].enabled = false;
        installed.version = 1;
        fs::write(
            dir(&root).join(BUILTIN_ID).join("template.json"),
            serde_json::to_string_pretty(&installed).unwrap(),
        )
        .unwrap();

        seed(&root).expect("upgrade");

        let after = load(&root, BUILTIN_ID).unwrap();
        let rule = after.conventions.iter().find(|c| c.id == "align-equals").unwrap();
        assert_ne!(rule.text, old, "an untouched instruction must be corrected");
        assert!(rule.text.contains("\\begin{aligned}"));
        assert_eq!(rule.shipped, rule.text, "the new wording becomes the reference");
        assert!(!rule.enabled, "their on/off choice is theirs either way");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_bundled_template_refuses_edits_an_upgrade_would_erase() {
        let root = scratch("builtin");

        assert!(write_preamble(&root, BUILTIN_ID, "% nope").is_err());
        assert!(delete(&root, BUILTIN_ID).is_err());

        let mut renamed = load(&root, BUILTIN_ID).unwrap();
        renamed.name = "Autre nom".into();
        assert!(save(&root, &renamed).is_err(), "renaming would be undone by seed");

        // Colours, however, are carried over by seed and must stay editable.
        let mut recoloured = load(&root, BUILTIN_ID).unwrap();
        recoloured.keys[0].value = "#123456".into();
        assert!(save(&root, &recoloured).is_ok());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unknown_placeholder_is_named_rather_than_compiled() {
        let root = scratch("placeholder");
        let copy = duplicate(&root, BUILTIN_ID, "Copie").expect("duplicate");

        let error = write_preamble(&root, &copy.id, "{{color.chaptre}} {{color.chapter}}")
            .expect_err("typo must be refused");
        assert!(error.contains("color.chaptre"), "the error must name the key: {error}");
        assert!(!error.contains("color.chapter,"), "valid keys must not be listed");

        assert!(write_preamble(&root, &copy.id, "{{color.chapter}}").is_ok());

        // The real preamble writes a placeholder inside a LaTeX group, and
        // ordinary doubled braces must not be mistaken for one.
        write_preamble(
            &root,
            &copy.id,
            "\\definecolor{mc}{HTML}{{{color.chapter}}}\n{{\\bfseries x}}\n",
        )
        .expect("a grouped placeholder is valid LaTeX and a valid key");

        let _ = fs::remove_dir_all(&root);
    }

    /// The whole editing chain against the real engine.
    ///
    /// Ignored by default: it needs Tectonic and a network for its first run,
    /// neither of which belongs in CI. Run it with
    /// `cargo test -- --ignored check_compiles` after touching the probe.
    #[test]
    #[ignore = "needs the LaTeX engine"]
    fn the_check_compiles_a_healthy_template_and_rejects_a_broken_one() {
        let root = scratch("check");
        let copy = duplicate(&root, BUILTIN_ID, "Contrôle").expect("duplicate");
        let template = load(&root, &copy.id).expect("load");

        check(&root, &template).expect("a healthy template must compile");

        let broken = read_preamble(&root, &copy.id)
            .unwrap()
            .replace("\\newtcolorbox{mccrochet}", "\\newtcolorbox{mccroche}");
        write_preamble(&root, &copy.id, &broken).expect("write");
        assert!(
            check(&root, &template).is_err(),
            "a check that never fails is worthless"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn deleting_keeps_the_template_in_the_bin() {
        let root = scratch("delete");
        let copy = duplicate(&root, BUILTIN_ID, "Jetable").expect("duplicate");
        delete(&root, &copy.id).expect("delete");

        assert!(load(&root, &copy.id).is_none());
        assert!(
            root.join("Corbeille").join("Modeles").join(&copy.id).exists(),
            "a deleted template must be recoverable"
        );
        let _ = fs::remove_dir_all(&root);
    }

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
