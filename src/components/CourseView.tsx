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
import { formatMoney, t, tn } from "../i18n";
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
import { useAdvanced } from "../ui/mode";
import { Icon } from "../ui/Icon";
import { AdvancedRow, Meter, OverflowMenu } from "../ui/controls";
import { BlockPanel } from "./BlockPanel";
import { DocumentPreview } from "./DocumentPreview";
import "katex/dist/katex.min.css";

const MODELS = [
  { id: "sonnet", labelKey: "model.sonnet" },
  { id: "opus", labelKey: "model.opus" },
  { id: "fable", labelKey: "model.fable" },
] as const;

type Props = {
  documentId: string;
  initialStep?: StepId;
  defaultModel: string;
  concurrentPages: number;
  templates: Template[];
  onBack: () => void;
  onChanged: () => void;
  onDeleted: () => void;
};

export function CourseView({
  documentId,
  initialStep,
  defaultModel,
  concurrentPages,
  templates,
  onBack,
  onChanged,
  onDeleted,
}: Props) {
  const advanced = useAdvanced();
  const [document, setDocument] = useState<PlumeDocument | null>(null);
  const [pagePaths, setPagePaths] = useState<string[]>([]);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [step, setStep] = useState<StepId>(initialStep ?? "pages");
  const [model, setModel] = useState(defaultModel);
  const [audience, setAudience] = useState("teacher");
  const [rules, setRules] = useState("");
  const [progress, setProgress] = useState<TranscriptionProgress | null>(null);
  const [scan, setScan] = useState<Record<number, ScanInfo>>({});
  const [correcting, setCorrecting] = useState<CorrectionProgress | null>(null);
  const [openBlock, setOpenBlock] = useState<string | null>(null);
  const [filter, setFilter] = useState("doubt");
  const [running, setRunning] = useState(false);
  const [building, setBuilding] = useState(false);
  const [build, setBuild] = useState<BuildResult | null>(null);
  const [texOpen, setTexOpen] = useState(false);
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
      .then((existing) => {
        if (!initialStep) setStep(existing ? "review" : "pages");
      })
      .catch((cause) => setError(String(cause)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
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

  const template = templates.find((tpl) => tpl.id === document?.templateId);
  const blocks = transcript?.pages.flatMap((p) => p.blocks) ?? [];
  const keptFor = (who: string) =>
    blocks.filter((b) => b.audience.length === 0 || b.audience.includes(who)).length;
  const doubtful = blocks.filter((b) => b.confidence < DOUBT_THRESHOLD && !b.reviewed);
  const annotated = blocks.filter((b) => b.note);
  const teacherOnly = blocks.filter(
    (b) => b.audience.length > 0 && !b.audience.includes("student"),
  );
  const studentOnly = blocks.filter(
    (b) => b.audience.length > 0 && !b.audience.includes("teacher"),
  );

  // The review list in reading order, narrowed by the active filter — the
  // sequence ↑/↓ walks through.
  const sequence = (transcript?.pages ?? [])
    .flatMap((page) => page.blocks.map((block) => ({ block, page: page.number })))
    .filter(({ block }) => {
      if (filter === "doubt") return block.confidence < DOUBT_THRESHOLD && !block.reviewed;
      if (filter === "teacher")
        return block.audience.length > 0 && !block.audience.includes("student");
      if (filter === "student")
        return block.audience.length > 0 && !block.audience.includes("teacher");
      return true;
    });
  const all = (transcript?.pages ?? []).flatMap((page) =>
    page.blocks.map((block) => ({ block, page: page.number })),
  );
  const selected = all.find((entry) => entry.block.id === openBlock);
  const selectedAt = all.findIndex((entry) => entry.block.id === openBlock);

  function stepTo(delta: number) {
    if (all.length === 0) return;
    const pool = sequence.length > 0 ? sequence : all;
    const at = pool.findIndex((entry) => entry.block.id === openBlock);
    const next = pool[(at < 0 ? 0 : at + delta + pool.length) % pool.length];
    if (next) setOpenBlock(next.block.id);
  }

  const FILTERS = [
    { id: "doubt", label: t("review.filter.doubt"), count: doubtful.length },
    { id: "all", label: t("review.filter.all"), count: blocks.length },
    { id: "teacher", label: t("review.filter.teacher"), count: teacherOnly.length },
    { id: "student", label: t("review.filter.student"), count: studentOnly.length },
  ];

  const done: Record<StepId, boolean> = {
    pages: pagePaths.length > 0,
    read: blocks.length > 0,
    review: blocks.length > 0 && doubtful.length === 0 && annotated.length === 0,
    export: (build?.pdfPath ?? document?.lastPdf) != null,
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
        title: t("read.reread.title"),
        message: t("read.reread.message", { blocks: blocks.length }),
        detail: tn("read.reread.detail", pagePaths.length),
        confirmLabel: t("read.reread.confirm"),
        tone: "danger",
      });
      if (!ok) return;
    }

    setStep("read");
    setRunning(true);
    setError(null);
    setBuild(null);
    setProgress(null);
    setScan({});
    try {
      setTranscript(await transcribeDocument(documentId, model));
      setStep("review");
      onChanged();
      refresh().catch(() => {});
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
      refresh().catch(() => {});
    } catch (cause) {
      setError(String(cause));
      logError("claude", String(cause));
    } finally {
      setRunning(false);
    }
  }

  async function compile() {
    setError(null);
    setBuilding(true);
    try {
      setBuild(await buildDocument(documentId, audience));
      onChanged();
      refresh().catch(() => {});
    } catch (cause) {
      setError(String(cause));
      logError("claude", String(cause));
    } finally {
      setBuilding(false);
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
      title: t("pages.remove.title", { number }),
      message: t("pages.remove.message"),
      detail: hasTranscript ? t("pages.remove.detail") : undefined,
      confirmLabel: t("pages.remove.confirm"),
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
      title: t("course.rename.title"),
      message: t("course.rename.message"),
      confirmLabel: t("course.rename.confirm"),
      input: {
        label: t("course.rename.field"),
        value: document?.title ?? "",
        placeholder: t("wizard.title.placeholder"),
      },
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
      title: t("course.trash.title", { title: document?.title ?? "" }),
      message: t("course.trash.message"),
      confirmLabel: t("course.trash.confirm"),
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

  // Review shortcuts: Entrée valide le passage ouvert, ↑/↓ naviguent. Typing
  // fields keep their keys — the listener steps aside for them.
  useEffect(() => {
    if (step !== "review") return;
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName ?? "";
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (event.key === "ArrowDown") {
        event.preventDefault();
        stepTo(1);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        stepTo(-1);
      } else if (event.key === "Enter" && selected) {
        event.preventDefault();
        persist({ ...selected.block, reviewed: true }).catch((cause) =>
          setError(String(cause)),
        );
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, openBlock, transcript, filter]);

  if (!document) return <p className="muted">{t("app.loading")}</p>;

  const quality = model === "sonnet" ? "fast" : "deep";

  return (
    <div className="workspace">
      {/* Deep view: the steps bar owns the top, under the window's title bar. */}
      <div className="stepbar">
        <div className="stepbar__path">
          <button type="button" className="stepbar__back" onClick={onBack}>
            <Icon name="back" size={14} />
            {t("course.back")}
          </button>
          <span className="stepbar__divider" />
          {STEPS.map((s, index) => (
            <span key={s.id} className="stepbar__unit">
              {index > 0 && <span className="stepbar__sep" />}
              <button
                type="button"
                className={`stepbar__step ${step === s.id ? "stepbar__step--current" : ""}`}
                onClick={() => setStep(s.id)}
              >
                <span
                  className={`stepbar__dot ${done[s.id] ? "stepbar__dot--done" : ""} ${step === s.id ? "stepbar__dot--current" : ""}`}
                >
                  {done[s.id] ? <Icon name="check" size={11} /> : index + 1}
                </span>
                {t(s.labelKey)}
              </button>
            </span>
          ))}
        </div>

        <div className="stepbar__tools">
          {step === "pages" && pagePaths.length > 0 && (
            <button type="button" className="btn btn--primary btn--sm" onClick={read}>
              {tn("pages.start", pagePaths.length)}
            </button>
          )}
          {step === "review" && (
            <button
              type="button"
              className="btn btn--primary btn--sm"
              onClick={() => setStep("export")}
            >
              {t("review.makePdf")}
            </button>
          )}
          <OverflowMenu
            label={t("courses.menu.label")}
            entries={[
              { label: t("courses.menu.rename"), icon: "marker", onPick: rename },
              {
                label: t("courses.menu.trash"),
                icon: "trash",
                danger: true,
                onPick: remove,
              },
            ]}
          />
        </div>
      </div>

      {advanced && step !== "review" && (
        <div className="statcards">
          <div className="statcard">
            <span className="statcard__label">{t("advanced.model")}</span>
            <select
              className="statcard__select"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              disabled={running}
            >
              {MODELS.map((m) => (
                <option key={m.id} value={m.id}>
                  {t(m.labelKey)}
                </option>
              ))}
            </select>
            <span className="statcard__meta">{t("advanced.model.change")}</span>
          </div>
          <div className="statcard">
            <span className="statcard__label">{t("advanced.cost")}</span>
            <span className="statcard__value statcard__value--mono">
              {formatMoney(document.costUsd ?? 0)}
            </span>
            <span className="statcard__meta">
              {tn("advanced.cost.meta", blocks.length, {
                pages: tn("common.pages", pagePaths.length),
              })}
            </span>
          </div>
          <div className="statcard">
            <span className="statcard__label">{t("advanced.parallel")}</span>
            <span className="statcard__value">
              {concurrentPages === 0 ? `3 ${t("advanced.parallel.auto")}` : concurrentPages}
            </span>
            <span className="statcard__meta">{t("settings.parallel.label")}</span>
          </div>
          <div className="statcard">
            <span className="statcard__label">{t("advanced.doubt")}</span>
            <span className="statcard__value statcard__value--mono">
              {DOUBT_THRESHOLD.toLocaleString("fr-FR")}
            </span>
            <span className="statcard__meta">
              {tn("advanced.doubt.meta", doubtful.length)}
            </span>
          </div>
        </div>
      )}

      <div className={`workspace__body ${step === "review" ? "workspace__body--flush" : ""}`}>
        {error && (
          <p className="notice notice--error" role="alert">
            {error}
          </p>
        )}

        {step === "pages" && (
          <div className="pagestep">
            <section className="stack stack--tight">
              <div className="section-head">
                <h2 className="section-title--plain">
                  {tn("pages.title", pagePaths.length)}
                </h2>
                <button type="button" className="btn btn--outline btn--sm" onClick={pickAndAdd}>
                  {t("pages.add")}
                </button>
              </div>
              <div className="thumbs">
                {pagePaths.map((path, index) => (
                  <figure key={path} className="thumb">
                    <img src={convertFileSrc(path)} alt={t("pages.page", { number: index + 1 })} loading="lazy" />
                    <button
                      type="button"
                      className="thumb__remove"
                      onClick={() => dropPage(index + 1)}
                      aria-label={t("pages.remove.aria", { number: index + 1 })}
                    >
                      <Icon name="close" size={13} />
                    </button>
                    <figcaption className="thumb__foot">
                      {t("pages.page", { number: index + 1 })}
                    </figcaption>
                  </figure>
                ))}
              </div>
            </section>

            <aside className="pagestep__side">
              <div className="panelcard">
                <div className="panelcard__lead">
                  <span className="panelcard__title">{t("pages.rules.title")}</span>
                  <span className="panelcard__hint">{t("pages.rules.hint")}</span>
                </div>
                <textarea
                  className="input"
                  rows={5}
                  value={rules}
                  onChange={(e) => setRules(e.target.value)}
                  placeholder={t("pages.rules.placeholder")}
                />
                <span className="overline">{t("pages.rules.examples")}</span>
                <div className="chips">
                  {(["example1", "example2", "example3"] as const).map((example) => (
                    <button
                      key={example}
                      type="button"
                      className="chip"
                      onClick={() =>
                        setRules((current) =>
                          current.trim()
                            ? `${current.trim()}\n${t(`pages.rules.${example}.text`)}`
                            : t(`pages.rules.${example}.text`),
                        )
                      }
                    >
                      {t(`pages.rules.${example}`)}
                    </button>
                  ))}
                </div>
                <button
                  type="button"
                  className="btn btn--outline btn--sm"
                  onClick={() =>
                    setReadingRules(documentId, rules)
                      .then(refresh)
                      .catch((cause) => setError(String(cause)))
                  }
                  disabled={rules === (document.readingRules ?? "")}
                >
                  {t("pages.rules.save")}
                </button>
                <span className="panelcard__hint">
                  {t("pages.rules.global", { annotations: t("nav.annotations") })}
                </span>
              </div>
            </aside>
          </div>
        )}

        {step === "read" && (
          <div className="readstep">
            <div className="readstep__main">
              <div className="panelcard">
                <div className="panelcard__row">
                  <div className="panelcard__lead">
                    <span className="panelcard__title panelcard__title--lg">
                      {running ? t("read.running.title") : t("read.idle.title")}
                    </span>
                    <span className="panelcard__hint">
                      {progress && progress.phase === "page"
                        ? advanced
                          ? t("read.pageProgress", {
                              page: progress.page,
                              total: progress.total,
                              cost: formatMoney(progress.costUsd),
                            })
                          : t("read.progress", {
                              done: progress.page,
                              total: progress.total,
                            })
                        : tn("common.pages", pagePaths.length)}
                    </span>
                  </div>
                  {running ? (
                    <button type="button" className="btn btn--outline btn--sm" onClick={stopReading}>
                      {t("read.stop")}
                    </button>
                  ) : (
                    <button
                      type="button"
                      className="btn btn--primary"
                      onClick={read}
                      disabled={pagePaths.length === 0}
                    >
                      {blocks.length ? t("read.restart") : t("read.start")}
                    </button>
                  )}
                </div>
                <div className={`bigmeter ${running ? "bigmeter--live" : ""}`}>
                  <Meter
                    share={
                      progress && progress.total > 0 ? progress.page / progress.total : 0
                    }
                    tone="accent"
                  />
                </div>
                <p className="panelcard__hint">{t("read.reassurance")}</p>
              </div>

              {progress && progress.phase !== "page" && (
                <p className={`notice ${progress.phase === "failed" ? "notice--error" : ""}`}>
                  {progress.phase === "failed"
                    ? t("read.failedPage", {
                        page: progress.page,
                        message: progress.message ?? "",
                      })
                    : progress.phase === "cancelled"
                      ? t("read.cancelled", {
                          blocks: progress.blocks,
                          cost: formatMoney(progress.costUsd),
                        })
                      : t("read.done", {
                          blocks: progress.blocks,
                          cost: formatMoney(progress.costUsd),
                        })}
                </p>
              )}

              <div className="listcard">
                <header className="listcard__head listcard__head--bar">
                  {t("read.list.title")}
                </header>
                {pagePaths.map((path, index) => {
                  const number = index + 1;
                  const info = scan[number];
                  const already = transcript?.pages.find((p) => p.number === number);
                  const state = info?.state ?? (already ? "done" : "waiting");
                  const found = info?.blocks || already?.blocks.length || 0;
                  return (
                    <div key={path} className={`scanrow scanrow--${state}`}>
                      <img className="scanrow__thumb" src={convertFileSrc(path)} alt="" />
                      <div className="scanrow__body">
                        <span className="scanrow__title">
                          {t("pages.page", { number })}
                        </span>
                        <span className="scanrow__label">
                          {state === "reading"
                            ? (info?.label ?? t("read.state.reading"))
                            : state === "done"
                              ? tn("read.state.done", found)
                              : state === "failed"
                                ? (info?.message ?? t("read.state.failed"))
                                : state === "cancelled"
                                  ? t("read.state.cancelled")
                                  : t("read.state.waiting")}
                        </span>
                      </div>
                      <span className={`scanrow__dot scanrow__dot--${state}`} aria-hidden="true">
                        {state === "done" && <Icon name="check" size={11} />}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>

            <aside className="readstep__side">
              {!advanced ? (
                <>
                  <div className="panelcard">
                    <span className="panelcard__title panelcard__title--sm">
                      {t("read.quality.title")}
                    </span>
                    <div className="radios">
                      {(
                        [
                          ["fast", "sonnet", t("read.quality.fast"), t("read.quality.fast.hint")],
                          ["deep", "opus", t("read.quality.deep"), t("read.quality.deep.hint")],
                        ] as const
                      ).map(([id, target, label, hint]) => (
                        <button
                          key={id}
                          type="button"
                          className={`radio ${quality === id ? "radio--on" : ""}`}
                          onClick={() => setModel(target)}
                          disabled={running}
                        >
                          <span className="radio__mark" />
                          <span className="radio__copy">
                            <span className="radio__label">{label}</span>
                            <span className="radio__hint">{hint}</span>
                          </span>
                        </button>
                      ))}
                    </div>
                  </div>
                  <div className="asidecard">
                    <span className="overline">{t("read.advanced.title")}</span>
                    <p className="asidecard__text">
                      {t("read.advanced.hint", { mode: t("titlebar.mode.advanced") })}
                    </p>
                  </div>
                </>
              ) : (
                <div className="panelcard">
                  <label className="field">
                    <span className="field__label">{t("read.model.label")}</span>
                    <select
                      className="input"
                      value={model}
                      onChange={(e) => setModel(e.target.value)}
                      disabled={running}
                    >
                      {MODELS.map((m) => (
                        <option key={m.id} value={m.id}>
                          {t(m.labelKey)}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
              )}
            </aside>
          </div>
        )}

        {step === "review" && (
          <section className="review">
            <div className="review__bar">
              <div className="chips">
                {FILTERS.map((entry) => (
                  <button
                    key={entry.id}
                    type="button"
                    className={`chip ${entry.id === "doubt" && entry.count > 0 ? "chip--warn" : ""} ${filter === entry.id ? "chip--on" : ""}`}
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
                  <button type="button" className="btn btn--outline btn--sm" onClick={stopCorrecting}>
                    {t("review.correct.stop")}
                  </button>
                )}
                {annotated.length > 0 && !running && (
                  <button type="button" className="btn btn--outline btn--sm" onClick={correct}>
                    {tn("review.correct", annotated.length)}
                  </button>
                )}
              </div>
            </div>

            {correcting && (
              <p className={`notice ${correcting.phase === "failed" ? "notice--error" : ""}`}>
                {correcting.phase === "done"
                  ? t("review.correct.done", { done: correcting.done, total: correcting.total })
                  : correcting.phase === "cancelled"
                    ? tn("review.correct.cancelled", correcting.done, {
                        done: correcting.done,
                        total: correcting.total,
                      })
                    : correcting.phase === "failed"
                      ? t("review.correct.failed", {
                          block: correcting.blockId,
                          message: correcting.message ?? "",
                        })
                      : t("review.correct.progress", {
                          done: correcting.done,
                          total: correcting.total,
                        })}
              </p>
            )}

            {blocks.length === 0 ? (
              <p className="muted">{t("review.unread")}</p>
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
                    position={selectedAt + 1}
                    total={all.length}
                    pageSrc={pagePaths[selected.page - 1]}
                    onClose={() => setOpenBlock(null)}
                    onPrev={() => stepTo(-1)}
                    onNext={() => stepTo(1)}
                    onSave={persist}
                    onNote={(note) => annotate(selected.block.id, note)}
                  />
                )}
              </div>
            )}
          </section>
        )}

        {step === "export" && (
          <div className="exportstep">
            <div className="exportstep__side">
              <div className="panelcard">
                <span className="panelcard__title">{t("export.choice.title")}</span>
                <div className="radios">
                  {(
                    [
                      ["teacher", t("export.teacher.title")],
                      ["student", t("export.student.title")],
                      ["all", t("export.all.title")],
                    ] as const
                  ).map(([id, label]) => {
                    const kept = id === "all" ? blocks.length : keptFor(id);
                    const removed = blocks.length - kept;
                    const hint =
                      id === "all"
                        ? t("export.all.hint")
                        : id === "teacher"
                          ? tn("export.teacher.hint", kept)
                          : removed > 0
                            ? tn("export.student.hint", kept, { removed })
                            : t("export.student.hint.same");
                    return (
                      <button
                        key={id}
                        type="button"
                        className={`radio ${audience === id ? "radio--on" : ""}`}
                        onClick={() => setAudience(id)}
                      >
                        <span className="radio__mark" />
                        <span className="radio__copy">
                          <span className="radio__label">{label}</span>
                          <span className="radio__hint">{hint}</span>
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>

              {teacherOnly.length === 0 && studentOnly.length === 0 && (
                <p className="field__hint">{t("export.sameVersions")}</p>
              )}

              <button
                type="button"
                className="btn btn--primary btn--tall"
                onClick={compile}
                disabled={blocks.length === 0 || building}
              >
                {building ? t("export.building") : t("export.build")}
              </button>

              {build && !build.error && (
                <div className="donecard">
                  <div className="donecard__head">
                    <span className="donecard__badge">
                      <Icon name="check" size={12} />
                    </span>
                    <span className="donecard__title">{t("export.ready")}</span>
                  </div>
                  <div className="donecard__actions">
                    {build.pdfPath && (
                      <button
                        type="button"
                        className="btn btn--soft btn--sm"
                        onClick={() =>
                          revealPath(build.pdfPath!).catch((cause) =>
                            logError("interface", t("error.refresh"), cause),
                          )
                        }
                      >
                        {t("export.openPdf")}
                      </button>
                    )}
                    <button
                      type="button"
                      className="btn btn--soft btn--sm"
                      onClick={() =>
                        revealPath(build.texPath).catch((cause) =>
                          logError("interface", t("error.refresh"), cause),
                        )
                      }
                    >
                      {t("export.reveal")}
                    </button>
                  </div>
                </div>
              )}

              {build?.error && (
                <p className="notice notice--error" role="alert">
                  {t("export.failed", { error: build.error })}
                </p>
              )}

              <AdvancedRow
                text={t("export.texRow")}
                open={texOpen}
                onToggle={() => setTexOpen((current) => !current)}
              >
                <div className="adv__body">
                  {build ? (
                    <button
                      type="button"
                      className="btn btn--outline btn--sm"
                      onClick={() =>
                        revealPath(build.texPath).catch((cause) =>
                          logError("interface", t("error.refresh"), cause),
                        )
                      }
                    >
                      {t("export.openTex")}
                    </button>
                  ) : (
                    <span className="field__hint">{t("export.noTex")}</span>
                  )}
                </div>
              </AdvancedRow>
            </div>

            <div className="exportstep__preview">
              {build?.pdfPath ? (
                <iframe
                  className="pdf-preview"
                  src={convertFileSrc(build.pdfPath)}
                  title={t("export.preview.title", {
                    audience:
                      audience === "teacher"
                        ? t("export.teacher.title")
                        : audience === "student"
                          ? t("export.student.title")
                          : t("export.all.title"),
                  })}
                />
              ) : (
                <div className="exportstep__empty">
                  <Icon name="pdf" size={28} />
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
