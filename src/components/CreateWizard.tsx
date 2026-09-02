import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { createDocument } from "../api";
import { t, tn } from "../i18n";
import { isTauri } from "../platform";
import { logError } from "../log";
import type { ImportProgress, PlumeDocument, Template } from "../types";
import { Icon } from "../ui/Icon";
import { moved, useDragOrder } from "../ui/dragOrder";
import { Modal } from "./Modal";

const IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "heic", "heif", "webp", "tif", "tiff"];

const basename = (path: string) => path.split(/[\\/]/).pop() ?? path;
const isImage = (path: string) =>
  IMAGE_EXTENSIONS.includes(path.split(".").pop()?.toLowerCase() ?? "");

const STEP_KEYS = ["wizard.step.title", "wizard.step.pages", "wizard.step.check"] as const;

type Props = {
  templates: Template[];
  /** Photos dropped before the wizard opened — straight to step 2. */
  initialPages?: string[];
  onCancel: () => void;
  onCreated: (document: PlumeDocument) => void;
};

export function CreateWizard({ templates, initialPages = [], onCancel, onCreated }: Props) {
  const [step, setStep] = useState(0);
  const [title, setTitle] = useState("");
  const [templateId, setTemplateId] = useState(templates[0]?.id ?? "");
  const [pages, setPages] = useState<string[]>(initialPages);
  const [dragging, setDragging] = useState(false);
  const listRef = useRef<HTMLOListElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [imported, setImported] = useState<ImportProgress | null>(null);

  const template = templates.find((tpl) => tpl.id === templateId);

  useEffect(() => {
    if (!isTauri()) return;
    // If the listener fails (webview without drag-and-drop), the file picker
    // still works: that is not a reason to bring the wizard down.
    let stop: (() => void) | null = null;
    import("@tauri-apps/api/webview")
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((event) => {
          if (event.payload.type === "over") setDragging(true);
          else if (event.payload.type === "drop") {
            setDragging(false);
            addPages(event.payload.paths);
          } else setDragging(false);
        }),
      )
      .then((off) => {
        stop = off;
      })
      .catch(() => null);
    return () => stop?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // A dozen phone photos take a few seconds each to rotate and resample.
  useEffect(() => {
    const stop = listen<ImportProgress>("import", (event) => setImported(event.payload));
    return () => {
      stop.then((off) => off()).catch(() => {});
    };
  }, []);

  function addPages(candidates: string[]) {
    const images = candidates.filter(isImage);
    const ignored = candidates.length - images.length;
    setPages((current) => [...current, ...images.filter((p) => !current.includes(p))]);
    setError(ignored > 0 ? tn("wizard.ignored", ignored) : null);
  }

  async function pickPages() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
    });
    if (Array.isArray(picked)) addPages(picked);
    else if (typeof picked === "string") addPages([picked]);
  }

  // Shared with the course view, which reorders the same photographs later.
  const { held, grab } = useDragOrder(listRef, (from, to) =>
    setPages((current) => moved(current, from, to)),
  );

  function move(index: number, delta: number) {
    setPages((current) => {
      const next = [...current];
      const target = index + delta;
      if (target < 0 || target >= next.length) return current;
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }

  async function create() {
    setBusy(true);
    setError(null);
    try {
      onCreated(await createDocument(title, templateId, pages));
    } catch (cause) {
      setError(String(cause));
      logError("workspace", t("error.refresh"), cause);
      setBusy(false);
      setStep(2);
    }
  }

  const canContinue = step === 0 ? title.trim().length > 0 : pages.length > 0;

  return (
    <Modal
      title={t("wizard.title")}
      subtitle={t("wizard.subtitle", {
        step: step + 1,
        total: STEP_KEYS.length,
        name: t(STEP_KEYS[step]),
      })}
      onClose={onCancel}
      wide
      footer={
        <>
          <span className="modal__note">
            {template &&
              t("wizard.footer.template", { name: template.name })}
          </span>
          <div className="modal__buttons">
            {step > 0 && (
              <button type="button" className="btn btn--outline" onClick={() => setStep(step - 1)}>
                {t("common.back")}
              </button>
            )}
            {step < STEP_KEYS.length - 1 ? (
              <button
                type="button"
                className="btn btn--primary"
                onClick={() => setStep(step + 1)}
                disabled={!canContinue}
              >
                {t("common.continue")}
              </button>
            ) : (
              <button
                type="button"
                className="btn btn--primary"
                onClick={create}
                disabled={busy || !title.trim() || pages.length === 0}
              >
                {busy
                  ? imported
                    ? t("wizard.importing", { done: imported.done, total: imported.total })
                    : t("wizard.creating")
                  : t("wizard.create")}
              </button>
            )}
          </div>
        </>
      }
    >
      <ol className="wsteps">
        {STEP_KEYS.map((key, index) => (
          <li
            key={key}
            className={`wstep ${index === step ? "wstep--current" : ""} ${index < step ? "wstep--done" : ""}`}
          >
            <span className="wstep__dot">
              {index < step ? <Icon name="check" size={11} /> : index + 1}
            </span>
            {t(key)}
            {index < STEP_KEYS.length - 1 && <span className="wstep__line" />}
          </li>
        ))}
      </ol>

      {step === 0 && (
        <div className="stack stack--tight">
          <label className="field">
            <span className="field__label">{t("wizard.title.label")}</span>
            <input
              className="input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t("wizard.title.placeholder")}
              autoFocus
            />
          </label>

          {templates.length > 1 && (
            <div className="field">
              <span className="field__label">{t("wizard.template.label")}</span>
              <div className="picks">
                {templates.map((tpl) => (
                  <button
                    key={tpl.id}
                    type="button"
                    className={`pick ${tpl.id === templateId ? "pick--on" : ""}`}
                    onClick={() => setTemplateId(tpl.id)}
                  >
                    <span className="pick__title">{tpl.name}</span>
                    <span className="pick__meta">{tpl.description}</span>
                    <span className="pick__meta">
                      {tn("wizard.template.meta", tpl.keys.length, { engine: tpl.engine })}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {step === 1 && (
        <div className="stack stack--tight">
          <button
            type="button"
            className={`dropzone dropzone--accent ${dragging ? "dropzone--active" : ""}`}
            onClick={pickPages}
          >
            <Icon name="upload" size={24} />
            <span className="dropzone__title">{t("wizard.drop.title")}</span>
            <span className="dropzone__text">
              {t("wizard.drop.text", { browse: t("wizard.drop.browse") })}
            </span>
          </button>

          {pages.length > 0 && (
            <>
              <span className="field__label">{tn("wizard.order.title", pages.length)}</span>
              <ol className={`wpages ${held !== null ? "wpages--held" : ""}`} ref={listRef}>
                {pages.map((path, index) => (
                  <li
                    key={path}
                    className={`wpage ${held === index ? "wpage--held" : ""}`}
                  >
                    <span
                      className="wpage__grip"
                      onPointerDown={(event) => grab(event, index)}
                      aria-hidden="true"
                    >
                      <Icon name="grip" size={14} />
                    </span>
                    <span className="wpage__badge">{index + 1}</span>
                    <span className="wpage__name" title={path}>
                      {basename(path)}
                    </span>
                    <span className="wpage__actions">
                      <button
                        type="button"
                        className="icon-btn"
                        onClick={() => move(index, -1)}
                        disabled={index === 0}
                        aria-label={t("wizard.order.move.up")}
                      >
                        <Icon name="arrow-up" size={13} />
                      </button>
                      <button
                        type="button"
                        className="icon-btn"
                        onClick={() => move(index, 1)}
                        disabled={index === pages.length - 1}
                        aria-label={t("wizard.order.move.down")}
                      >
                        <Icon name="arrow-down" size={13} />
                      </button>
                      <button
                        type="button"
                        className="icon-btn"
                        onClick={() => setPages((c) => c.filter((p) => p !== path))}
                        aria-label={t("wizard.order.remove")}
                      >
                        <Icon name="close" size={13} />
                      </button>
                    </span>
                  </li>
                ))}
              </ol>
            </>
          )}
        </div>
      )}

      {step === 2 && (
        <dl className="recap">
          <dt>{t("wizard.recap.title")}</dt>
          <dd>{title}</dd>
          <dt>{t("wizard.recap.template")}</dt>
          <dd>{template?.name}</dd>
          <dt>{t("wizard.recap.pages")}</dt>
          <dd>{pages.length}</dd>
          <dt>{t("wizard.recap.first")}</dt>
          <dd>{basename(pages[0] ?? "—")}</dd>
        </dl>
      )}

      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}
    </Modal>
  );
}
