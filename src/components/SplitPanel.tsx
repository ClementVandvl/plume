import { useState } from "react";
import { t } from "../i18n";
import { latexToHtml } from "../preview/latexToHtml";

/**
 * Choosing where to cut a passage in two.
 *
 * The cut points are the blank lines the passage already has — where a worked
 * example separates its statement from its answer, where a diagram stands
 * apart. Offering the raw LaTeX with a caret would be precise and useless: the
 * teacher is deciding about their course, not about markup, so each piece is
 * shown typeset and the choice is a line between two of them.
 */

/** The passage's own paragraphs, kept verbatim so joining them restores it. */
export function chunksOf(latex: string): string[] {
  return latex
    .split(/\n[ \t]*\n/)
    .map((chunk) => chunk.trim())
    .filter((chunk) => chunk.length > 0);
}

export function SplitPanel({
  latex,
  onCancel,
  onSplit,
}: {
  latex: string;
  onCancel: () => void;
  onSplit: (head: string, tail: string) => Promise<void>;
}) {
  const chunks = chunksOf(latex);
  const [busy, setBusy] = useState(false);

  if (chunks.length < 2) {
    return (
      <div className="split-choice">
        <p className="field__hint">{t("split.tooShort")}</p>
        <button type="button" className="btn btn--outline btn--sm" onClick={onCancel}>
          {t("common.cancel")}
        </button>
      </div>
    );
  }

  async function cutAt(index: number) {
    setBusy(true);
    try {
      await onSplit(
        chunks.slice(0, index).join("\n\n"),
        chunks.slice(index).join("\n\n"),
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="split-choice">
      <p className="field__hint">{t("split.hint")}</p>

      {chunks.map((chunk, index) => (
        <div key={index}>
          {index > 0 && (
            <button
              type="button"
              className="split-choice__cut"
              onClick={() => cutAt(index)}
              disabled={busy}
            >
              <span className="split-choice__label">{t("split.here")}</span>
            </button>
          )}
          <div
            className="split-choice__piece"
            dangerouslySetInnerHTML={{ __html: latexToHtml(chunk) }}
          />
        </div>
      ))}

      <button
        type="button"
        className="btn btn--outline btn--sm"
        onClick={onCancel}
        disabled={busy}
      >
        {t("common.cancel")}
      </button>
    </div>
  );
}
