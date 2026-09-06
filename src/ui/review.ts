import { DOUBT_THRESHOLD, type Block } from "../types";

/**
 * Whether a passage still wants the teacher's eye.
 *
 * Two documents, two meanings, and they are not interchangeable.
 *
 * A photographed document has a source: the passage says something the page also
 * says, and the model reports how well it made the handwriting out. Reading it
 * back is only worth the teacher's time where that reading was unsure.
 *
 * A written document has no source. `confidence` would be 1.0 for every passage
 * and would mean nothing — nothing was read, so nothing could be misread. What
 * a generated exercise sheet needs is not verification against a page but the
 * teacher deciding the mathematics is right, and until they have opened a
 * passage, they have not. Marking such a document "tout est relu" the second it
 * arrives is the one thing this must not do.
 *
 * Kept in one place because it was written out four times — twice in the document
 * view and twice in the preview — and a filter that disagrees with the count
 * beside it sends the teacher looking for passages that are not there.
 */
export function needsReview(block: Block, origin?: string): boolean {
  if (block.reviewed) return false;
  return origin === "written" || block.confidence < DOUBT_THRESHOLD;
}
