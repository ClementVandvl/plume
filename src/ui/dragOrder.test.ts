import { describe, expect, it } from "vitest";
import { insertionIndex, moved, type Box } from "./dragOrder";

/** Three columns of 100×100 thumbnails, 8px apart. */
const grid = (count: number): Box[] =>
  Array.from({ length: count }, (_, index) => ({
    left: (index % 3) * 108,
    top: Math.floor(index / 3) * 108,
    width: 100,
    height: 100,
  }));

/** A stack of full-width rows. */
const list = (count: number): Box[] =>
  Array.from({ length: count }, (_, index) => ({
    left: 0,
    top: index * 60,
    width: 400,
    height: 50,
  }));

describe("insertionIndex", () => {
  it("moves a thumbnail within its row", () => {
    const boxes = grid(6);
    // Dragging the first onto the right half of the second.
    expect(insertionIndex(boxes, { x: 108 + 70, y: 50 }, 0, "grid")).toBe(1);
    // Dragging the third onto the left half of the first.
    expect(insertionIndex(boxes, { x: 20, y: 50 }, 2, "grid")).toBe(0);
  });

  it("crosses rows in reading order", () => {
    const boxes = grid(6);
    // The last thumbnail dragged up to the start of the first row.
    expect(insertionIndex(boxes, { x: 10, y: 50 }, 5, "grid")).toBe(0);
    // The first dragged down onto the second row's middle item.
    expect(insertionIndex(boxes, { x: 108 + 70, y: 158 }, 0, "grid")).toBe(4);
  });

  it("never lands outside the list", () => {
    const boxes = grid(6);
    expect(insertionIndex(boxes, { x: 9999, y: 9999 }, 0, "grid")).toBe(5);
    expect(insertionIndex(boxes, { x: -50, y: -50 }, 3, "grid")).toBe(0);
  });

  it("keeps the stacked list on its vertical midpoints", () => {
    const boxes = list(4);
    // The right half of a row must not count as "past" it, as it would in a
    // grid: a stacked row is only ever crossed from above or below.
    expect(insertionIndex(boxes, { x: 390, y: 10 }, 2, "list")).toBe(0);
    expect(insertionIndex(boxes, { x: 10, y: 190 }, 0, "list")).toBe(2);
  });
});

describe("moved", () => {
  it("takes the item out and puts it back at the target", () => {
    expect(moved(["a", "b", "c", "d"], 0, 2)).toEqual(["b", "c", "a", "d"]);
    expect(moved(["a", "b", "c", "d"], 3, 0)).toEqual(["d", "a", "b", "c"]);
    expect(moved(["a", "b", "c"], 1, 1)).toEqual(["a", "b", "c"]);
  });
});
