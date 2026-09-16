import type { MessageKey, PluralKey } from "../i18n";
import { countGaps } from "../preview/gaps";
import type { Block } from "../types";

/**
 * The ways a lesson can be adapted for a pupil who works under a PAP.
 *
 * One list, read by both places that need it: the adaptation step, which
 * shows a sidebar of them and opens one at a time, and the export step, which
 * offers the adapted copy and says what goes into it. Adding an adaptation is
 * an entry here plus its editor — not a new switch bolted onto a growing row
 * of switches in two different screens.
 *
 * `count` is what the document already carries of that adaptation. Nothing
 * offers an adaptation the teacher has not prepared: a control that would do
 * nothing is still a question they have to read and dismiss.
 */

export type AdaptationId = "gaps";

export type Adaptation = {
  id: AdaptationId;
  /** Name, on the sidebar and beside the export switch. */
  labelKey: MessageKey;
  /** One line saying what it does to the copy. */
  hintKey: MessageKey;
  /** Plural key for the count: "2 trous". */
  countKey: PluralKey;
  /** How much of it this document holds. */
  count: (blocks: Block[]) => number;
};

export const ADAPTATIONS: Adaptation[] = [
  {
    id: "gaps",
    labelKey: "adapt.gaps.name",
    hintKey: "adapt.gaps.hint",
    countKey: "adapt.count",
    count: (blocks) => blocks.reduce((total, block) => total + countGaps(block.latex), 0),
  },
];

/** What this document has been prepared for, in the order of the list. */
export const preparedAdaptations = (blocks: Block[]) =>
  ADAPTATIONS.filter((adaptation) => adaptation.count(blocks) > 0);
