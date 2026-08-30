import { useState, type PointerEvent, type RefObject } from "react";

/**
 * Reordering a list by dragging, on pointer events.
 *
 * Not HTML5 drag-and-drop: the webview's native file drop owns that machinery
 * and swallows internal drags on some platforms. Pointer events work
 * everywhere. Items reorder live under the cursor, so the list always shows
 * what dropping would produce.
 *
 * `onMove(from, to)` is called for each step, and `onSettle` once the pointer
 * is released — the place to persist an order rather than saving on every
 * intermediate position.
 */
/** Just enough of a DOMRect to place a pointer against it. */
export type Box = { top: number; left: number; width: number; height: number };

/**
 * Where an item dragged from `from` should land, given where the pointer is.
 *
 * Pure so the geometry can be tested: a grid wraps, and getting "same row" or
 * "row below" wrong silently sends a page to the far end of the course.
 */
export function insertionIndex(
  boxes: Box[],
  pointer: { x: number; y: number },
  from: number,
  axis: "list" | "grid",
): number {
  let to = 0;
  boxes.forEach((box, at) => {
    const middleY = box.top + box.height / 2;

    // A stacked list only ever moves vertically. A grid wraps, so an item is
    // behind the pointer when it sits on an earlier row, or on the same row and
    // to its left.
    const behind =
      axis === "list"
        ? pointer.y > middleY
        : middleY < pointer.y - box.height / 2
          ? true
          : middleY > pointer.y + box.height / 2
            ? false
            : box.left + box.width / 2 < pointer.x;

    if (behind) to = at + 1;
  });

  // The insertion index counts the dragged item itself when moving down.
  if (to > from) to -= 1;
  return Math.min(to, boxes.length - 1);
}

export function useDragOrder(
  listRef: RefObject<HTMLElement | null>,
  onMove: (from: number, to: number) => void,
  options: {
    /** `list` stacks items; `grid` wraps them into rows. */
    axis?: "list" | "grid";
    onSettle?: () => void;
  } = {},
) {
  const { axis = "list", onSettle } = options;
  const [held, setHeld] = useState<number | null>(null);

  function grab(event: PointerEvent, index: number) {
    event.preventDefault();
    const list = listRef.current;
    if (!list) return;

    let from = index;
    let moved = false;
    setHeld(index);

    const onPointerMove = (pointer: PointerEvent | globalThis.PointerEvent) => {
      const boxes = Array.from(list.children).map((item) =>
        item.getBoundingClientRect(),
      );
      const to = insertionIndex(
        boxes,
        { x: pointer.clientX, y: pointer.clientY },
        from,
        axis,
      );
      if (to !== from) {
        onMove(from, to);
        from = to;
        moved = true;
        setHeld(to);
      }
    };

    const onPointerUp = () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      setHeld(null);
      if (moved) onSettle?.();
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
  }

  return { held, grab };
}

/** Moves one item, returning a new array. */
export function moved<T>(items: T[], from: number, to: number): T[] {
  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}
