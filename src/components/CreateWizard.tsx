import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { createDocument } from "../api";
import { logError } from "../log";
import type { ImportProgress, PlumeDocument, Template } from "../types";
import { Modal } from "./Modal";

const IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "heic", "heif", "webp", "tif", "tiff"];

const basename = (path: string) => path.split(/[\\/]/).pop() ?? path;
const isImage = (path: string) =>
  IMAGE_EXTENSIONS.includes(path.split(".").pop()?.toLowerCase() ?? "");

const STEPS = ["Le cours", "Les pages", "Vérification"];

type Props = {
  templates: Template[];
  onCancel: () => void;
  onCreated: (document: PlumeDocument) => void;
};

export function CreateWizard({ templates, onCancel, onCreated }: Props) {
  const [step, setStep] = useState(0);
  const [title, setTitle] = useState("");
  const [templateId, setTemplateId] = useState(templates[0]?.id ?? "");
  const [pages, setPages] = useState<string[]>([]);
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [imported, setImported] = useState<ImportProgress | null>(null);

  const template = templates.find((t) => t.id === templateId);

  useEffect(() => {
    // If the listener fails (webview without drag-and-drop), the file picker
    // still works: that is not a reason to bring the wizard down.
    const unlisten = getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") setDragging(true);
        else if (event.payload.type === "drop") {
          setDragging(false);
          addPages(event.payload.paths);
        } else setDragging(false);
      })
      .catch(() => null);

    return () => {
      unlisten.then((stop) => stop?.()).catch(() => {});
    };
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
    setError(
      ignored > 0
        ? `${ignored} fichier${ignored > 1 ? "s" : ""} ignoré${ignored > 1 ? "s" : ""} : seules les images sont acceptées.`
        : null,
    );
  }

  async function pickPages() {
    const picked = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
    });
    if (Array.isArray(picked)) addPages(picked);
    else if (typeof picked === "string") addPages([picked]);
  }

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
      logError("workspace", "Création du cours impossible", cause);
      setBusy(false);
      setStep(2);
    }
  }

  const canContinue = step === 0 ? title.trim().length > 0 : pages.length > 0;

  return (
    <Modal
      title="Nouveau cours"
      subtitle={STEPS[step]}
      onClose={onCancel}
      wide
      footer={
        <>
          <span className="muted">
            Étape {step + 1} sur {STEPS.length}
          </span>
          <div className="modal__buttons">
            {step > 0 && (
              <button type="button" className="btn btn--ghost" onClick={() => setStep(step - 1)}>
                Retour
              </button>
            )}
            {step < STEPS.length - 1 ? (
              <button
                type="button"
                className="btn btn--primary"
                onClick={() => setStep(step + 1)}
                disabled={!canContinue}
              >
                Continuer
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
                    ? `Import ${imported.done} / ${imported.total}…`
                    : "Création…"
                  : "Créer le cours"}
              </button>
            )}
          </div>
        </>
      }
    >
      <ol className="steps steps--compact">
        {STEPS.map((label, index) => (
          <li
            key={label}
            className={`step ${index === step ? "step--current" : ""} ${index < step ? "step--done" : ""}`}
          >
            <span className="step__dot">{index + 1}</span>
            {label}
          </li>
        ))}
      </ol>

      {step === 0 && (
        <div className="stack stack--tight">
          <label className="field">
            <span className="field__label">Titre du cours</span>
            <input
              className="input"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Vecteurs"
              autoFocus
            />
          </label>

          <div className="field">
            <span className="field__label">Modèle</span>
            <div className="picks">
              {templates.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  className={`pick ${t.id === templateId ? "pick--on" : ""}`}
                  onClick={() => setTemplateId(t.id)}
                >
                  <span className="pick__title">{t.name}</span>
                  <span className="pick__meta">{t.description}</span>
                  <span className="pick__meta">
                    {t.keys.length} réglages · {t.engine}
                  </span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {step === 1 && (
        <div className="stack stack--tight">
          <div
            className={`dropzone ${dragging ? "dropzone--active" : ""}`}
            onClick={pickPages}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => e.key === "Enter" && pickPages()}
          >
            <p className="dropzone__text">
              Glissez vos photos ici, ou <span className="dropzone__link">parcourez</span>
            </p>
            <p className="dropzone__hint">L'ordre de la liste est celui des pages.</p>
          </div>

          {pages.length > 0 && (
            <ol className="pages">
              {pages.map((path, index) => (
                <li key={path} className="page">
                  <span className="page__number">{index + 1}</span>
                  <span className="page__name" title={path}>
                    {basename(path)}
                  </span>
                  <span className="page__actions">
                    <button
                      type="button"
                      className="icon-btn"
                      onClick={() => move(index, -1)}
                      disabled={index === 0}
                      aria-label="Monter"
                    >
                      ↑
                    </button>
                    <button
                      type="button"
                      className="icon-btn"
                      onClick={() => move(index, 1)}
                      disabled={index === pages.length - 1}
                      aria-label="Descendre"
                    >
                      ↓
                    </button>
                    <button
                      type="button"
                      className="icon-btn"
                      onClick={() => setPages((c) => c.filter((p) => p !== path))}
                      aria-label="Retirer"
                    >
                      ×
                    </button>
                  </span>
                </li>
              ))}
            </ol>
          )}
        </div>
      )}

      {step === 2 && (
        <dl className="recap">
          <dt>Titre</dt>
          <dd>{title}</dd>
          <dt>Modèle</dt>
          <dd>{template?.name}</dd>
          <dt>Pages</dt>
          <dd>{pages.length}</dd>
          <dt>Première page</dt>
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
