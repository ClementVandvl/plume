import { useEffect, useRef, useState, type ReactNode } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { renderFigure, renderPassage } from "../api";
import { t } from "../i18n";
import { logError } from "../log";

/**
 * A piece of the document typeset by the real LaTeX engine, cached on disk.
 *
 * A `figure` is a diagram: the review surface is where a wrong one gets
 * caught, so it has to be visible here. A `passage` is a whole block that
 * lays itself out — a table, columns — which the HTML converter can only
 * stack; the engine shows it as it will print, with the charte's preamble.
 *
 * When the engine cannot — not installed, or the block does not compile —
 * a passage falls back to the HTML rendering handed in, with a word on why.
 */
type Props = {
  documentId: string;
  kind: "figure" | "passage";
  source: string;
  /** Shown instead of the image when the engine fails; passages only. */
  fallback?: ReactNode;
};

const RENDER = { figure: renderFigure, passage: renderPassage } as const;

/**
 * Milliseconds between the last edit and the compile it triggers. Each render
 * is a real compilation, serialised on the Rust side: a burst of keystrokes
 * queued one per key, and the preview lagged by the whole queue.
 */
const SETTLE_MS = 500;

export function EngineImage({ documentId, kind, source, fallback }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const first = useRef(true);

  useEffect(() => {
    let cancelled = false;
    setSrc(null);
    setError(null);

    const request = () =>
      RENDER[kind](documentId, source)
        .then((path) => {
          // The mock hands back an inline image; the app, a path on disk.
          if (!cancelled) setSrc(path.startsWith("data:") ? path : convertFileSrc(path));
        })
        .catch((cause) => {
          if (!cancelled) {
            setError(String(cause));
            logError("latex", kind === "figure" ? "Schéma non rendu" : "Passage non rendu", cause);
          }
        });

    // The first request goes at once — a cached image comes back in a blink —
    // and only edits wait for the typing to settle.
    const delay = first.current ? 0 : SETTLE_MS;
    first.current = false;
    const timer = window.setTimeout(request, delay);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [documentId, kind, source]);

  if (error) {
    if (kind === "passage") {
      return (
        <>
          <p className="tex-layout-note">
            {t("preview.layout.fallback")} {error}
          </p>
          {fallback}
        </>
      );
    }
    return <span className="tex-figure tex-figure--error">{error}</span>;
  }

  if (!src) {
    return <span className="tex-figure">{t(`${kind}.compiling`)}</span>;
  }

  return (
    <img
      className={kind === "figure" ? "tex-figure-image" : "tex-passage-image"}
      src={src}
      alt={t(`${kind}.alt`)}
    />
  );
}
