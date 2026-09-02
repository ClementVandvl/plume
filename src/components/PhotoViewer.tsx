import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { t } from "../i18n";
import { Icon } from "../ui/Icon";

/**
 * A page photograph, full screen.
 *
 * The reason to open a photograph is always the same — reading the handwriting
 * Plume was unsure about — so the same view serves the Photos step and the
 * review panel. Enlarging in place was never enough: a side panel is a few
 * hundred pixels wide, and that is where a doubtful passage has to be settled.
 *
 * The pages are all here, so the arrows walk the course: the passage before was
 * often on the page before.
 */
export function PhotoViewer({
  paths,
  index,
  onIndex,
  onClose,
}: {
  paths: string[];
  index: number;
  onIndex: (index: number) => void;
  onClose: () => void;
}) {
  const [zoomed, setZoomed] = useState(false);

  useEffect(() => {
    setZoomed(false);
  }, [index]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
      else if (event.key === "ArrowLeft" && index > 0) onIndex(index - 1);
      else if (event.key === "ArrowRight" && index < paths.length - 1) onIndex(index + 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [index, paths.length, onClose, onIndex]);

  const path = paths[index];
  if (!path) return null;

  return (
    <div
      className="viewer"
      role="dialog"
      aria-modal="true"
      aria-label={t("viewer.title", { number: index + 1 })}
      // Clicking the backdrop closes; clicking the photograph itself does not.
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <header className="viewer__bar">
        <span className="viewer__count">
          {t("viewer.of", { number: index + 1, total: paths.length })}
        </span>
        <div className="viewer__tools">
          <button
            type="button"
            className="btn btn--outline btn--sm"
            onClick={() => setZoomed((current) => !current)}
          >
            {zoomed ? t("viewer.fit") : t("viewer.zoom")}
          </button>
          <button
            type="button"
            className="icon-btn icon-btn--close"
            onClick={onClose}
            aria-label={t("common.close")}
          >
            ×
          </button>
        </div>
      </header>

      <div className={`viewer__stage ${zoomed ? "viewer__stage--zoomed" : ""}`}>
        <img
          className="viewer__image"
          src={convertFileSrc(path)}
          alt={t("pages.page", { number: index + 1 })}
          onClick={() => setZoomed((current) => !current)}
        />
      </div>

      {paths.length > 1 && (
        <>
          <button
            type="button"
            className="viewer__step viewer__step--prev"
            onClick={() => onIndex(index - 1)}
            disabled={index === 0}
            aria-label={t("viewer.previous")}
          >
            <Icon name="back" size={20} />
          </button>
          <button
            type="button"
            className="viewer__step viewer__step--next"
            onClick={() => onIndex(index + 1)}
            disabled={index === paths.length - 1}
            aria-label={t("viewer.next")}
          >
            <Icon name="next" size={20} />
          </button>
        </>
      )}
    </div>
  );
}
