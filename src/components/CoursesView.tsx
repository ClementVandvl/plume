import { useMemo, useState } from "react";
import { deleteDocument, openCoursePdf, renameDocument } from "../api";
import { useConfirm } from "../confirm";
import { formatRelative, t, tn } from "../i18n";
import { logError } from "../log";
import type { DocumentStatus, DocumentSummary, Route, StepId } from "../types";
import { Icon } from "../ui/Icon";
import { Meter, OverflowMenu, PageSkeleton, ReadingPill } from "../ui/controls";

type Props = {
  documents: DocumentSummary[];
  /** Courses being read right now. */
  reading: Set<string>;
  onCreate: () => void;
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
      phrase: tn("courses.state.doubtful", doc.doubtfulCount),
      action: t("courses.action.review"),
      step: "review",
    };
  if (doc.status === "ready")
    return {
      phrase: t("courses.state.ready"),
      action: t("courses.action.openPdf"),
      step: "export",
      pdf: true,
    };
  if (doc.blockCount > 0)
    return {
      phrase: t("courses.state.reviewed"),
      action: t("review.makePdf"),
      step: "export",
    };
  if (doc.pageCount > 0)
    return {
      phrase: t("courses.state.unread"),
      action: t("courses.action.read"),
      step: "read",
    };
  return {
    phrase: t("courses.state.empty"),
    action: t("courses.action.addPages"),
    step: "pages",
  };
}

export function CoursesView({
  documents,
  reading,
  onCreate,
  onNavigate,
  onChanged,
}: Props) {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<DocumentStatus | null>(null);
  const { confirm, promptFor } = useConfirm();

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return documents
      .filter((d) => !status || d.status === status)
      .filter((d) => !needle || d.title.toLowerCase().includes(needle))
      .sort((a, b) => b.updatedAt - a.updatedAt);
  }, [documents, query, status]);

  const countFor = (wanted: DocumentStatus) =>
    documents.filter((d) => d.status === wanted).length;

  // The one course the teacher is most likely here for: the most recently
  // touched one with doubts left. Its button is the filled one.
  const urgent = visible.find((d) => d.doubtfulCount > 0);

  async function rename(doc: DocumentSummary) {
    const title = await promptFor({
      title: t("course.rename.title"),
      message: t("course.rename.message"),
      confirmLabel: t("course.rename.confirm"),
      input: {
        label: t("course.rename.field"),
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
      title: t("course.trash.title", { title: doc.title }),
      message: t("course.trash.message"),
      confirmLabel: t("course.trash.confirm"),
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
      openCoursePdf(doc.id).catch(() =>
        onNavigate({ name: "course", id: doc.id, step: "export" }),
      );
      return;
    }
    onNavigate({ name: "course", id: doc.id, step: next.step });
  }

  return (
    <div className="stack">
      <header className="page-head">
        <h1 className="page-title">{t("courses.title")}</h1>
        <div className="page-head__tools">
          <label className="search">
            <Icon name="search" size={15} />
            <input
              className="search__input"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("courses.search")}
              type="search"
            />
          </label>
          <button type="button" className="btn btn--primary" onClick={onCreate}>
            {t("courses.new")}
          </button>
        </div>
      </header>

      <div className="chips">
        <button
          type="button"
          className={`chip ${status === null ? "chip--on" : ""}`}
          onClick={() => setStatus(null)}
        >
          {t("courses.filter.all")} <span className="chip__count">{documents.length}</span>
        </button>
        {(["review", "ready", "draft"] as const).map((id) => (
          <button
            key={id}
            type="button"
            className={`chip ${status === id ? "chip--on" : ""}`}
            onClick={() => setStatus(status === id ? null : id)}
            disabled={countFor(id) === 0}
          >
            {t(`status.${id}`)} <span className="chip__count">{countFor(id)}</span>
          </button>
        ))}
      </div>

      {visible.length === 0 ? (
        <p className="muted">
          {documents.length === 0 ? t("courses.empty.none") : t("courses.empty.filtered")}
        </p>
      ) : (
        <div className="ctable">
          <div className="ctable__head">
            <span />
            <span>{t("courses.column.course")}</span>
            <span>{t("courses.column.state")}</span>
            <span>{t("courses.column.next")}</span>
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
                onClick={() => onNavigate({ name: "course", id: doc.id })}
                role="button"
                tabIndex={0}
                onKeyDown={(e) =>
                  e.key === "Enter" && onNavigate({ name: "course", id: doc.id })
                }
              >
                <PageSkeleton size="sm" />
                <div className="ctable__identity">
                  <span className="ctable__title">{doc.title}</span>
                  <span className="ctable__meta">
                    {tn("common.pages", doc.pageCount)} ·{" "}
                    {t("common.modified", { when: formatRelative(doc.updatedAt) })}
                  </span>
                  {/* Where the class got to — the question a Sunday evening
                      asks of a course being taught over several weeks. The
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
                    // Offering to read a course already being read would start
                    // a second reading over the first, and bill for it. Going
                    // to watch it is the only useful thing left to do.
                    if (reading.has(doc.id)) {
                      onNavigate({ name: "course", id: doc.id });
                      return;
                    }
                    act(doc);
                  }}
                >
                  {reading.has(doc.id)
                    ? t("courses.action.watch")
                    : doc.id === urgent?.id || doc.doubtfulCount === 0
                      ? next.action
                      : t("courses.action.reread")}
                </button>
                <div onClick={(e) => e.stopPropagation()}>
                  <OverflowMenu
                    label={t("courses.menu.label")}
                    entries={[
                      {
                        label: t("courses.menu.open"),
                        icon: "book",
                        onPick: () => onNavigate({ name: "course", id: doc.id }),
                      },
                      {
                        label: t("courses.menu.rename"),
                        icon: "marker",
                        onPick: () => rename(doc),
                      },
                      {
                        label: t("courses.menu.trash"),
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
    </div>
  );
}
