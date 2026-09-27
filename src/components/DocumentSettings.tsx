import { useState } from "react";
import { t, tn } from "../i18n";
import type { PlumeDocument, Template } from "../types";
import { groupKeys, KeyInput } from "./HouseStyleView";
import { Modal } from "./Modal";
import { TagField, withDraft } from "./TagEditor";

/**
 * A document's settings: what it is (its tags), which charte it is typeset
 * with, and the charte values changed for this document alone.
 *
 * The overrides are what lets one exercise sheet drop its « Nom / Classe »
 * line without touching the charte every other sheet shares. Only the keys
 * that differ from the charte are stored, so a colour changed later in « Mes
 * chartes » still reaches every value this document did not pin.
 */
type Props = {
  document: PlumeDocument;
  templates: Template[];
  /** Every tag in use across the workbook, for the suggestions. */
  known: string[];
  busy?: boolean;
  onCancel: () => void;
  onSave: (tags: string[], templateId: string, overrides: Record<string, string>) => Promise<void>;
};

/**
 * The charte as this document is typeset with it. Mirrors
 * `templates::for_document`: the preview has to promise what the PDF gives.
 */
export function withOverrides(
  template: Template | undefined,
  overrides: Record<string, string> | undefined,
): Template | undefined {
  if (!template || !overrides || Object.keys(overrides).length === 0) return template;
  return {
    ...template,
    keys: template.keys.map((key) =>
      key.key in overrides ? { ...key, value: overrides[key.key] } : key,
    ),
  };
}

export function DocumentSettings({ document, templates, known, busy, onCancel, onSave }: Props) {
  const [tags, setTags] = useState(document.tags);
  const [draft, setDraft] = useState("");
  const [templateId, setTemplateId] = useState(document.templateId);
  const [overrides, setOverrides] = useState<Record<string, string>>(
    document.templateOverrides ?? {},
  );

  const template = templates.find((tpl) => tpl.id === templateId);
  // Only the keys this charte has count: an override carried over from
  // another charte is kept while it still names one of its keys.
  const pinned = template
    ? Object.fromEntries(
        Object.entries(overrides).filter(([name, value]) =>
          template.keys.some((key) => key.key === name && key.value !== value),
        ),
      )
    : {};
  const effective = withOverrides(template, pinned);
  const pinnedCount = Object.keys(pinned).length;

  function edit(name: string, value: string) {
    setOverrides((current) => ({ ...current, [name]: value }));
  }

  function reset(name: string) {
    setOverrides((current) => {
      const { [name]: _dropped, ...rest } = current;
      return rest;
    });
  }

  return (
    <Modal
      wide
      title={t("docSettings.title")}
      subtitle={document.title}
      onClose={onCancel}
      footer={
        <>
          <span className="modal__note">
            {pinnedCount > 0 && tn("docSettings.pinned", pinnedCount)}
          </span>
          <div className="modal__buttons">
            <button type="button" className="btn btn--outline" onClick={onCancel} disabled={busy}>
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => onSave(withDraft(tags, draft), templateId, pinned)}
              disabled={busy || !template}
            >
              {busy ? t("common.saving") : t("common.save")}
            </button>
          </div>
        </>
      }
    >
      <section className="stack stack--tight">
        <h3 className="section-title">{t("tags.title")}</h3>
        <TagField tags={tags} onChange={setTags} draft={draft} onDraft={setDraft} known={known} />
      </section>

      <section className="stack stack--tight">
        <h3 className="section-title">{t("docSettings.template")}</h3>
        <select
          className="input"
          value={templateId}
          onChange={(event) => setTemplateId(event.target.value)}
        >
          {!template && <option value={templateId}>{t("docSettings.template.missing")}</option>}
          {templates.map((tpl) => (
            <option key={tpl.id} value={tpl.id}>
              {tpl.name}
            </option>
          ))}
        </select>
        {template?.description && <p className="field__hint">{template.description}</p>}
      </section>

      {effective && (
        <section className="stack stack--tight">
          <h3 className="section-title">{t("docSettings.keys")}</h3>
          <p className="field__hint">{t("docSettings.keys.hint")}</p>
          {groupKeys(effective.keys).map(([group, rows]) => (
            <div key={group} className="stack stack--tight">
              <span className="overline">{group}</span>
              <div className="stylelines">
                {rows.map((row) => {
                  const changed = row.keys.filter((key) => key.key in pinned);
                  return (
                    <div
                      key={row.keys[0].key}
                      className={`styleline ${changed.length > 0 ? "styleline--override" : ""}`}
                    >
                      <span className="styleline__name">
                        {row.label}
                        {changed.length > 0 && (
                          <button
                            type="button"
                            className="btn btn--link styleline__reset"
                            onClick={() => changed.forEach((key) => reset(key.key))}
                          >
                            {t("docSettings.reset")}
                          </button>
                        )}
                      </span>
                      {row.keys.map((key) => (
                        <span
                          key={key.key}
                          className={
                            key.type === "color"
                              ? "styleline__cell styleline__cell--color"
                              : "styleline__cell styleline__cell--value"
                          }
                        >
                          <KeyInput keyDef={key} onChange={(value) => edit(key.key, value)} />
                        </span>
                      ))}
                    </div>
                  );
                })}
              </div>
            </div>
          ))}
        </section>
      )}
    </Modal>
  );
}
