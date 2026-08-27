import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  addPages,
  applyCorrections,
  buildDocument,
  cancelCorrections,
  cancelTranscription,
  deleteDocument,
  removePage,
  renameDocument,
  documentPagePaths,
  getDocument,
  loadTranscript,
  revealPath,
  saveBlock,
  setBlockNote,
  setReadingRules,
  transcribeDocument,
} from "../api";
import {
  DOUBT_THRESHOLD,
  STEPS,
  type Block,
  type BuildResult,
  type CorrectionProgress,
  type PlumeDocument,
  type StepId,
  type TranscriptionProgress,
  type Transcript,
  type Template,
  type PageStateEvent,
  type HeartbeatEvent,
  type ScanInfo,
} from "../types";
import { logError, logInfo } from "../log";
import { useConfirm } from "../confirm";
import { BlockPanel } from "./BlockPanel";
import { DocumentPreview } from "./DocumentPreview";
import "katex/dist/katex.min.css";

const MODELS = [
  { id: "sonnet", label: "Sonnet — rapide, recommandé" },
  { id: "opus", label: "Opus — plus lent, plus fin" },
  { id: "fable", label: "Fable" },
];

const AUDIENCES = [
  { id: "all", label: "Version complète" },
  { id: "teacher", label: "Version professeur" },
  { id: "student", label: "Version élève" },
];

const money = (usd: number) => `${usd.toFixed(2)} $`;

type Props = {
  documentId: string;
  defaultModel: string;
  templates: Template[];
  onBack: () => void;
  onChanged: () => void;
  onDeleted: () => void;
};

export function CourseView({
  documentId,
  defaultModel,
  templates,
  onBack,
  onChanged,
  onDeleted,
}: Props) {
  const [document, setDocument] = useState<PlumeDocument | null>(null);
  const [pagePaths, setPagePaths] = useState<string[]>([]);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [step, setStep] = useState<StepId>("pages");
  const [model, setModel] = useState(defaultModel);
  const [audience, setAudience] = useState("all");
  const [rules, setRules] = useState("");
  const [progress, setProgress] = useState<TranscriptionProgress | null>(null);
  const [scan, setScan] = useState<Record<number, ScanInfo>>({});
  const [correcting, setCorrecting] = useState<CorrectionProgress | null>(null);
  const [openBlock, setOpenBlock] = useState<string | null>(null);
  const [filter, setFilter] = useState("all");
  const [running, setRunning] = useState(false);
  const [build, setBuild] = useState<BuildResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { confirm, promptFor } = useConfirm();

  const refresh = useCallback(async () => {
    const [doc, paths, existing] = await Promise.all([
      getDocument(documentId),
      documentPagePaths(documentId),
      loadTranscript(documentId),
    ]);
    setDocument(doc);
    setRules(doc.readingRules ?? "");
    setPagePaths(paths);
    setTranscript(existing);
    return existing;
  }, [documentId]);

  useEffect(() => {
    refresh()
      .then((existing) => setStep(existing ? "review" : "pages"))
      .catch((cause) => setError(String(cause)));
  }, [refresh]);

  useEffect(() => {
    const stop = listen<TranscriptionProgress>("transcription", (event) => {
      if (event.payload.documentId === documentId) setProgress(event.payload);
    });
    return () => {
      stop.then((off) => off()).catch(() => {});
    };
  }, [documentId]);

  // The timeline: page lifecycles plus heartbeats. A heartbeat only refreshes
  // the label — the state machine belongs to page-state events.
  useEffect(() => {
    const states = listen<PageStateEvent>("page-state", (event) => {
      if (event.payload.documentId !== documentId) return;
      const { page, state, blocks, message } = event.payload;
      setScan((current) => ({
        ...current,
        [page]: { ...current[page], state, blocks, message: message ?? undefined },
      }));
    });
    const beats = listen<HeartbeatEvent>("heartbeat", (event) => {
      if (event.payload.documentId !== documentId) return;
      const { page, label } = event.payload;
      setScan((current) => ({
        ...current,
        [page]: { ...(current[page] ?? { state: "reading" }), state: "reading", label },
      }));
    });
    return () => {
      states.then((off) => off()).catch(() => {});
      beats.then((off) => off()).catch(() => {});
    };
  }, [documentId]);

  useEffect(() => {
    const stop = listen<CorrectionProgress>("correction", (event) => {
      if (event.payload.documentId === documentId) setCorrecting(event.payload);
    });
    return () => {
      stop.then((off) => off()).catch(() => {});
    };
  }, [documentId]);

  const template = templates.find((t) => t.id === document?.templateId);
  const keptFor = (who: string) =>
    blocks.filter((b) => b.audience.length === 0 || b.audience.includes(who)).length;
  const blocks = transcript?.pages.flatMap((p) => p.blocks) ?? [];
  const selected = transcript?.pages
    .flatMap((page) => page.blocks.map((block) => ({ block, page: page.number })))
    .find((entry) => entry.block.id === openBlock);
  const doubtful = blocks.filter((b) => b.confidence < DOUBT_THRESHOLD && !b.reviewed);
  const annotated = blocks.filter((b) => b.note);
  const teacherOnly = blocks.filter(
    (b) => b.audience.length > 0 && !b.audience.includes("student"),
  );
  const studentOnly = blocks.filter(
    (b) => b.audience.length > 0 && !b.audience.includes("teacher"),
  );

  const FILTERS = [
    { id: "all", label: "Tous", count: blocks.length },
    { id: "doubt", label: "À vérifier", count: doubtful.length },
    { id: "teacher", label: "Prof seul", count: teacherOnly.length },
    { id: "student", label: "Élève seul", count: studentOnly.length },
  ];

  const done: Record<StepId, boolean> = {
    pages: pagePaths.length > 0,
    read: blocks.length > 0,
    review: blocks.length > 0 && doubtful.length === 0 && annotated.length === 0,
    export: build?.pdfPath != null,
  };

  async function stopReading() {
    try {
      const stopped = await cancelTranscription(documentId);
      logInfo("claude", `Annulation demandée — ${stopped} processus arrêté(s)`);
    } catch (cause) {
      logError("claude", "Annulation impossible", cause);
    }
  }

  async function stopCorrecting() {
    try {
      const stopped = await cancelCorrections(documentId);
      logInfo("claude", `Annulation demandée — ${stopped} processus arrêté(s)`);
    } catch (cause) {
      logError("claude", "Annulation impossible", cause);
    }
  }

  async function read() {
    // Re-reading throws away every manual edit, confirmation and annotation.
    if (blocks.length > 0) {
      const ok = await confirm({
        title: "Relire toutes les pages ?",
        message: `La transcription actuelle (${blocks.length} blocs) est remplacée, avec vos corrections manuelles, vos confirmations et vos annotations.`,
        detail: `${pagePaths.length} page(s) seront relues et consommeront du quota.`,
        confirmLabel: "Relire et remplacer",
        tone: "danger",
      });
      if (!ok) return;
    }

    setRunning(true);
    setError(null);
    setBuild(null);
    setProgress(null);
    setScan({});
    try {
      setTranscript(await transcribeDocument(documentId, model));
      setStep("review");
      onChanged();
    } catch (cause) {
      setError(String(cause));
      logError("claude", String(cause));
    } finally {
      setRunning(false);
    }
  }

  async function correct() {
    setRunning(true);
    setError(null);
    setCorrecting(null);
    try {
      setTranscript(await applyCorrections(documentId, model));
    } catch (cause) {
      setError(String(cause));
      logError("claude", String(cause));
    } finally {
      setRunning(false);
    }
  }

  async function compile() {
    setError(null);
    try {
      setBuild(await buildDocument(documentId, audience));
    } catch (cause) {
      setError(String(cause));
      logError("claude", String(cause));
    }
  }

  async function pickAndAdd() {
    setError(null);
    const picked = await open({
      multiple: true,
      filters: [
        {
          name: "Images",
          extensions: ["jpg", "jpeg", "png", "heic", "heif", "webp", "tif", "tiff"],
        },
      ],
    });
    const sources = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (sources.length === 0) return;

    try {
      await addPages(documentId, sources);
      await refresh();
      onChanged();
    } catch (cause) {
      setError(String(cause));
      logError("workspace", "Ajout de pages impossible", cause);
    }
  }

  async function dropPage(number: number) {
    const hasTranscript = (transcript?.pages.length ?? 0) > 0;
    const ok = await confirm({
      title: `Retirer la page ${number} ?`,
      message: "La photo est supprimée du cours. Votre photothèque n'est pas touchée.",
      detail: hasTranscript
        ? "La transcription de cette page est supprimée et les pages suivantes sont renumérotées. Les autres pages sont conservées."
        : undefined,
      confirmLabel: "Retirer la page",
      tone: "danger",
    });
    if (!ok) return;

    try {
      await removePage(documentId, number);
      await refresh();
      onChanged();
    } catch (cause) {
      setError(String(cause));
      logError("workspace", "Suppression de page impossible", cause);
    }
  }

  async function rename() {
    const title = await promptFor({
      title: "Renommer le cours",
      message: "Le dossier sur le disque garde son nom ; seul le titre affiché change.",
      confirmLabel: "Renommer",
      input: { label: "Titre", value: document?.title ?? "", placeholder: "Vecteurs" },
    });
    if (!title || !title.trim()) return;
    try {
      await renameDocument(documentId, title);
      await refresh();
      onChanged();
    } catch (cause) {
      setError(String(cause));
      logError("workspace", "Renommage impossible", cause);
    }
  }

  async function remove() {
    const ok = await confirm({
      title: `Mettre « ${document?.title} » à la corbeille ?`,
      message:
        "Le dossier du cours est déplacé dans Plume/Corbeille — photos, transcription et fichiers produits compris.",
      detail: "Rien n'est effacé : videz la corbeille vous-même depuis le Finder.",
      confirmLabel: "Mettre à la corbeille",
      tone: "danger",
    });
    if (!ok) return;
    try {
      await deleteDocument(documentId);
      onDeleted();
    } catch (cause) {
      setError(String(cause));
      logError("workspace", "Mise à la corbeille impossible", cause);
    }
  }

  async function persist(block: Block) {
    await saveBlock(documentId, block);
    setTranscript(await loadTranscript(documentId));
  }

  async function annotate(blockId: string, note: string | null) {
    await setBlockNote(documentId, blockId, note);
    setTranscript(await loadTranscript(documentId));
  }

  if (!document) return <p className="muted">Chargement…</p>;

  return (
    <div className="workspace">
      {/* Deep view: it takes the whole window. The sidebar would only compete
          with the document being reviewed. */}
      <header className="workspace__bar">
        <button type="button" className="btn btn--ghost" onClick={onBack}>
          ← Mes cours
        </button>
        <div className="workspace__identity">
          <span className="workspace__title">{document.title}</span>
          <span className="workspace__meta">
            {document.pageCount} page{document.pageCount > 1 ? "s" : ""} · {document.templateId}
          </span>
        </div>

        <div className="workspace__tools">
          <button type="button" className="btn btn--ghost" onClick={rename}>
            Renommer
          </button>
          <button type="button" className="btn btn--ghost" onClick={remove}>
            Corbeille
          </button>
        </div>

        <ol className="steps steps--bar">
          {STEPS.map((s, index) => (
            <li
              key={s.id}
              className={`step ${step === s.id ? "step--current" : ""} ${done[s.id] ? "step--done" : ""}`}
            >
              <button type="button" className="step__button" onClick={() => setStep(s.id)}>
                <span className="step__dot">{done[s.id] ? "✓" : index + 1}</span>
                {s.label}
              </button>
            </li>
          ))}
        </ol>
      </header>

      <div className={`workspace__body ${step === "review" ? "workspace__body--flush" : ""}`}>
        {error && (
          <p className="notice notice--error" role="alert">
            {error}
          </p>
        )}

      {step === "pages" && (
        <section className="stack stack--tight">
          <div className="section-head">
            <h2 className="section-title">Pages importées</h2>
            <button type="button" className="btn btn--ghost" onClick={pickAndAdd}>
              Ajouter des pages
            </button>
          </div>
          <div className="thumbs">
            {pagePaths.map((path, index) => (
              <figure key={path} className="thumb">
                <img src={convertFileSrc(path)} alt={`Page ${index + 1}`} loading="lazy" />
                <figcaption className="thumb__foot">
                  <span>Page {index + 1}</span>
                  <button
                    type="button"
                    className="icon-btn"
                    onClick={() => dropPage(index + 1)}
                    aria-label={`Retirer la page ${index + 1}`}
                  >
                    ×
                  </button>
                </figcaption>
              </figure>
            ))}
          </div>

          <label className="field">
            <span className="field__label">Règles de lecture</span>
            <textarea
              className="input"
              rows={4}
              value={rules}
              onChange={(e) => setRules(e.target.value)}
              placeholder="Ce que je surligne en orange doit être en gras. Un trait bleu devant un paragraphe signifie qu'il est réservé à ma version."
            />
            <span className="field__hint">
              Vos conventions, dans vos mots. Elles sont transmises telles quelles et
              font autorité sur les règles par défaut.
            </span>
          </label>
          <div className="row-actions">
            <button
              type="button"
              className="btn btn--ghost"
              onClick={() =>
                setReadingRules(documentId, rules)
                  .then(refresh)
                  .catch((cause) => setError(String(cause)))
              }
              disabled={rules === (document.readingRules ?? "")}
            >
              Enregistrer les règles
            </button>
            <button type="button" className="btn btn--primary" onClick={() => setStep("read")}>
              Continuer
            </button>
          </div>
        </section>
      )}

      {step === "read" && (
        <section className="stack stack--tight">
          <h2 className="section-title">Lecture des pages</h2>
          <label className="field field--half">
            <span className="field__label">Modèle de lecture</span>
            <select
              className="input"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              disabled={running}
            >
              {MODELS.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}
                </option>
              ))}
            </select>
            <span className="field__hint">
              {pagePaths.length} page{pagePaths.length > 1 ? "s" : ""} à lire. Le
              parallélisme suit vos réglages.
            </span>
          </label>

          <ol className="scan">
            {pagePaths.map((path, index) => {
              const number = index + 1;
              const info = scan[number];
              const already = transcript?.pages.find((p) => p.number === number);
              const state = info?.state ?? (already ? "done" : "waiting");
              const blocks = info?.blocks || already?.blocks.length || 0;
              return (
                <li key={path} className={`scan__row scan__row--${state}`}>
                  <img className="scan__thumb" src={convertFileSrc(path)} alt="" />
                  <div className="scan__body">
                    <span className="scan__title">Page {number}</span>
                    <span className="scan__label">
                      {state === "reading"
                        ? (info?.label ?? "Lecture en cours…")
                        : state === "done"
                          ? `${blocks} bloc${blocks > 1 ? "s" : ""}`
                          : state === "failed"
                            ? (info?.message ?? "Échec de lecture")
                            : state === "cancelled"
                              ? "Annulée — pages déjà lues conservées"
                              : "En attente"}
                    </span>
                  </div>
                  <span className={`scan__dot scan__dot--${state}`} aria-hidden="true" />
                </li>
              );
            })}
          </ol>

          {progress && (
            <p className={`notice ${progress.phase === "failed" ? "notice--error" : ""}`}>
              {progress.phase === "failed"
                ? `Page ${progress.page} : ${progress.message}`
                : progress.phase === "cancelled"
                  ? `Lecture annulée — ${progress.blocks} blocs conservés sur les pages déjà lues, ${money(progress.costUsd)} dépensés.`
                  : progress.phase === "done"
                    ? `Lecture terminée — ${progress.blocks} blocs, ${money(progress.costUsd)}.`
                    : `Page ${progress.page} sur ${progress.total} — ${money(progress.costUsd)} cumulés.`}
            </p>
          )}

          <div className="row-actions">
            {running && (
              <button type="button" className="btn btn--ghost" onClick={stopReading}>
                Annuler la lecture
              </button>
            )}
            <button
              type="button"
              className="btn btn--primary"
              onClick={read}
              disabled={running || pagePaths.length === 0}
            >
              {running ? "Lecture en cours…" : blocks.length ? "Relire les pages" : "Lire les pages"}
            </button>
          </div>
        </section>
      )}

      {step === "review" && (
        <section className="review">
          <div className="review__bar">
            <div className="chips">
              {FILTERS.map((entry) => (
                <button
                  key={entry.id}
                  type="button"
                  className={`chip ${filter === entry.id ? "chip--on" : ""}`}
                  onClick={() => setFilter(entry.id)}
                  disabled={entry.count === 0 && entry.id !== "all"}
                >
                  {entry.label}
                  <span className="chip__count">{entry.count}</span>
                </button>
              ))}
            </div>

            <div className="review__actions">
              {running && (
                <button type="button" className="btn btn--ghost" onClick={stopCorrecting}>
                  Annuler la correction
                </button>
              )}
              {annotated.length > 0 && !running && (
                <button type="button" className="btn btn--ghost" onClick={correct}>
                  Corriger {annotated.length} bloc{annotated.length > 1 ? "s" : ""}
                </button>
              )}
              <button type="button" className="btn btn--primary" onClick={() => setStep("export")}>
                {doubtful.length > 0 || annotated.length > 0
                  ? "Passer à l'export malgré tout"
                  : "Passer à l'export"}
              </button>
            </div>
          </div>

          {correcting && (
            <p className={`notice ${correcting.phase === "failed" ? "notice--error" : ""}`}>
              {correcting.phase === "done"
                ? `Correction terminée — ${correcting.done} sur ${correcting.total}.`
                : correcting.phase === "cancelled"
                  ? `Corrections annulées — ${correcting.done} sur ${correcting.total} appliquée${correcting.done > 1 ? "s" : ""}, conservée${correcting.done > 1 ? "s" : ""}.`
                  : correcting.phase === "failed"
                    ? `${correcting.blockId} : ${correcting.message}`
                    : `Bloc ${correcting.done} sur ${correcting.total} corrigé.`}
            </p>
          )}

          {blocks.length === 0 ? (
            <p className="muted">
              Ce cours n'a pas encore été lu. Revenez à l'étape « Lecture ».
            </p>
          ) : (
            <div className={`review__split ${selected ? "review__split--open" : ""}`}>
              <div className="review__paper">
                {transcript && (
                  <DocumentPreview
                    documentId={documentId}
                    transcript={transcript}
                    filter={filter}
                    template={template}
                    selectedId={openBlock}
                    onSelect={setOpenBlock}
                  />
                )}
              </div>

              {selected && (
                <BlockPanel
                  block={selected.block}
                  page={selected.page}
                  pageSrc={pagePaths[selected.page - 1]}
                  onClose={() => setOpenBlock(null)}
                  onSave={persist}
                  onNote={(note) => annotate(selected.block.id, note)}
                />
              )}
            </div>
          )}
        </section>
      )}

      {step === "export" && (
        <section className="stack stack--tight">
          <h2 className="section-title">Export</h2>
          <div className="export">
            <div className="picks">
              {AUDIENCES.map((a) => {
                const kept = a.id === "all" ? blocks.length : keptFor(a.id);
                const removed = blocks.length - kept;
                return (
                  <button
                    key={a.id}
                    type="button"
                    className={`pick ${a.id === audience ? "pick--on" : ""}`}
                    onClick={() => setAudience(a.id)}
                  >
                    <span className="pick__title">{a.label}</span>
                    <span className="pick__meta">
                      {kept} bloc{kept > 1 ? "s" : ""}
                      {removed > 0 ? ` · ${removed} retiré${removed > 1 ? "s" : ""}` : ""}
                    </span>
                  </button>
                );
              })}
            </div>

            {keptFor("student") === blocks.length && (
              <p className="field__hint">
                Aucun bloc n'est réservé au professeur : les trois versions sont
                identiques. Marquez un bloc « Professeur » seulement dans la
                relecture, ou décrivez votre convention dans les règles de lecture.
              </p>
            )}

            <div className="row-actions">
              <button
                type="button"
                className="btn btn--primary"
                onClick={compile}
                disabled={blocks.length === 0}
              >
                Générer le LaTeX et le PDF
              </button>
            </div>
          </div>

          {build?.pdfPath && (
            <iframe
              className="pdf-preview"
              src={convertFileSrc(build.pdfPath)}
              title="Aperçu du PDF"
            />
          )}

          {build && (
            <div className={`notice ${build.error ? "notice--error" : ""}`}>
              {build.error
                ? `Le .tex est écrit, mais la compilation a échoué : ${build.error}`
                : "Document généré."}
              <span className="notice__actions">
                <button
                  type="button"
                  className="btn btn--link"
                  onClick={() => revealPath(build.texPath).catch((cause) => logError("interface", "Ouverture du fichier impossible", cause))}
                >
                  Ouvrir le .tex
                </button>
                {build.pdfPath && (
                  <button
                    type="button"
                    className="btn btn--link"
                    onClick={() => revealPath(build.pdfPath!).catch((cause) => logError("interface", "Ouverture du fichier impossible", cause))}
                  >
                    Ouvrir le PDF
                  </button>
                )}
              </span>
            </div>
          )}
        </section>
      )}
      </div>
    </div>
  );
}
