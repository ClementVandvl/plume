import { useEffect } from "react";
import { t } from "../i18n";
import {
  EFFECT_LABEL,
  KIND_LABEL,
  TRIGGER_LABEL,
  type Effect,
  type ReadingRule,
  type Trigger,
} from "../types";

const COLOURED_TRIGGERS = new Set<Trigger["kind"]>([
  "highlight",
  "marginBar",
  "underline",
  "circled",
  "penColour",
]);

type Props = {
  rule: ReadingRule;
  onChange: (rule: ReadingRule) => void;
  onDelete: () => void;
  onClose: () => void;
  /** Drawn over the page as a drawer, instead of embedded in a column. */
  float?: boolean;
};

export function RulePanel({ rule, onChange, onDelete, onClose, float }: Props) {
  // A drawer closes on Escape, like any transient surface.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const setTrigger = (patch: Partial<Trigger>) =>
    onChange({ ...rule, trigger: { ...rule.trigger, ...patch } });
  const setEffect = (patch: Partial<Effect>) =>
    onChange({ ...rule, effect: { ...rule.effect, ...patch } });

  return (
    <aside className={`panel-side ${float ? "panel-side--float" : ""}`}>
      <header className="panel-side__head">
        <div>
          <span className="panel-side__kind">{t("rulePanel.title")}</span>
          <span className="panel-side__meta">
            {rule.enabled ? t("rulePanel.active") : t("instructions.disabled")}
          </span>
        </div>
        <button
          type="button"
          className="icon-btn icon-btn--close"
          onClick={onClose}
          aria-label={t("common.close")}
        >
          ×
        </button>
      </header>

      <div className="panel-side__body">
        <label className="check">
          <input
            type="checkbox"
            checked={rule.enabled}
            onChange={(e) => onChange({ ...rule, enabled: e.target.checked })}
          />
          {t("rulePanel.apply")}
        </label>

        <section className="stack stack--tight">
          <h3 className="section-title">{t("rulePanel.when")}</h3>

          <label className="field">
            <span className="field__label">{t("rulePanel.mark")}</span>
            <select
              className="input"
              value={rule.trigger.kind}
              onChange={(e) => setTrigger({ kind: e.target.value as Trigger["kind"] })}
            >
              {Object.entries(TRIGGER_LABEL).map(([id, label]) => (
                <option key={id} value={id}>
                  {label}
                </option>
              ))}
            </select>
          </label>

          {COLOURED_TRIGGERS.has(rule.trigger.kind) ? (
            <>
              <label className="field">
                <span className="field__label">{t("rulePanel.colour")}</span>
                <span className="key__color">
                  <input
                    type="color"
                    className="swatch"
                    value={rule.trigger.colour || "#F2A93B"}
                    onChange={(e) => setTrigger({ colour: e.target.value.toUpperCase() })}
                  />
                  <input
                    className="input input--compact"
                    value={rule.trigger.colour}
                    onChange={(e) => setTrigger({ colour: e.target.value.toUpperCase() })}
                  />
                </span>
              </label>

              <label className="field">
                <span className="field__label">{t("rulePanel.colourName")}</span>
                <input
                  className="input"
                  value={rule.trigger.label}
                  onChange={(e) => setTrigger({ label: e.target.value })}
                  placeholder={t("rulePanel.colourName.placeholder")}
                />
                <span className="field__hint">{t("rulePanel.colourName.hint")}</span>
              </label>
            </>
          ) : (
            <label className="field">
              <span className="field__label">{t("rulePanel.describe")}</span>
              <textarea
                className="input"
                rows={3}
                value={rule.trigger.label}
                onChange={(e) => setTrigger({ label: e.target.value })}
                placeholder={t("rulePanel.describe.placeholder")}
              />
            </label>
          )}
        </section>

        <section className="stack stack--tight">
          <h3 className="section-title">{t("rulePanel.then")}</h3>

          <label className="field">
            <span className="field__label">{t("rulePanel.effect")}</span>
            <select
              className="input"
              value={rule.effect.kind}
              onChange={(e) => setEffect({ kind: e.target.value as Effect["kind"] })}
            >
              {Object.entries(EFFECT_LABEL).map(([id, label]) => (
                <option key={id} value={id}>
                  {label}
                </option>
              ))}
            </select>
          </label>

          {rule.effect.kind === "blockKind" && (
            <label className="field">
              <span className="field__label">{t("rulePanel.blockKind")}</span>
              <select
                className="input"
                value={rule.effect.value || "definition"}
                onChange={(e) => setEffect({ value: e.target.value })}
              >
                {Object.entries(KIND_LABEL).map(([id, label]) => (
                  <option key={id} value={id}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
          )}

          {rule.effect.kind === "custom" && (
            <label className="field">
              <span className="field__label">{t("rulePanel.customEffect")}</span>
              <textarea
                className="input"
                rows={3}
                value={rule.effect.value}
                onChange={(e) => setEffect({ value: e.target.value })}
                placeholder={t("rulePanel.customEffect.placeholder")}
              />
            </label>
          )}
        </section>
      </div>

      <footer className="panel-side__foot">
        <button type="button" className="btn btn--ghost" onClick={onDelete}>
          {t("common.delete")}
        </button>
      </footer>
    </aside>
  );
}
