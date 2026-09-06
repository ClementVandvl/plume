import { useEffect, useMemo, useState } from "react";
import { deleteDocument, openDocumentPdf, renameDocument, setTags } from "../api";
import { useAdvanced } from "../ui/mode";
import { TagEditor } from "./TagEditor";
import { useConfirm } from "../confirm";
import { formatRelative, t, tn } from "../i18n";
import { logError } from "../log";
import type { DocumentStatus, DocumentSummary, Route, StepId } from "../types";
import { Icon } from "../ui/Icon";
import { Meter, OverflowMenu, PageSkeleton, ReadingPill } from "../ui/controls";

type Props = {
  documents: DocumentSummary[];
  /** Documents being read right now. */
  reading: Set<string>;
  onCreate: () => void;
  /** A document written elsewhere — an exercise sheet asked of Claude. */
  onImport: () => void;
  onNavigate: (route: Route) => void;
  onChanged: () => void;
};

/** What one row should say, and the single button that answers it. */
function nextStep(doc: DocumentSummary): {
  phrase: string;
  action: string;
  step: StepId;
  pdf?: boolean;
} {
  if (doc.doubtfulCount > 0)
    return {
      phrase: tn("documents.state.doubtful", doc.doubtfulCount),
      action: t("documents.action.review"),
      step: "review",
    };
  if (doc.status === "ready")
    return {
      phrase: t("documents.state.ready"),
      action: t("documents.action.openPdf"),
      step: "export",
      pdf: true,
    };
  if (doc.blockCount > 0)
    return {
      phrase: t("documents.state.reviewed"),
      action: t("review.makePdf"),
      step: "export",
    };
  if (doc.pageCount > 0)
    return {
      phrase: t("documents.state.unread"),
      action: t("documents.action.read"),
      step: "read",
    };
  return {
    phrase: t("documents.state.empty"),
    action: t("documents.action.addPages"),
    step: "pages",
  };
}

export function DocumentsView({
  documents,
  reading,
  onCreate,
  onImport,
  onNavigate,
  onChanged,
}: Props) {
  const advanced = useAdvanced();
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<DocumentStatus | null>(null);
  const [tag, setTag] = useState<string | null>(null);
  const [tagging, setTagging] = useState<DocumentSummary | null>(null);
  const [savingTags, setSavingTags] = useState(false);
  const { confirm, promptFor } = useConfirm();

  const carries = (d: DocumentSummary, wanted: string) =>
    d.tags.some((mine) => mine.toLowerCase() === wanted.toLowerCase());

  // Every tag in use, most used first: the shelf reached for most sits first.
  const tags = useMemo(() => {
    const counts = new Map<string, { tag: string; count: number }>();
    for (const d of documents) {
      for (const mine of d.tags) {
        const key = mine.toLowerCase();
        const entry = counts.get(key) ?? { tag: mine, count: 0 };
        entry.count += 1;
        counts.set(key, entry);
      }
    }
    return [...counts.values()].sort(
      (a, b) => b.count - a.count || a.tag.localeCompare(b.tag, "fr"),
    );
  }, [documents]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return documents
      .filter((d) => !status || d.status === status)
      .filter((d) => !tag || carries(d, tag))
      .filter((d) => !needle || d.title.toLowerCase().includes(needle))
      .sort((a, b) => b.updatedAt - a.updatedAt);
  }, [documents, query, status, tag]);

  async function saveTags(doc: DocumentSummary, next: string[]) {
    setSavingTags(true);
    try {
      await setTags(doc.id, next);
      setTagging(null);
      onChanged();
    } catch (cause) {
      logError("workspace", t("error.refresh"), cause);
    } finally {
      setSavingTags(false);
    }
  }

  // Counted within the shelf being looked at: « À vérifier 2 » among the DS
  // is the number that answers the question the teacher is asking.
  const countFor = (wanted: DocumentStatus) =>
    documents.filter((d) => (!tag || carries(d, tag)) && d.status === wanted).length;

  // Changing shelf can empty the selected state; a lit, disabled segment over
  // an empty list is not an answer.
  useEffect(() => {
    if (status && countFor(status) === 0) setStatus(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tag, documents]);

  // The one document the teacher is most likely here for: the most recently
  // touched one with doubts left. Its button is the filled one.
  const urgent = visible.find((d) => d.doubtfulCount > 0);

  async function rename(doc: DocumentSummary) {
    const title = await promptFor({
      title: t("document.rename.title"),
      message: t("document.rename.message"),
      confirmLabel: t("document.rename.confirm"),
      input: {
        label: t("document.rename.field"),
        value: doc.title,
        placeholder: t("wizard.title.placeholder"),
      },
    });
    if (!title || !title.trim()) return;
    try {
      await renameDocument(doc.id, title);
      onChanged();
    } catch (cause) {
      logError("workspace", t("error.refresh"), cause);
    }
  }

  async function trash(doc: DocumentSummary) {
    const ok = await confirm({
      title: t("document.trash.title", { title: doc.title }),
      message: t("document.trash.message"),
      confirmLabel: t("document.trash.confirm"),
      tone: "danger",
    });
    if (!ok) return;
    try {
      await deleteDocument(doc.id);
      onChanged();
    } catch (cause) {
      logError("workspace", t("error.refresh"), cause);
    }
  }

  function act(doc: DocumentSummary) {
    const next = nextStep(doc);
    if (next.pdf) {
      openDocumentPdf(doc.id).catch(() =>
        onNavigate({ name: "document", id: doc.id, step: "export" }),
      );
      return;
    }
    onNavigate({ name: "document", id: doc.id, step: next.step });
  }

  return (
    <div className="stack">
      <header className="page-head">
        <h1 className="page-title">{t("documents.title")}</h1>
        <div className="page-head__tools">
          <label className="search">
            <Icon name="search" size={15} />
            <input
              className="search__input"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("documents.search")}
              type="search"
            />
          </label>
          {/* Importing means asking a model for JSON and bringing it back:
              a step that presumes some ease with Claude, so it waits behind
              the advanced mode rather than sit beside "Nouveau cours". */}
          {advanced && (
            <button type="button" className="btn btn--outline" onClick={onImport}>
              {t("documents.import")}
            </button>
          )}
          <button type="button" className="btn btn--primary" onClick={onCreate}>
            {t("documents.new")}
          </button>
        </div>
      </header>

      {/* One bar for two questions, each with its own control: what a document
          is — the teacher's own shelves, as tabs — and where it stands, as a
          switch. Two rows of the same pills read as one question asked twice,
          with « Tous » lit on both. */}
      <div className="filterbar">
        {tags.length > 0 ? (
          <div className="tabs tabs--inline" role="tablist" aria-label={t("documents.tags.label")}>
            <button
              type="button"
              role="tab"
              aria-selected={tag === null}
              className={`tab ${tag === null ? "tab--on" : ""}`}
              onClick={() => setTag(null)}
            >
              {t("documents.filter.all")}
              <span className="tab__count">{documents.length}</span>
            </button>
            {tags.map((entry) => {
              const on = tag?.toLowerCase() === entry.tag.toLowerCase();
              return (
                <button
                  key={entry.tag}
                  type="button"
                  role="tab"
                  aria-selected={on}
                  className={`tab ${on ? "tab--on" : ""}`}
                  onClick={() => setTag(on ? null : entry.tag)}
                >
                  {entry.tag}
                  <span className="tab__count">{entry.count}</span>
                </button>
              );
            })}
          </div>
        ) : (
          <span />
        )}

        {/* No « Tous » here: nothing lit means no filter, and pressing the lit
            one again clears it. A neutral segment would be a second « Tous »
            on the same line as the tabs' one. */}
        <div className="seg" role="group" aria-label={t("documents.column.state")}>
          {(["review", "ready", "draft"] as const).map((id) => (
            <button
              key={id}
              type="button"
              className={`seg__opt ${status === id ? "seg__opt--on" : ""}`}
              onClick={() => setStatus(status === id ? null : id)}
              disabled={countFor(id) === 0}
              aria-pressed={status === id}
            >
              {t(`status.${id}`)}
              <span className="seg__count">{countFor(id)}</span>
            </button>
          ))}
        </div>
      </div>

      {visible.length === 0 ? (
        <p className="muted">
          {documents.length === 0 ? t("documents.empty.none") : t("documents.empty.filtered")}
        </p>
      ) : (
        <div className="ctable">
          <div className="ctable__head">
            <span />
            <span>{t("documents.column.document")}</span>
            <span>{t("documents.column.state")}</span>
            <span>{t("documents.column.next")}</span>
            <span />
          </div>

          {visible.map((doc) => {
            const next = nextStep(doc);
            const reviewed = doc.blockCount - doc.doubtfulCount;
            const share =
              doc.status === "ready"
                ? 1
                : doc.blockCount > 0
                  ? reviewed / doc.blockCount
                  : doc.pageCount > 0
                    ? 0.08
                    : 0;
            const tone =
              doc.doubtfulCount > 0
                ? "warn"
                : doc.status === "ready" || doc.blockCount > 0
                  ? "ok"
                  : "muted";
            return (
              <div
                key={doc.id}
                className={`ctable__row ${doc.id === urgent?.id ? "ctable__row--urgent" : ""}`}
                onClick={() => onNavigate({ name: "document", id: doc.id })}
                role="button"
                tabIndex={0}
                onKeyDown={(e) =>
                  e.key === "Enter" && onNavigate({ name: "document", id: doc.id })
                }
              >
                <PageSkeleton size="sm" />
                <div className="ctable__identity">
                  <span className="ctable__title">{doc.title}</span>
                  <span className="ctable__meta">
                    {doc.tags.length > 0 && (
                      <span className="ctable__tags">
                        {doc.tags.map((mine) => (
                          <span key={mine} className="tagpill">
                            {mine}
                          </span>
                        ))}
                      </span>
                    )}
                    {tn("common.pages", doc.pageCount)} ·{" "}
                    {t("common.modified", { when: formatRelative(doc.updatedAt) })}
                  </span>
                  {/* Where the class got to — the question a Sunday evening
                      asks of a document being taught over several weeks. The
                      heading when there is one, since that is how a teacher
                      names the place; a count otherwise. */}
                  {doc.taughtCount != null && (
                    <span className="ctable__taught">
                      {doc.taughtHeading
                        ? t("taught.card.heading", { heading: doc.taughtHeading })
                        : tn("taught.card.count", doc.taughtCount)}
                    </span>
                  )}
                </div>
                <div className="ctable__state">
                  {reading.has(doc.id) ? (
                    <ReadingPill />
                  ) : (
                    <>
                      <span
                        className={`ctable__phrase ${doc.status === "ready" && doc.doubtfulCount === 0 ? "ctable__phrase--ok" : ""}`}
                      >
                        {next.phrase}
                      </span>
                      <Meter share={share} tone={tone} />
                    </>
                  )}
                </div>
                <button
                  type="button"
                  className={`btn ${doc.id === urgent?.id ? "btn--primary" : "btn--outline"} btn--sm`}
                  onClick={(e) => {
                    e.stopPropagation();
                    // Offering to read a document already being read would start
                    // a second reading over the first, and bill for it. Going
                    // to watch it is the only useful thing left to do.
                    if (reading.has(doc.id)) {
                      onNavigate({ name: "document", id: doc.id });
                      return;
                    }
                    act(doc);
                  }}
                >
                  {reading.has(doc.id)
                    ? t("documents.action.watch")
                    : doc.id === urgent?.id || doc.doubtfulCount === 0
                      ? next.action
                      : t("documents.action.reread")}
                </button>
                <div onClick={(e) => e.stopPropagation()}>
                  <OverflowMenu
                    label={t("documents.menu.label")}
                    entries={[
                      {
                        label: t("documents.menu.open"),
                        icon: "book",
                        onPick: () => onNavigate({ name: "document", id: doc.id }),
                      },
                      {
                        label: t("documents.menu.rename"),
                        icon: "marker",
                        onPick: () => rename(doc),
                      },
                      {
                        label: t("documents.menu.tags"),
                        icon: "folder",
                        onPick: () => setTagging(doc),
                      },
                      {
                        label: t("documents.menu.trash"),
                        icon: "trash",
                        danger: true,
                        onPick: () => trash(doc),
                      },
                    ]}
                  />
                </div>
              </div>
            );
          })}
        </div>
      )}

      {tagging && (
        <TagEditor
          title={tagging.title}
          tags={tagging.tags}
          known={tags.map((entry) => entry.tag)}
          busy={savingTags}
          onCancel={() => setTagging(null)}
          onSave={(next) => saveTags(tagging, next)}
        />
      )}
    </div>
  );
}
