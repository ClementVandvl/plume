import { useEffect, useMemo, useRef, useState } from "react";
import { t } from "../i18n";
import { logError } from "../log";
import { hasLayout } from "../preview/detect";
import {
  build,
  clearGaps,
  hideable,
  mark,
  runs,
  tokenise,
  type Token,
} from "../preview/gaps";
import { inlineHtml } from "../preview/latexToHtml";
import type { Colours } from "../preview/commands";
import { KIND_LABEL, type Block, type Transcript } from "../types";

/**
 * Hiding words, for a copy a pupil fills in rather than copies out.
 *
 * The teacher drags across a stretch of text and it becomes a hole: printed
 * as a ruled space in the adapted copy, printed as itself in everyone else's.
 * The marking lives in the passage's LaTeX (see `preview/gaps.ts`), so it
 * survives every edit the review can make.
 */

type Props = {
  transcript: Transcript;
  colours: Colours;
  /** Saves one passage whose LaTeX the marking changed. */
  onPersist: (block: Block) => Promise<void>;
  /** Raised so the page can offer "clear everything" from its own bar. */
  onClearRef: (clear: (() => Promise<void>) | null) => void;
};

/**
 * A passage as it is worked on here: cut into clickable pieces, with the runs
 * those pieces form — so the highlight covers the space between two marked
 * words exactly as the hole in the PDF will.
 */
type Row = { block: Block; tokens: Token[]; inside: boolean[] };

export function GapEditor({ transcript, colours, onPersist, onClearRef }: Props) {
  const [error, setError] = useState<string | null>(null);
  /** The stretch being dragged over right now: one passage, two token indices. */
  const [drag, setDrag] = useState<{ blockId: string; from: number; to: number } | null>(null);
  const dragging = useRef(drag);
  dragging.current = drag;

  const rows: Row[] = useMemo(() => {
    const out: Row[] = [];
    for (const page of transcript.pages) {
      for (const block of page.blocks) {
        // A passage that lays itself out — a table, columns — reads as a
        // sentence nowhere: cut into words it comes apart into alignment
        // tabs and rules, and the teacher would be marking scaffolding.
        // The review typesets those with the engine for the same reason.
        if (hasLayout(block.latex)) continue;
        const tokens = tokenise(block.latex);
        // A heading carries its text in `title` and a figure carries a
        // drawing: neither has a word to hide, and an empty row would only
        // be something to scroll past.
        if (tokens.some(hideable)) out.push({ block, tokens, inside: runs(tokens) });
      }
    }
    return out;
  }, [transcript]);

  async function commit(row: Row, from: number, to: number) {
    const latex = build(mark(row.tokens, from, to));
    if (latex === row.block.latex) return;
    try {
      setError(null);
      await onPersist({ ...row.block, latex });
    } catch (cause) {
      setError(String(cause));
      logError("workspace", "Aménagement non enregistré", cause);
    }
  }

  // The drag ends wherever the button is released, inside the passage or well
  // outside it; a listener on the window is the only thing that sees both.
  useEffect(() => {
    const finish = () => {
      const current = dragging.current;
      setDrag(null);
      if (!current) return;
      const row = rows.find(({ block }) => block.id === current.blockId);
      if (row) void commit(row, current.from, current.to);
    };
    window.addEventListener("pointerup", finish);
    return () => window.removeEventListener("pointerup", finish);
    // `rows` is what a commit needs; re-binding on each change keeps it fresh.
  }, [rows]);

  // Clearing belongs to the page's bar, beside the count, but only this
  // editor knows how to do it — so it hands the page the action.
  useEffect(() => {
    const clear = async () => {
      try {
        setError(null);
        for (const row of rows) {
          const latex = clearGaps(row.block.latex);
          if (latex !== row.block.latex) await onPersist({ ...row.block, latex });
        }
      } catch (cause) {
        setError(String(cause));
        logError("workspace", "Aménagements non effacés", cause);
      }
    };
    onClearRef(clear);
    return () => onClearRef(null);
  }, [rows, onClearRef]);

  return (
    <>
      {error && (
        <p className="notice notice--error" role="alert">
          {error}
        </p>
      )}

      <div className="paper">
        <p className="adapt__intro">{t("adapt.gaps.how")}</p>

        {rows.length === 0 ? (
          <p className="adapt__empty">{t("adapt.nothing")}</p>
        ) : (
          rows.map((row) => (
            <div key={row.block.id} className="adapt__block">
              <span className="adapt__kind">{KIND_LABEL[row.block.kind] ?? row.block.kind}</span>
              <p className="adapt__text">
                {row.tokens.map((token, at) => {
                  const inStretch =
                    drag?.blockId === row.block.id &&
                    at >= Math.min(drag.from, drag.to) &&
                    at <= Math.max(drag.from, drag.to);
                  const html = { __html: inlineHtml(token.text, colours) };

                  if (!hideable(token)) {
                    return (
                      <span
                        key={at}
                        className={row.inside[at] ? "gapword--on" : undefined}
                        dangerouslySetInnerHTML={html}
                      />
                    );
                  }
                  return (
                    <span
                      key={at}
                      role="button"
                      tabIndex={0}
                      className={`gapword ${row.inside[at] ? "gapword--on" : ""} ${
                        inStretch ? "gapword--drag" : ""
                      }`}
                      onPointerDown={(event) => {
                        event.preventDefault();
                        setDrag({ blockId: row.block.id, from: at, to: at });
                      }}
                      onPointerEnter={() =>
                        setDrag((current) =>
                          current?.blockId === row.block.id ? { ...current, to: at } : current,
                        )
                      }
                      // The keyboard hides one word at a time: dragging has no
                      // equivalent, and a word is the unit either way.
                      onKeyDown={(event) => {
                        if (event.key !== "Enter" && event.key !== " ") return;
                        event.preventDefault();
                        void commit(row, at, at);
                      }}
                      dangerouslySetInnerHTML={html}
                    />
                  );
                })}
              </p>
            </div>
          ))
        )}
      </div>
    </>
  );
}
