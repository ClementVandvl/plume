import { describe, expect, it } from "vitest";
import { needsReview } from "./review";
import type { Block } from "../types";

const block = (over: Partial<Block> = {}): Block => ({
  id: "p01-b01",
  kind: "text",
  title: null,
  number: null,
  latex: "x",
  confidence: 1,
  doubt: null,
  audience: [],
  align: null,
  note: null,
  taughtEnd: false,
  reviewed: false,
  ...over,
});

describe("what still wants the teacher's eye", () => {
  it("flags a photographed passage only where the reading was unsure", () => {
    expect(needsReview(block({ confidence: 0.71 }), "photo")).toBe(true);
    expect(needsReview(block({ confidence: 0.99 }), "photo")).toBe(false);
  });

  /**
   * The regression this exists for: an imported sheet has confidence 1.0 on
   * every passage, because nothing was read and so nothing could be misread.
   * Left to the photographed rule it would arrive announcing "tout est relu"
   * on mathematics nobody has looked at.
   */
  it("flags every unopened passage of a written document", () => {
    expect(needsReview(block({ confidence: 1 }), "written")).toBe(true);
    expect(needsReview(block({ confidence: 1, reviewed: true }), "written")).toBe(false);
  });

  it("stops flagging once the teacher has been through it", () => {
    expect(needsReview(block({ confidence: 0.4, reviewed: true }), "photo")).toBe(false);
  });

  /** A document saved before provenance existed reads as photographed. */
  it("treats an unstated origin as a photographed document", () => {
    expect(needsReview(block({ confidence: 1 }))).toBe(false);
    expect(needsReview(block({ confidence: 0.5 }))).toBe(true);
  });
});
