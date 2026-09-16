import { describe, expect, it } from "vitest";
import { adaptable, build, clearGaps, countGaps, hideable, mark, runs, tokenise } from "./gaps";

/** The round trip is the whole contract: what is shown rebuilds what is stored. */
const roundTrip = (latex: string) => build(tokenise(latex));

describe("tokenise / build", () => {
  it("gives a passage back unchanged when nothing is marked", () => {
    for (const latex of [
      "Deux vecteurs sont colinéaires lorsqu'ils ont la même direction.",
      "Soit $\\vec{u}$ et $\\vec{v}$ deux vecteurs du plan.",
      "\\begin{itemize}\n\\item une direction\n\\item un sens\n\\end{itemize}",
      "Le point $A(-4\\,;\\,17)$ appartient-il à $\\mathcal{C}$ ?",
    ]) {
      expect(roundTrip(latex)).toBe(latex);
    }
  });

  it("gives a passage back unchanged when marks are already in it", () => {
    const latex = "Deux vecteurs sont \\trou{colinéaires} lorsqu'ils ont la \\trou{même direction}.";
    expect(roundTrip(latex)).toBe(latex);
  });

  it("reads the words inside a mark as marked", () => {
    const tokens = tokenise("sont \\trou{colinéaires} lorsque");
    expect(tokens.filter((token) => token.gap).map((token) => token.text)).toEqual([
      "colinéaires",
    ]);
  });

  /** A formula is one piece: half of `$a + b$` would not compile. */
  it("keeps a formula whole", () => {
    const tokens = tokenise("on a $a + b = 0$ donc");
    expect(tokens.find((token) => token.text.startsWith("$"))?.text).toBe("$a + b = 0$");
  });

  /** The teacher said text, not diagrams: a hole the size of a figure helps nobody. */
  it("refuses to hide a diagram", () => {
    const tokens = tokenise("voici\n\\begin{tikzpicture}\\draw (0,0) -- (1,1);\\end{tikzpicture}\nle schéma");
    const figure = tokens.find((token) => token.text.includes("tikzpicture"));
    expect(figure && hideable(figure)).toBe(false);
  });

  it("leaves the scaffolding of a list alone", () => {
    const structural = tokenise("\\begin{itemize}\\item une direction\\end{itemize}")
      .filter((token) => /\\(begin|end|item)/.test(token.text));
    expect(structural.length).toBeGreaterThan(0);
    expect(structural.every((token) => !hideable(token))).toBe(true);
  });
});

describe("mark", () => {
  it("hides one word and writes the mark around it", () => {
    const tokens = tokenise("sont colinéaires lorsque");
    const at = tokens.findIndex((token) => token.text === "colinéaires");
    expect(build(mark(tokens, at, at))).toBe("sont \\trou{colinéaires} lorsque");
  });

  /** Three words dragged over are one hole, spaces included — one gesture, one blank. */
  it("joins a stretch of words into a single mark", () => {
    const tokens = tokenise("ont la même direction ici");
    const from = tokens.findIndex((token) => token.text === "la");
    const to = tokens.findIndex((token) => token.text === "direction");
    expect(build(mark(tokens, from, to))).toBe("ont \\trou{la même direction} ici");
  });

  it("takes a marking back when the stretch is already hidden", () => {
    const tokens = tokenise("sont \\trou{colinéaires} lorsque");
    const at = tokens.findIndex((token) => token.text === "colinéaires");
    expect(build(mark(tokens, at, at))).toBe("sont colinéaires lorsque");
  });

  it("marks a formula like any other word", () => {
    const tokens = tokenise("on pose $x = 2$ ici");
    const at = tokens.findIndex((token) => token.text.startsWith("$"));
    expect(build(mark(tokens, at, at))).toBe("on pose \\trou{$x = 2$} ici");
  });

  it("passes over what cannot be hidden inside a stretch", () => {
    const latex = "avant \\begin{tikzpicture}\\draw (0,0);\\end{tikzpicture} après";
    const tokens = tokenise(latex);
    const built = build(mark(tokens, 0, tokens.length - 1));
    expect(built).toContain("\\trou{avant}");
    expect(built).toContain("\\trou{après}");
    expect(built).toContain("\\begin{tikzpicture}\\draw (0,0);\\end{tikzpicture}");
  });

  it("changes nothing when the stretch holds nothing hideable", () => {
    const tokens = tokenise("\\begin{itemize}\\end{itemize}");
    expect(mark(tokens, 0, tokens.length - 1)).toBe(tokens);
  });
});

describe("countGaps / clearGaps / adaptable", () => {
  it("counts the runs a teacher drew, not the words in them", () => {
    expect(countGaps("\\trou{la même direction} et \\trou{colinéaires}")).toBe(2);
    expect(countGaps("rien de marqué")).toBe(0);
    expect(countGaps("un \\trouble passager"), "a word that merely starts like the mark").toBe(0);
  });

  it("takes every marking off a passage", () => {
    expect(clearGaps("sont \\trou{colinéaires} et \\trou{de même sens} ici")).toBe(
      "sont colinéaires et de même sens ici",
    );
  });

  it("knows a passage with nothing to hide", () => {
    expect(adaptable("Deux vecteurs sont colinéaires.")).toBe(true);
    expect(adaptable("\\begin{tikzpicture}\\draw (0,0);\\end{tikzpicture}")).toBe(false);
    expect(adaptable("   ")).toBe(false);
  });
});

describe("runs", () => {
  /** The highlight on screen is the hole in the PDF, space between words included. */
  it("covers the space between two marked words", () => {
    const tokens = tokenise("ont \\trou{la même direction} ici");
    const inside = runs(tokens);
    const covered = tokens.filter((_, at) => inside[at]).map((token) => token.text);
    expect(covered.join("")).toBe("la même direction");
  });

  it("leaves everything outside a mark uncovered", () => {
    const tokens = tokenise("rien de marqué");
    expect(runs(tokens).some(Boolean)).toBe(false);
  });
});
