import { useCallback, useMemo, useState } from "react";
import { ADAPTATIONS, type AdaptationId } from "../adapt/kinds";
import { t, tn } from "../i18n";
import type { Colours } from "../preview/commands";
import type { Block, Transcript } from "../types";
import { GapEditor } from "./GapEditor";

/**
 * Adapting the lesson for a pupil who works under a PAP.
 *
 * A step of its own, between the review and the PDF, and skipped by everyone
 * who does not need it. The review already answers "is this what the page
 * said"; asking "what should this pupil not have to copy" in the same place
 * would put two different jobs behind the same row of buttons.
 *
 * One adaptation is open at a time, chosen from the sidebar. The list comes
 * from `adapt/kinds.ts` and there is one entry in it today — text with holes.
 * The shape is what matters: a second adaptation is an entry and an editor,
 * not a second screen, and not more controls crowding this one.
 */

type Props = {
  transcript: Transcript;
  blocks: Block[];
  colours: Colours;
  /** Saves one passage whose LaTeX an adaptation changed. */
  onPersist: (block: Block) => Promise<void>;
};

export function AdaptView({ transcript, blocks, colours, onPersist }: Props) {
  const [open, setOpen] = useState<AdaptationId>(ADAPTATIONS[0].id);
  /** The open editor's own "clear everything", raised for the bar. */
  const [clear, setClear] = useState<(() => Promise<void>) | null>(null);
  const onClearRef = useCallback(
    // Kept in state, and a function put there directly would be taken for a
    // state updater and called instead of stored.
    (action: (() => Promise<void>) | null) => setClear(() => action),
    [],
  );

  const counts = useMemo(
    () => new Map(ADAPTATIONS.map((adaptation) => [adaptation.id, adaptation.count(blocks)])),
    [blocks],
  );
  const current = ADAPTATIONS.find((adaptation) => adaptation.id === open) ?? ADAPTATIONS[0];
  const count = counts.get(current.id) ?? 0;

  return (
    <section className="adapt">
      <aside className="adapt__side">
        <span className="adapt__side-title">{t("adapt.side.title")}</span>
        {ADAPTATIONS.map((adaptation) => {
          const held = counts.get(adaptation.id) ?? 0;
          return (
            <button
              key={adaptation.id}
              type="button"
              className={`adaptkind ${adaptation.id === open ? "adaptkind--on" : ""}`}
              onClick={() => setOpen(adaptation.id)}
              aria-pressed={adaptation.id === open}
            >
              <span className="adaptkind__copy">
                <span className="adaptkind__label">{t(adaptation.labelKey)}</span>
                <span className="adaptkind__hint">{t(adaptation.hintKey)}</span>
              </span>
              {held > 0 && <span className="adaptkind__count">{held}</span>}
            </button>
          );
        })}
        <p className="adapt__side-note">{t("adapt.side.more")}</p>
      </aside>

      <div className="adapt__main">
        <div className="adapt__bar">
          <div className="adapt__count">
            <span className="adapt__count-value">{count}</span>
            <span className="adapt__count-label">{tn(current.countKey, count)}</span>
          </div>
          <button
            type="button"
            className="btn btn--outline btn--sm"
            onClick={() => void clear?.()}
            disabled={count === 0 || !clear}
          >
            {t("adapt.clear")}
          </button>
        </div>

        {current.id === "gaps" && (
          <GapEditor
            transcript={transcript}
            colours={colours}
            onPersist={onPersist}
            onClearRef={onClearRef}
          />
        )}
      </div>
    </section>
  );
}
