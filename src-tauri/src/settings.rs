//! Application settings, stored beside the courses.
//!
//! Reading conventions exist at two levels. Most of what a teacher wants is
//! true of every course they write — how they mark a teacher-only passage, how
//! they want diagrams drawn — and repeating it per course would guarantee it
//! drifts. Course-level rules stay, for the exceptions.
//!
//! Both are registries rather than prose. A marker rule pairs a trigger with an
//! effect — "highlighted in orange means bold". A convention is a standing
//! instruction with no visual trigger — "never let a label overlap a line".
//! Keeping each one a separate, nameable, switchable entry means a teacher can
//! turn one off without editing a paragraph, and the compiled prompt is the
//! same every time.

use crate::logbus;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const FILE: &str = "settings.json";

/// What the teacher drew on the page.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Trigger {
    /// `highlight` | `marginBar` | `underline` | `circled` | `penColour` | `custom`
    pub kind: String,
    /// Hex colour of the marker, when the marker has one.
    #[serde(default)]
    pub colour: String,
    /// How the teacher names it — "orange", "bleu clair" — or the whole
    /// description when `kind` is `custom`.
    #[serde(default)]
    pub label: String,
}

/// What it should mean in the transcription.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    /// `bold` | `italic` | `underline` | `teacherOnly` | `studentOnly`
    /// | `blockKind` | `skip` | `custom`
    pub kind: String,
    /// Block kind for `blockKind`, free text for `custom`.
    #[serde(default)]
    pub value: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReadingRule {
    pub id: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub trigger: Trigger,
    pub effect: Effect,
}

fn yes() -> bool {
    true
}

/// A standing instruction with no visual trigger.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Convention {
    pub id: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Short name, for the list.
    #[serde(default)]
    pub title: String,
    /// The instruction itself, in the teacher's words, passed through verbatim.
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Visual conventions, applied to every course.
    #[serde(default)]
    pub rules: Vec<ReadingRule>,
    /// Standing instructions, applied to every course.
    #[serde(default)]
    pub conventions: Vec<Convention>,
    /// Superseded by `conventions`; kept so an older file still loads, and
    /// migrated on read rather than silently dropped.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reading_rules: String,
    /// Model used unless a course overrides it at read time.
    #[serde(default = "default_model")]
    pub default_model: String,
    /// Look for a new version at start-up. Only the *check* is automatic —
    /// installing always waits for a click.
    #[serde(default = "yes")]
    pub check_updates: bool,
    /// Pages read at once. `0` means automatic, from the machine's memory —
    /// three `claude` processes at once froze an 8 GB laptop.
    #[serde(default)]
    pub concurrent_pages: u32,
}

fn default_model() -> String {
    "sonnet".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            conventions: Vec::new(),
            reading_rules: String::new(),
            default_model: default_model(),
            check_updates: true,
            concurrent_pages: 0,
        }
    }
}

pub fn path() -> PathBuf {
    crate::workspace::root().join(FILE)
}

/// Missing or unreadable settings fall back to the defaults rather than
/// stopping the app: nothing here is worth a blank window.
pub fn load() -> Settings {
    let mut settings: Settings = fs::read_to_string(path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    // Free text written before conventions existed becomes one entry rather
    // than being lost the first time the file is rewritten.
    if !settings.reading_rules.trim().is_empty() {
        settings.conventions.push(Convention {
            id: format!("migrated-{}", settings.conventions.len()),
            enabled: true,
            title: "Conventions générales".to_string(),
            text: settings.reading_rules.trim().to_string(),
        });
        settings.reading_rules.clear();
    }

    settings
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let _ = crate::workspace::ensure_root();
    let serialised = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path(), serialised).map_err(|e| format!("Écriture des réglages : {e}"))?;
    logbus::info(
        "workspace",
        format!(
            "Réglages enregistrés — {} marque(s), {} convention(s)",
            settings.rules.iter().filter(|r| r.enabled).count(),
            settings.conventions.iter().filter(|c| c.enabled).count()
        ),
    );
    Ok(())
}

fn describe_trigger(trigger: &Trigger) -> String {
    let colour = if trigger.colour.is_empty() {
        String::new()
    } else if trigger.label.is_empty() {
        format!(" ({})", trigger.colour)
    } else {
        format!(" ({}, {})", trigger.label, trigger.colour)
    };

    match trigger.kind.as_str() {
        "highlight" => format!("text highlighted with a marker pen{colour}"),
        "marginBar" => format!("a vertical bar drawn in the margin beside the passage{colour}"),
        "underline" => format!("text underlined by hand{colour}"),
        "circled" => format!("text circled or boxed by hand{colour}"),
        "penColour" => format!("text written in coloured ink{colour}"),
        _ => {
            if trigger.label.is_empty() {
                "a marker the teacher described".to_string()
            } else {
                trigger.label.clone()
            }
        }
    }
}

fn describe_effect(effect: &Effect) -> String {
    match effect.kind.as_str() {
        "bold" => "wrap that text in \\textbf{...}".to_string(),
        "italic" => "wrap that text in \\emph{...}".to_string(),
        "underline" => "wrap that text in \\underline{...}".to_string(),
        "teacherOnly" => {
            "the block is for the teacher's copy only: set its audience to [\"teacher\"]"
                .to_string()
        }
        "studentOnly" => {
            "the block is for the students' copy only: set its audience to [\"student\"]"
                .to_string()
        }
        "blockKind" => format!("emit that passage as a `{}` block", effect.value),
        "skip" => "do not transcribe that passage at all".to_string(),
        _ => effect.value.clone(),
    }
}

/// Turns the registry into the instruction block handed to the recogniser.
fn compile(settings: &Settings) -> String {
    let active: Vec<&ReadingRule> = settings.rules.iter().filter(|rule| rule.enabled).collect();
    if active.is_empty() {
        return String::new();
    }

    let lines: Vec<String> = active
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            format!(
                "{}. When you see {} -> {}.",
                index + 1,
                describe_trigger(&rule.trigger),
                describe_effect(&rule.effect)
            )
        })
        .collect();

    format!(
        "Marker conventions defined by the teacher. Apply them exactly; they \
         override your defaults. A marker that is not present on a block changes \
         nothing about it.\n{}",
        lines.join("\n")
    )
}

/// Formats a list of conventions into numbered, titled lines.
fn number(conventions: &[Convention]) -> Vec<String> {
    conventions
        .iter()
        .filter(|convention| convention.enabled && !convention.text.trim().is_empty())
        .enumerate()
        .map(|(index, convention)| {
            let title = convention.title.trim();
            if title.is_empty() {
                format!("{}. {}", index + 1, convention.text.trim())
            } else {
                format!("{}. {} — {}", index + 1, title, convention.text.trim())
            }
        })
        .collect()
}

/// The full instruction block, narrowing from the teacher to this one course:
/// marker rules, standing conventions, the template's own, then the course's.
pub fn combine(
    settings: &Settings,
    template_conventions: &[Convention],
    course_rules: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let compiled = compile(settings);
    if !compiled.is_empty() {
        parts.push(compiled);
    }
    let conventions = number(&settings.conventions);
    if !conventions.is_empty() {
        parts.push(format!(
            "Standing conventions defined by the teacher, in their own words. \
             Follow every one of them.\n{}",
            conventions.join("\n")
        ));
    }

    let from_template = number(template_conventions);
    if !from_template.is_empty() {
        parts.push(format!(
            "Typesetting conventions of the template this course uses. They \
             shape the LaTeX you produce.\n{}",
            from_template.join("\n")
        ));
    }
    if !course_rules.trim().is_empty() {
        parts.push(format!("Specific to this course:\n{}", course_rules.trim()));
    }

    parts.join("\n\n")
}

pub fn combined_rules(template_id: &str, course_rules: &str) -> String {
    let template = crate::templates::load(&crate::workspace::root(), template_id);
    let conventions = template.map(|t| t.conventions).unwrap_or_default();
    combine(&load(), &conventions, course_rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: &str, colour: &str, label: &str, effect: &str, value: &str) -> ReadingRule {
        ReadingRule {
            id: "r".into(),
            enabled: true,
            trigger: Trigger {
                kind: kind.into(),
                colour: colour.into(),
                label: label.into(),
            },
            effect: Effect { kind: effect.into(), value: value.into() },
        }
    }

    #[test]
    fn compiles_a_registry_into_numbered_instructions() {
        let settings = Settings {
            rules: vec![
                rule("highlight", "#F2A93B", "orange", "bold", ""),
                rule("marginBar", "#1F618D", "bleu", "teacherOnly", ""),
            ],
            ..Settings::default()
        };

        let text = compile(&settings);
        assert!(text.contains("highlighted with a marker pen (orange, #F2A93B)"));
        assert!(text.contains("\\textbf"));
        assert!(text.contains("vertical bar drawn in the margin"));
        assert!(text.contains("[\"teacher\"]"));
        assert!(text.contains("1."), "rules must be numbered");
        assert!(text.contains("2."));
    }

    fn convention(id: &str, enabled: bool, title: &str, text: &str) -> Convention {
        Convention {
            id: id.into(),
            enabled,
            title: title.into(),
            text: text.into(),
        }
    }

    #[test]
    fn conventions_are_numbered_titled_and_filtered() {
        let settings = Settings {
            conventions: vec![
                convention("c1", true, "Annotations", "Aucune étiquette ne chevauche un trait."),
                convention("c2", false, "Ignorée", "Ne doit pas apparaître."),
                convention("c3", true, "", "Sans titre mais active."),
            ],
            ..Settings::default()
        };

        let text = combine(&settings, &[], "");
        assert!(text.contains("1. Annotations — Aucune étiquette ne chevauche un trait."));
        assert!(text.contains("2. Sans titre mais active."));
        assert!(!text.contains("Ne doit pas apparaître"), "disabled entries must be dropped");
    }

    #[test]
    fn the_three_levels_are_concatenated_in_order() {
        let settings = Settings {
            rules: vec![rule("highlight", "#F2A93B", "orange", "bold", "")],
            conventions: vec![convention("c1", true, "Schémas", "Pas de recouvrement.")],
            ..Settings::default()
        };

        let text = combine(&settings, &[], "Ce chapitre utilise des repères.");
        let marker = text.find("Marker conventions").expect("marker block");
        let standing = text.find("Standing conventions").expect("standing block");
        let course = text.find("Specific to this course").expect("course block");
        assert!(marker < standing && standing < course);
    }

    /// The template's own typesetting rules sit between the teacher's standing
    /// conventions and whatever this one course adds.
    #[test]
    fn template_conventions_are_a_level_of_their_own() {
        let settings = Settings {
            conventions: vec![convention("c1", true, "Schémas", "Pas de recouvrement.")],
            ..Settings::default()
        };
        let from_template = vec![
            convention("t1", true, "Alignement", "Aligne les calculs sur le =."),
            convention("t2", false, "Ignorée", "Ne doit pas apparaître."),
        ];

        let text = combine(&settings, &from_template, "Ce chapitre utilise des repères.");
        let standing = text.find("Standing conventions").expect("standing block");
        let template = text.find("Typesetting conventions").expect("template block");
        let course = text.find("Specific to this course").expect("course block");
        assert!(standing < template && template < course);
        assert!(text.contains("Aligne les calculs sur le =."));
        assert!(!text.contains("Ne doit pas apparaître"), "disabled entries must be dropped");
    }

    /// Numbering restarts per section, so the model never sees two "1.".
    #[test]
    fn each_level_numbers_itself_from_one() {
        let settings = Settings {
            conventions: vec![convention("c1", true, "", "Première du prof.")],
            ..Settings::default()
        };
        let text = combine(&settings, &[convention("t1", true, "", "Première du modèle.")], "");
        assert!(text.contains("1. Première du prof."));
        assert!(text.contains("1. Première du modèle."));
    }

    #[test]
    fn a_disabled_rule_is_not_compiled() {
        let mut disabled = rule("highlight", "#F2A93B", "orange", "bold", "");
        disabled.enabled = false;
        let settings = Settings { rules: vec![disabled], ..Settings::default() };
        assert_eq!(compile(&settings), "");
    }
}
