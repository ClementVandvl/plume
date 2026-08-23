import { useEffect, useState } from "react";
import { saveSettings } from "../api";
import { logError } from "../log";
import { useConfirm } from "../confirm";
import {
  EFFECT_LABEL,
  TRIGGER_LABEL,
  type Convention,
  type ReadingRule,
  type Settings,
} from "../types";
import { ConventionPanel } from "./ConventionPanel";
import { RulePanel } from "./RulePanel";

/**
 * The registry of marker conventions.
 *
 * These are rules, not prose: "highlighted in orange means bold" has a trigger
 * and an effect, so it compiles to the same instruction every time and can be
 * switched off without rewriting a paragraph. Free text stays alongside for
 * what a registry cannot express.
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
      title: isRule ? "Supprimer cette marque ?" : "Supprimer cette convention ?",
      message: isRule
        ? "Elle ne sera plus appliquée à vos prochaines lectures."
        : "Elle ne sera plus transmise au modèle lors de vos prochaines lectures.",
      detail: "Pour la désactiver sans la perdre, décochez « Appliquer » à la place.",
      confirmLabel: "Supprimer",
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
      logError("workspace", "Enregistrement des règles impossible", cause);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">Règles de lecture</h1>
          <p className="page-subtitle">
            Ce que vos marques veulent dire. Appliquées à tous vos cours, à la
            prochaine lecture.
          </p>
        </div>
        <div className="review__actions">
          <button
            type="button"
            className="btn btn--primary"
            onClick={persist}
            disabled={!dirty || saving}
          >
            {saving ? "Enregistrement…" : "Enregistrer"}
          </button>
        </div>
      </header>

      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}

      <div className={`split ${selected ? "split--open" : ""}`}>
        <div className="split__main">
          <section className="registry">
            <header className="registry__head">
              <div>
                <h2 className="section-title">Marques</h2>
                <p className="registry__hint">
                  Quelque chose que vous tracez sur la feuille, et ce que ça veut dire.
                </p>
              </div>
              <button type="button" className="btn btn--ghost" onClick={addRule}>
                Ajouter une marque
              </button>
            </header>

            {draft.rules.length === 0 ? (
              <p className="registry__empty">
                Aucune marque. Par exemple : surligné orange → gras, trait bleu en
                marge → réservé au professeur.
              </p>
            ) : (
              <ul className="rules">
                {draft.rules.map((rule) => (
                  <li key={rule.id}>
                    <button
                      type="button"
                      className={`rule ${rule.enabled ? "" : "rule--off"} ${
                        selected?.id === rule.id ? "rule--selected" : ""
                      }`}
                      onClick={() => setSelected({ list: "rules", id: rule.id })}
                    >
                      <span
                        className="rule__swatch"
                        style={{ background: rule.trigger.colour || "transparent" }}
                        aria-hidden="true"
                      />
                      <span className="rule__trigger">
                        {TRIGGER_LABEL[rule.trigger.kind]}
                        {rule.trigger.label && (
                          <span className="rule__colour"> {rule.trigger.label}</span>
                        )}
                      </span>
                      <span className="rule__arrow" aria-hidden="true">
                        →
                      </span>
                      <span className="rule__effect">
                        {rule.effect.kind === "custom" && rule.effect.value
                          ? rule.effect.value
                          : rule.effect.kind === "blockKind" && rule.effect.value
                            ? `${EFFECT_LABEL.blockKind} : ${rule.effect.value}`
                            : EFFECT_LABEL[rule.effect.kind]}
                      </span>
                      {!rule.enabled && <span className="flag">désactivée</span>}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="registry">
            <header className="registry__head">
              <div>
                <h2 className="section-title">Conventions</h2>
                <p className="registry__hint">
                  Une consigne permanente, sans marque à repérer — la façon de dessiner
                  les schémas, de nommer les points, d'abréger un intitulé.
                </p>
              </div>
              <button type="button" className="btn btn--ghost" onClick={addConvention}>
                Ajouter une convention
              </button>
            </header>

            {draft.conventions.length === 0 ? (
              <p className="registry__empty">
                Aucune convention. Par exemple : jamais d'annotation par-dessus un
                trait dans les schémas.
              </p>
            ) : (
              <ul className="rules">
                {draft.conventions.map((convention) => (
                  <li key={convention.id}>
                    <button
                      type="button"
                      className={`rule rule--convention ${
                        convention.enabled ? "" : "rule--off"
                      } ${selected?.id === convention.id ? "rule--selected" : ""}`}
                      onClick={() => setSelected({ list: "conventions", id: convention.id })}
                    >
                      <span className="rule__trigger">
                        {convention.title.trim() || "Sans titre"}
                      </span>
                      <span className="rule__effect">{firstLine(convention.text)}</span>
                      {!convention.enabled && <span className="flag">désactivée</span>}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>

        {selectedRule && (
          <RulePanel
            rule={selectedRule}
            onChange={update}
            onDelete={() => remove("rules", selectedRule.id)}
            onClose={() => setSelected(null)}
          />
        )}

        {selectedConvention && (
          <ConventionPanel
            convention={selectedConvention}
            onChange={updateConvention}
            onDelete={() => remove("conventions", selectedConvention.id)}
            onClose={() => setSelected(null)}
          />
        )}
      </div>
    </div>
  );
}
