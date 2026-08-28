import { useEffect, useState, type ReactNode } from "react";
import { formatRelative, t, tn } from "../i18n";
import { isTauri } from "../platform";
import type { DocumentSummary, Environment, Route } from "../types";
import { Icon } from "../ui/Icon";
import { Meter, PageSkeleton, StatusPill } from "../ui/controls";

const IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "heic", "heif", "webp", "tif", "tiff"];
const isImage = (path: string) =>
  IMAGE_EXTENSIONS.includes(path.split(".").pop()?.toLowerCase() ?? "");

type Props = {
  documents: DocumentSummary[];
  environment: Environment | null;
  /** Opens the wizard, optionally pre-filled with dropped photos. */
  onCreate: (pages?: string[]) => void;
  onNavigate: (route: Route) => void;
  onSettings: () => void;
};

/** `{placeholder}` in a message, rendered bold — for the one emphasised bit. */
function withStrong(message: string, emphasis: string): ReactNode {
  const at = message.indexOf(emphasis);
  if (at < 0) return message;
  return (
    <>
      {message.slice(0, at)}
      <strong>{emphasis}</strong>
      {message.slice(at + emphasis.length)}
    </>
  );
}

export function HomeView({ documents, environment, onCreate, onNavigate, onSettings }: Props) {
  const [dragging, setDragging] = useState(false);

  // Dropping photos anywhere on this screen starts a course with them.
  useEffect(() => {
    if (!isTauri) return;
    let stop: (() => void) | null = null;
    import("@tauri-apps/api/webview")
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((event) => {
          if (event.payload.type === "over") setDragging(true);
          else if (event.payload.type === "drop") {
            setDragging(false);
            const images = event.payload.paths.filter(isImage);
            if (images.length > 0) onCreate(images);
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

  const missingEngine = environment?.tools.some((tool) => !tool.found) ?? false;

  if (documents.length === 0) {
    return (
      <Welcome
        environment={environment}
        dragging={dragging}
        onCreate={onCreate}
        onSettings={onSettings}
      />
    );
  }

  const resume =
    documents.find((d) => d.doubtfulCount > 0) ??
    documents.find((d) => d.status === "review");
  const others = documents.filter((d) => d.id !== resume?.id).slice(0, 3);
  const reviewed = resume ? resume.blockCount - resume.doubtfulCount : 0;

  return (
    <div className="stack">
      <header className="page-head">
        <div>
          <h1 className="page-title">{t("home.greeting")}</h1>
          <p className="page-subtitle">
            {resume
              ? t("home.subtitle.review")
              : documents.some((d) => d.status === "draft")
                ? t("home.subtitle.reading")
                : t("home.subtitle.idle")}
          </p>
        </div>
      </header>

      {resume && (
        <section className="resume">
          <PageSkeleton size="lg" />
          <div className="resume__body">
            <div className="resume__identity">
              <span className="overline">{t("home.resume.overline")}</span>
              <span className="resume__title">{resume.title}</span>
            </div>
            <p className="resume__text">
              {withStrong(
                t("home.resume.summary", {
                  pages: resume.pageCount,
                  remaining: tn("home.resume.left", resume.doubtfulCount),
                  blocks: resume.blockCount,
                }),
                tn("home.resume.left", resume.doubtfulCount),
              )}
            </p>
            <div className="resume__meter">
              <Meter
                share={resume.blockCount ? reviewed / resume.blockCount : 0}
                tone="ok"
              />
              <span className="resume__count">
                {t("home.resume.progress", { reviewed, blocks: resume.blockCount })}
              </span>
            </div>
          </div>
          <div className="resume__actions">
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => onNavigate({ name: "course", id: resume.id, step: "review" })}
            >
              {t("home.resume.continue")}
            </button>
            <button
              type="button"
              className="btn btn--ghost"
              onClick={() => onNavigate({ name: "course", id: resume.id })}
            >
              {t("home.resume.open")}
            </button>
          </div>
        </section>
      )}

      {missingEngine && (
        <div className="banner banner--warn">
          <Icon name="warning" />
          <span className="banner__text">{t("home.engine.banner")}</span>
          <button type="button" className="btn btn--soft" onClick={onSettings}>
            {t("home.engine.install")}
          </button>
        </div>
      )}

      <div className="home-grid">
        <button
          type="button"
          className={`dropzone ${dragging ? "dropzone--active" : ""}`}
          onClick={() => onCreate()}
        >
          <Icon name="upload" size={26} />
          <span className="dropzone__title">{t("home.drop.title")}</span>
          <span className="dropzone__text">
            {withStrong(
              t("home.drop.text", { browse: t("home.drop.browse") }),
              t("home.drop.browse"),
            )}
          </span>
        </button>

        <section className="howto">
          <span className="overline">{t("home.howto.title")}</span>
          <ol className="howto__list">
            {(["step1", "step2", "step3", "step4"] as const).map((step, index) => (
              <li key={step} className="howto__step">
                <span className="howto__number">{index + 1}</span>
                {t(`home.howto.${step}`)}
              </li>
            ))}
          </ol>
        </section>
      </div>

      {others.length > 0 && (
        <section className="stack stack--tight">
          <div className="section-head">
            <h2 className="section-title--plain">{t("home.others.title")}</h2>
            <button
              type="button"
              className="btn btn--link"
              onClick={() => onNavigate({ name: "courses" })}
            >
              {t("home.others.all")}
            </button>
          </div>
          <div className="cards">
            {others.map((doc) => (
              <CourseCard
                key={doc.id}
                document={doc}
                onOpen={() => onNavigate({ name: "course", id: doc.id })}
              />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

/** One course tile: title, status pill, a line of facts, its progress. */
function CourseCard({
  document,
  onOpen,
}: {
  document: DocumentSummary;
  onOpen: () => void;
}) {
  const reviewed = document.blockCount - document.doubtfulCount;
  const share = document.blockCount
    ? reviewed / document.blockCount
    : document.status === "draft"
      ? 0.06
      : 0;
  const tone =
    document.status === "ready"
      ? "ok"
      : document.doubtfulCount > 0
        ? "warn"
        : document.blockCount === 0
          ? "muted"
          : "ok";

  const facts =
    document.doubtfulCount > 0
      ? `${tn("common.pages", document.pageCount)} · ${tn("courses.state.doubtful", document.doubtfulCount)}`
      : document.blockCount > 0
        ? `${tn("common.pages", document.pageCount)} · ${t("courses.state.reviewed").toLowerCase()}`
        : tn("common.pages", document.pageCount);

  return (
    <button type="button" className="card" onClick={onOpen}>
      <span className="card__head">
        <span className="card__title">{document.title}</span>
        <StatusPill status={document.status} />
      </span>
      <span className="card__meta">{facts}</span>
      <Meter share={share} tone={tone} />
      <span className="card__date">
        {t("common.modified", { when: formatRelative(document.updatedAt) })}
      </span>
    </button>
  );
}

// ------------------------------------------------------------ first launch
function Welcome({
  environment,
  dragging,
  onCreate,
  onSettings,
}: {
  environment: Environment | null;
  dragging: boolean;
  onCreate: (pages?: string[]) => void;
  onSettings: () => void;
}) {
  return (
    <div className="welcome">
      <div className="welcome__icon" aria-hidden="true">
        <svg viewBox="0 0 120 120" width="86" height="86">
          <path d="M0 84 C30 74 58 78 120 62 L120 120 L0 120 Z" className="welcome__ground" />
          <path
            d="M95 22 c0 40 -27 62 -60 62 h-15 c0 -37 25 -62 60 -62 z"
            className="welcome__quill"
          />
          <path d="M20 100 L68 52" className="welcome__quill" />
        </svg>
      </div>

      <div className="welcome__lead">
        <h1 className="welcome__title">{t("welcome.title")}</h1>
        <p className="welcome__text">{t("welcome.text")}</p>
      </div>

      <section className="welcome__check">
        <header className="welcome__check-head">{t("welcome.check.title")}</header>
        {(environment?.tools ?? []).map((tool) => (
          <div
            key={tool.key}
            className={`toolrow ${tool.found ? "" : "toolrow--missing"}`}
          >
            {tool.found ? (
              <span className="toolrow__ok">
                <Icon name="check" size={13} />
              </span>
            ) : (
              <span className="toolrow__warn">
                <Icon name="warning" size={19} />
              </span>
            )}
            <div className="toolrow__body">
              <span className="toolrow__label">{tool.label}</span>
              <span className={`toolrow__hint ${tool.found ? "" : "toolrow__hint--warn"}`}>
                {tool.found ? (tool.role ?? "") : (tool.hint ?? tool.role)}
              </span>
            </div>
            {tool.found ? (
              <span className="toolrow__state">{t("settings.tool.ready")}</span>
            ) : (
              <button type="button" className="btn btn--primary btn--sm" onClick={onSettings}>
                {t("settings.tool.install")}
              </button>
            )}
          </div>
        ))}
      </section>

      <button
        type="button"
        className={`dropzone dropzone--welcome ${dragging ? "dropzone--active" : ""}`}
        onClick={() => onCreate()}
      >
        <Icon name="upload" size={26} />
        <span className="dropzone__title">{t("welcome.drop.title")}</span>
        <span className="dropzone__text">
          {withStrong(
            t("welcome.drop.browse", { browse: t("home.drop.browse") }),
            t("home.drop.browse"),
          )}
        </span>
      </button>

      <p className="welcome__hint">{t("welcome.engine.hint")}</p>
    </div>
  );
}
