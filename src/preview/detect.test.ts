import { describe, expect, it } from "vitest";
import { hasLayout, hasStrayAlignment, route } from "./detect";

describe("hasStrayAlignment", () => {
  it("catches an alignment tab with no environment around it", () => {
    expect(
      hasStrayAlignment("A = \\frac{3}{5} &= \\frac{9}{15} \\\\ &= \\frac{16}{15}"),
    ).toBe(true);
    expect(hasStrayAlignment("$A &= 1 \\\\ &= 2$")).toBe(true);
  });

  it("accepts a tab inside its environment", () => {
    expect(hasStrayAlignment("$\\begin{aligned}[t] A &= 1 \\\\ &= 2 \\end{aligned}$")).toBe(
      false,
    );
    expect(hasStrayAlignment("\\begin{align*} A &= 1 \\end{align*}")).toBe(false);
    expect(hasStrayAlignment("\\begin{pmatrix} a & b \\end{pmatrix}")).toBe(false);
    expect(hasStrayAlignment("Pierre \\& Marie Curie")).toBe(false);
  });
});

describe("hasLayout", () => {
  // The same list as `render::LAYOUT_COMMANDS`; the two once disagreed, and a
  // passage was reported in the console without being flagged on screen.
  it("flags every construct the console warns about", () => {
    for (const latex of [
      "\\begin{minipage}[t]{0.48\\textwidth}a\\end{minipage}",
      "\\begin{multicols}{2}a\\end{multicols}",
      "\\begin{tabular}{cc}a & b\\end{tabular}",
      "\\hfill\\rule{0.4pt}{6cm}\\hfill",
      "avant\\newpage après",
      "\\vspace{1cm}",
    ]) {
      expect(hasLayout(latex)).toBe(true);
    }
  });

  it("stays quiet on ordinary content", () => {
    expect(hasLayout("Soient $\\vec{u}$ et $\\vec{v}$ deux vecteurs.")).toBe(false);
    expect(hasLayout("\\begin{center}\\begin{tikzpicture}\\end{tikzpicture}\\end{center}")).toBe(
      false,
    );
    expect(hasLayout("$\\begin{aligned} a &= b \\end{aligned}$")).toBe(false);
  });
});

describe("route", () => {
  /**
   * The case this exists for: a table with a diagram in every row. Split
   * around its diagrams it was prose with stray `&` between two pictures;
   * whole, it is one object for the engine.
   */
  it("sends a block with its own layout to the engine, diagrams and all", () => {
    const table =
      "\\begin{tabular}{|c|c|}\\hline $x \\in [a;b]$ & \\begin{tikzpicture}\\draw (0,0) -- (1,0);\\end{tikzpicture} \\\\ \\hline\\end{tabular}";
    expect(route(table)).toEqual({ renderer: "engine", reason: "layout" });
  });

  it("converts prose in the page and sends only its diagrams to the engine", () => {
    const plan = route(
      "Soit le repère :\n\\begin{tikzpicture}\\draw (0,0) -- (1,0);\\end{tikzpicture}\nd'où le résultat.",
    );
    expect(plan.renderer).toBe("html");
    if (plan.renderer !== "html") return;
    expect(plan.segments.map((segment) => segment.kind)).toEqual(["text", "figure", "text"]);
  });

  it("keeps plain prose as one converted segment", () => {
    const plan = route("Soient $\\vec{u}$ et $\\vec{v}$ deux vecteurs.");
    expect(plan.renderer).toBe("html");
    if (plan.renderer !== "html") return;
    expect(plan.segments).toHaveLength(1);
  });
});
