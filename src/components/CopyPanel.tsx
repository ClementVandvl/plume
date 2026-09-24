import { useEffect, useMemo, useState } from "react";
import { listDocuments, readingDocuments } from "../api";
import { t, tn } from "../i18n";
import { latexToHtml } from "../preview/latexToHtml";
import { KIND_LABEL, type Block, type DocumentSummary } from "../types";
import { Modal } from "./Modal";

/**
 * Copying one passage into another document.
 *
 * Two destinations, because a passage is reused for one of two reasons: it
 * belongs on a sheet that already exists — this week's exercises, a revision
 * sheet — or it is the start of something new, a sheet of its own that nothing
 * has been photographed for.
 *
 * Either way the copy arrives at the very end, and this document keeps its
 * passage: it still belongs to the lesson it was written in.
 */

type Mode = "existing" | "new";

export function CopyPanel({
  block,
  documentId,
  busy,
  onClose,
  onCopy,
}: {
  block: Block;
  /** The document the passage is in: never offered as a destination. */
  documentId: string;
  busy: boolean;
  onClose: () => void;
  /** `target` names an existing document; `null` creates one called `title`. */
  onCopy: (target: string | null, title: string | null) => Promise<void>;
}) {
  const [mode, setMode] = useState<Mode>("existing");
  const [documents, setDocuments] = useState<DocumentSummary[] | null>(null);
  const [reading, setReading] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [target, setTarget] = useState<string | null>(null);
  // A heading copied out is usually what the new sheet is about.
  const [title, setTitle] = useState(block.title?.trim() ?? "");

  useEffect(() => {
    listDocuments()
      .then(setDocuments)
      .catch(() => setDocuments([]));
    readingDocuments()
      .then(setReading)
      .catch(() => setReading([]));
  }, []);

  /**
   * Where the passage can go. Not a document still waiting for its reading:
   * the passages it holds arrive with it, laid out around this one as if it
   * had been on the page.
   */
  const destinations = useMemo(() => {
    const words = query.trim().toLocaleLowerCase("fr");
    return (documents ?? [])
      .filter((d) => d.id !== documentId)
      .filter((d) => d.origin === "written" || d.blockCount > 0)
      .filter(
        (d) =>
          !words ||
          d.title.toLocaleLowerCase("fr").includes(words) ||
          d.tags.some((tag) => tag.toLocaleLowerCase("fr").includes(words)),
      )
      .sort((a, b) => b.updatedAt - a.updatedAt);
  }, [documents, documentId, query]);

  const heading = block.title?.trim();

  return (
    <Modal title={t("copy.title")} subtitle={t("copy.subtitle")} onClose={onClose}>
      {/* The passage as it reads, so the teacher knows what is being copied. */}
      <div className="copy__passage">
        <strong>
          {KIND_LABEL[block.kind] ?? block.kind}
          {heading ? ` — ${heading}` : ""}
        </strong>
        {block.latex.trim() && (
          <div dangerouslySetInnerHTML={{ __html: latexToHtml(block.latex) }} />
        )}
      </div>

      <div className="tabs" role="tablist">
        {(
          [
            ["existing", t("copy.tab.existing")],
            ["new", t("copy.tab.new")],
          ] as [Mode, string][]
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={mode === id}
            className={`tab ${mode === id ? "tab--on" : ""}`}
            onClick={() => setMode(id)}
          >
            {label}
          </button>
        ))}
      </div>

      {mode === "existing" ? (
        <section className="stack stack--tight">
          <input
            className="input"
            value={query}
            placeholder={t("copy.search")}
            onChange={(event) => setQuery(event.target.value)}
            autoFocus
          />

          {documents === null ? (
            <p className="field__hint">{t("copy.loading")}</p>
          ) : destinations.length === 0 ? (
            <p className="field__hint">
              {query.trim() ? t("copy.none.match") : t("copy.none")}
            </p>
          ) : (
            <div className="radios copy__list">
              {destinations.map((d) => {
                const busyThere = reading.includes(d.id);
                return (
                  <button
                    key={d.id}
                    type="button"
                    className={`radio ${target === d.id ? "radio--on" : ""}`}
                    onClick={() => setTarget(d.id)}
                    disabled={busyThere}
                  >
                    <span className="radio__mark" />
                    <span className="radio__copy">
                      <span className="radio__label">{d.title}</span>
                      <span className="radio__hint">
                        {busyThere
                          ? t("copy.reading")
                          : [tn("copy.count", d.blockCount), ...d.tags].join(" · ")}
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
          )}

          <p className="field__hint">{t("copy.existing.hint")}</p>
          <div className="review__actions">
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => target && onCopy(target, null)}
              disabled={busy || !target}
            >
              {busy ? t("copy.copying") : t("copy.existing.action")}
            </button>
          </div>
        </section>
      ) : (
        <section className="stack stack--tight">
          <label className="field">
            <span className="field__label">{t("copy.new.title")}</span>
            <input
              className="input"
              value={title}
              placeholder={t("copy.new.placeholder")}
              onChange={(event) => setTitle(event.target.value)}
              autoFocus
            />
          </label>
          <p className="field__hint">{t("copy.new.hint")}</p>
          <div className="review__actions">
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => onCopy(null, title.trim())}
              disabled={busy || !title.trim()}
            >
              {busy ? t("copy.copying") : t("copy.new.action")}
            </button>
          </div>
        </section>
      )}
    </Modal>
  );
}
