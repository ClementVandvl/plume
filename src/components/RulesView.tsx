import { useEffect, useState } from "react";
import { saveSettings } from "../api";
import { t } from "../i18n";
import { logError } from "../log";
import { useConfirm } from "../confirm";
import {
  EFFECT_LABEL,
  TRIGGER_LABEL,
  type Convention,
  type ReadingRule,
  type Settings,
} from "../types";
import { Toggle } from "../ui/controls";
import { ConventionPanel } from "./ConventionPanel";
import { RulePanel } from "./RulePanel";

/**
 * The registry of marker conventions — "Mes annotations".
 *
 * These are rules, not prose: "highlighted in orange means bold" has a trigger
 * and an effect, so it compiles to the same instruction every time and can be
 * switched off without rewriting a paragraph. Each row reads as the sentence
 * it will become. Free text stays alongside for what a registry cannot express.
 */

const newRule = (): ReadingRule => ({
  id: crypto.randomUUID(),
  enabled: true,
  trigger: { kind: "highlight", colour: "#F2A93B", label: "orange" },
  effect: { kind: "bold", value: "" },
});

const newConvention = (): Convention => ({
  id: crypto.randomUUID(),
  enabled: true,
  title: "",
  text: "",
});

/** Which entry the side panel is editing. */
type Selection = { list: "rules" | "conventions"; id: string } | null;

const firstLine = (text: string) => text.trim().split("\n")[0] || "—";

/** "Surligné orange" — the trigger as the teacher would say it. */
function triggerPhrase(rule: ReadingRule): string {
  const base = TRIGGER_LABEL[rule.trigger.kind];
  return rule.trigger.label ? `${base} ${rule.trigger.label}` : base;
}

function effectPhrase(rule: ReadingRule): string {
  if (rule.effect.kind === "custom" && rule.effect.value) return rule.effect.value;
  if (rule.effect.kind === "blockKind" && rule.effect.value)
    return `${EFFECT_LABEL.blockKind} : ${rule.effect.value}`;
  return EFFECT_LABEL[rule.effect.kind];
}

type Props = {
  settings: Settings;
  onSaved: (settings: Settings) => void;
};

export function RulesView({ settings, onSaved }: Props) {
  const [draft, setDraft] = useState(settings);
  const [selected, setSelected] = useState<Selection>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { confirm } = useConfirm();

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  const selectedRule =
    selected?.list === "rules"
      ? (draft.rules.find((rule) => rule.id === selected.id) ?? null)
      : null;
  const selectedConvention =
    selected?.list === "conventions"
      ? (draft.conventions.find((item) => item.id === selected.id) ?? null)
      : null;
  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);

  function update(rule: ReadingRule) {
    setDraft((current) => ({
      ...current,
      rules: current.rules.map((existing) => (existing.id === rule.id ? rule : existing)),
    }));
  }

  function updateConvention(convention: Convention) {
    setDraft((current) => ({
      ...current,
      conventions: current.conventions.map((existing) =>
        existing.id === convention.id ? convention : existing,
      ),
    }));
  }

  async function remove(list: "rules" | "conventions", id: string) {
    const isRule = list === "rules";
    const ok = await confirm({
      title: isRule ? t("annotations.delete.rule.title") : t("annotations.delete.convention.title"),
      message: isRule
        ? t("annotations.delete.rule.message")
        : t("annotations.delete.convention.message"),
      detail: t("annotations.delete.detail"),
      confirmLabel: t("common.delete"),
      tone: "danger",
    });
    if (!ok) return;

    setDraft((current) => ({
      ...current,
      [list]: current[list].filter((entry: { id: string }) => entry.id !== id),
    }));
    setSelected(null);
  }

  function addRule() {
    const rule = newRule();
    setDraft((current) => ({ ...current, rules: [...current.rules, rule] }));
    setSelected({ list: "rules", id: rule.id });
  }

  function addConvention() {
    const convention = newConvention();
    setDraft((current) => ({
      ...current,
      conventions: [...current.conventions, convention],
    }));
    setSelected({ list: "conventions", id: convention.id });
  }

  async function persist() {
    setSaving(true);
    setError(null);
    try {
      await saveSettings(draft);
      onSaved(draft);
    } catch (cause) {
      setError(String(cause));
      logError("workspace", t("error.refresh"), cause);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">{t("annotations.title")}</h1>
          <p className="page-subtitle page-subtitle--wide">{t("annotations.subtitle")}</p>
        </div>
        <div className="page-head__tools">
          {dirty && (
            <button
              type="button"
              className="btn btn--primary"
              onClick={persist}
              disabled={saving}
            >
              {saving ? t("common.saving") : t("common.save")}
            </button>
          )}
        </div>
      </header>

      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}

      <div className="annotations-grid">
        <section className="listcard">
          <header className="listcard__head">
            <div className="listcard__lead">
              <span className="listcard__heading">{t("annotations.marks.title")}</span>
              <span className="listcard__hint">{t("annotations.marks.hint")}</span>
            </div>
          </header>

          {draft.rules.length === 0 ? (
            <p className="listcard__empty">{t("annotations.marks.empty")}</p>
          ) : (
            draft.rules.map((rule) => (
              <div
                key={rule.id}
                className={`arule ${rule.enabled ? "" : "arule--off"} ${
                  selected?.id === rule.id ? "arule--selected" : ""
                }`}
                onClick={() => setSelected({ list: "rules", id: rule.id })}
                role="button"
                tabIndex={0}
                onKeyDown={(e) =>
                  e.key === "Enter" && setSelected({ list: "rules", id: rule.id })
                }
              >
                <span
                  className="arule__swatch"
                  style={{ background: rule.trigger.colour || "transparent" }}
                  aria-hidden="true"
                />
                <span className="arule__sentence">
                  {t("annotations.sentence", {
                    trigger: triggerPhrase(rule).toLowerCase(),
                    effect: effectPhrase(rule).toLowerCase(),
                  })}
                </span>
                <span onClick={(e) => e.stopPropagation()}>
                  <Toggle
                    checked={rule.enabled}
                    onChange={(value) => update({ ...rule, enabled: value })}
                    label={t("rulePanel.apply")}
                  />
                </span>
              </div>
            ))
          )}

          <button type="button" className="listcard__add" onClick={addRule}>
            {t("annotations.marks.add")}
          </button>
        </section>

        <section className="listcard">
          <header className="listcard__head">
            <div className="listcard__lead">
              <span className="listcard__heading">
                {t("annotations.conventions.title")}
              </span>
              <span className="listcard__hint">{t("annotations.conventions.hint")}</span>
            </div>
          </header>

          {draft.conventions.length === 0 ? (
            <p className="listcard__empty">{t("annotations.conventions.empty")}</p>
          ) : (
            draft.conventions.map((convention) => (
              <div
                key={convention.id}
                className={`arule ${convention.enabled ? "" : "arule--off"} ${
                  selected?.id === convention.id ? "arule--selected" : ""
                }`}
                onClick={() => setSelected({ list: "conventions", id: convention.id })}
                role="button"
                tabIndex={0}
                onKeyDown={(e) =>
                  e.key === "Enter" &&
                  setSelected({ list: "conventions", id: convention.id })
                }
              >
                <span className="arule__sentence">
                  <strong>{convention.title.trim() || "—"}</strong>
                  <span className="arule__text">{firstLine(convention.text)}</span>
                </span>
                <span onClick={(e) => e.stopPropagation()}>
                  <Toggle
                    checked={convention.enabled}
                    onChange={(value) =>
                      updateConvention({ ...convention, enabled: value })
                    }
                    label={t("conventionPanel.apply")}
                  />
                </span>
              </div>
            ))
          )}

          <button type="button" className="listcard__add" onClick={addConvention}>
            {t("annotations.conventions.add")}
          </button>
        </section>
      </div>

      {selectedRule && (
        <RulePanel
          rule={selectedRule}
          onChange={update}
          onDelete={() => remove("rules", selectedRule.id)}
          onClose={() => setSelected(null)}
          float
        />
      )}

      {selectedConvention && (
        <ConventionPanel
          convention={selectedConvention}
          onChange={updateConvention}
          onDelete={() => remove("conventions", selectedConvention.id)}
          onClose={() => setSelected(null)}
          float
        />
      )}
    </div>
  );
}
