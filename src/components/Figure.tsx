import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { renderFigure } from "../api";
import { logError } from "../log";

/**
 * A diagram, compiled by the real LaTeX engine and cached on disk.
 *
 * The review surface is where a wrong diagram gets caught, so it has to be
 * visible here — a placeholder saying "see the PDF" defeats the purpose.
 */
export function Figure({ documentId, tikz }: { documentId: string; tikz: string }) {
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSrc(null);
    setError(null);

    renderFigure(documentId, tikz)
      .then((path) => {
        if (!cancelled) setSrc(convertFileSrc(path));
      })
      .catch((cause) => {
        if (!cancelled) {
          setError(String(cause));
          logError("latex", "Schéma non rendu", cause);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [documentId, tikz]);

  if (error) {
    return <span className="tex-figure tex-figure--error">{error}</span>;
  }

  if (!src) {
    return <span className="tex-figure">Compilation du schéma…</span>;
  }

  return <img className="tex-figure-image" src={src} alt="Schéma du cours" />;
}
