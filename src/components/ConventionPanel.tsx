import { useEffect } from "react";
import { t } from "../i18n";
import type { Convention } from "../types";

type Props = {
  convention: Convention;
  onChange: (convention: Convention) => void;
  onDelete: () => void;
  onClose: () => void;
  /** Drawn over the page as a drawer, instead of embedded in a column. */
  float?: boolean;
};

export function ConventionPanel({ convention, onChange, onDelete, onClose, float }: Props) {
  // A drawer closes on Escape, like any transient surface.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside className={`panel-side ${float ? "panel-side--float" : ""}`}>
      <header className="panel-side__head">
        <div>
          <span className="panel-side__kind">{t("conventionPanel.title")}</span>
          <span className="panel-side__meta">
            {convention.enabled ? t("rulePanel.active") : t("instructions.disabled")}
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
            checked={convention.enabled}
            onChange={(e) => onChange({ ...convention, enabled: e.target.checked })}
          />
          {t("conventionPanel.apply")}
        </label>

        <label className="field">
          <span className="field__label">{t("conventionPanel.name")}</span>
          <input
            className="input"
            value={convention.title}
            onChange={(e) => onChange({ ...convention, title: e.target.value })}
            placeholder={t("conventionPanel.name.placeholder")}
          />
          <span className="field__hint">{t("conventionPanel.name.hint")}</span>
        </label>

        <label className="field">
          <span className="field__label">{t("conventionPanel.text")}</span>
          <textarea
            className="input"
            rows={12}
            value={convention.text}
            onChange={(e) => onChange({ ...convention, text: e.target.value })}
            placeholder={t("conventionPanel.text.placeholder")}
          />
          <span className="field__hint">{t("conventionPanel.text.hint")}</span>
        </label>
      </div>

      <footer className="panel-side__foot">
        <button type="button" className="btn btn--ghost" onClick={onDelete}>
          {t("common.delete")}
        </button>
      </footer>
    </aside>
  );
}
