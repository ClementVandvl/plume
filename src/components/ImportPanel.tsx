import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  importCourse,
  importInstructions,
  inspectImport,
  inspectImportFile,
} from "../api";
import { t, tn } from "../i18n";
import { logError } from "../log";
import { latexToHtml } from "../preview/latexToHtml";
import { KIND_LABEL, type ImportPlan, type PlumeDocument, type Template } from "../types";
import { Icon } from "../ui/Icon";
import { Modal } from "./Modal";
import { splitTags } from "./TagEditor";

/**
 * Bringing in a document written somewhere other than on paper.
 *
 * A teacher asks Claude for an exercise sheet and wants to keep working on it —
 * split a statement from its answer, reserve a correction, apply their charte,
 * send the class the half they have covered. All of that already works on
 * passages, so the sheet only has to arrive as passages.
 *
 * Two steps on purpose. A document from outside is the one thing in Plume the
 * teacher did not write, and reading it before it enters the workbook is what
 * makes accepting it a decision rather than a surprise.
 */

type Props = {
  templates: Template[];
  onCancel: () => void;
  onImported: (document: PlumeDocument) => void;
};

export function ImportPanel({ templates, onCancel, onImported }: Props) {
  const [json, setJson] = useState("");
  const [plan, setPlan] = useState<ImportPlan | null>(null);
  const [title, setTitle] = useState("");
  const [tags, setTagsText] = useState("");
  const [templateId, setTemplateId] = useState(templates[0]?.id ?? "");
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const template = templates.find((tpl) => tpl.id === templateId);

  function accept(read: ImportPlan) {
    setPlan(read);
    setJson(read.source);
    setTitle(read.title);
    setTagsText(read.tags.join(", "));
    setError(null);
  }

  async function read() {
    setBusy(true);
    setError(null);
    try {
      accept(await inspectImport(json));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function pick() {
    const picked = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    const path = Array.isArray(picked) ? picked[0] : picked;
    if (!path) return;

    setBusy(true);
    setError(null);
    try {
      // Read on the Rust side, which is the side that can reach the disk; the
      // text comes back with it, so the commit sees the same characters.
      accept(await inspectImportFile(path));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function copyInstructions() {
    try {
      await navigator.clipboard.writeText(await importInstructions());
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch (cause) {
      setError(String(cause));
      logError("interface", "Copie impossible", cause);
    }
  }

  async function commit() {
    setBusy(true);
    setError(null);
    try {
      onImported(await importCourse(json, title, templateId, splitTags(tags)));
    } catch (cause) {
      setError(String(cause));
      logError("workspace", "Import impossible", cause);
      setBusy(false);
    }
  }

  return (
    <Modal
      title={t("import.title")}
      subtitle={plan ? t("import.subtitle.check") : t("import.subtitle.paste")}
      onClose={onCancel}
      wide
      footer={
        <>
          <span className="modal__note">
            {plan && template && t("wizard.footer.template", { name: template.name })}
          </span>
          <div className="modal__buttons">
            {plan ? (
              <>
                <button
                  type="button"
                  className="btn btn--outline"
                  onClick={() => setPlan(null)}
                  disabled={busy}
                >
                  {t("common.back")}
                </button>
                <button
                  type="button"
                  className="btn btn--primary"
                  onClick={commit}
                  disabled={busy || !title.trim() || !templateId}
                >
                  {busy ? t("import.importing") : t("import.commit")}
                </button>
              </>
            ) : (
              <button
                type="button"
                className="btn btn--primary"
                onClick={read}
                disabled={busy || !json.trim()}
              >
                {busy ? t("import.reading") : t("common.continue")}
              </button>
            )}
          </div>
        </>
      }
    >
      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}

      {!plan ? (
        <section className="stack stack--tight">
          <p className="field__hint">{t("import.explain")}</p>

          <div className="import__ways">
            <button
              type="button"
              className="btn btn--soft btn--sm"
              onClick={copyInstructions}
            >
              {copied ? t("import.copied") : t("import.copy")}
            </button>
            <button type="button" className="btn btn--outline btn--sm" onClick={pick}>
              {t("import.pick")}
            </button>
          </div>

          <label className="field">
            <span className="field__label">{t("import.paste")}</span>
            <textarea
              className="input input--code"
              rows={12}
              spellCheck={false}
              value={json}
              placeholder={t("import.paste.placeholder")}
              onChange={(event) => setJson(event.target.value)}
            />
          </label>
        </section>
      ) : (
        <section className="stack stack--tight">
          <div className="keys">
            <label className="key">
              <span className="key__label">{t("import.name")}</span>
              <input
                className="input input--compact"
                value={title}
                placeholder={t("import.name.placeholder")}
                onChange={(event) => setTitle(event.target.value)}
              />
            </label>
            <label className="key">
              <span className="key__label">{t("import.tags")}</span>
              <input
                className="input input--compact"
                value={tags}
                placeholder={t("import.tags.placeholder")}
                title={t("import.tags.hint")}
                onChange={(event) => setTagsText(event.target.value)}
              />
            </label>
            <label className="key">
              <span className="key__label">{t("import.charte")}</span>
              <select
                className="input input--compact"
                value={templateId}
                onChange={(event) => setTemplateId(event.target.value)}
              >
                {templates.map((tpl) => (
                  <option key={tpl.id} value={tpl.id}>
                    {tpl.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {plan.warnings.length > 0 && (
            <div className="notice notice--warn">
              <span className="notice__title">
                {tn("import.warnings", plan.warnings.length)}
              </span>
              <ul className="notice__list">
                {plan.warnings.map((warning, index) => (
                  <li key={index}>{warning}</li>
                ))}
              </ul>
            </div>
          )}

          <span className="overline">
            {tn("import.passages", plan.blocks.length)}
          </span>

          {/* The passages as they will arrive, in order. Not the finished
              typesetting — that is the charte's answer, and the review shows
              it — but enough to recognise the sheet that was asked for. */}
          <ol className="importlist">
            {plan.blocks.map((block, index) => {
              const teacherOnly =
                block.audience.length > 0 && !block.audience.includes("student");
              return (
                <li key={index} className="importrow">
                  <span className="importrow__head">
                    <span className="importrow__kind">
                      {KIND_LABEL[block.kind] ?? block.kind}
                    </span>
                    {block.number && (
                      <span className="importrow__number">{block.number}</span>
                    )}
                    {block.title && (
                      <span className="importrow__title">{block.title}</span>
                    )}
                    {teacherOnly && (
                      <span className="importrow__tag">
                        <Icon name="check" size={10} />
                        {t("preview.tag.teacher")}
                      </span>
                    )}
                  </span>
                  {block.latex && (
                    <div
                      className="importrow__body"
                      dangerouslySetInnerHTML={{ __html: latexToHtml(block.latex) }}
                    />
                  )}
                </li>
              );
            })}
          </ol>
        </section>
      )}
    </Modal>
  );
}
