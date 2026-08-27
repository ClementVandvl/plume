import { describe, expect, it } from "vitest";
import { latexToHtml } from "./latexToHtml";
import blocks from "./__fixtures__/blocks.json";

/**
 * The fixture is real output from a real run, not hand-written samples. The
 * preview's whole job is to never show LaTeX source to the teacher, so that is
 * what is asserted.
 */

const RAW_LATEX = /\\[a-zA-Z]+|\\begin\{|\\end\{/;

/**
 * Debris an environment leaves behind once its name is stripped but not its
 * arguments — `[t]{0.48}` from a minipage. It carries no backslash, so the
 * check above walks straight past it.
 */
const ARGUMENT_DEBRIS = /[{}]|\[[a-z]\]\s*\{/;

/**
 * What the teacher actually sees. KaTeX emits a MathML branch containing the
 * original TeX in an <annotation> element — never rendered, never visible, and
 * a false positive if left in.
 */
const visibleText = (html: string) =>
  html.replace(/<math[\s\S]*?<\/math>/g, "").replace(/<[^>]+>/g, "");

describe("latexToHtml", () => {
  it("leaks no LaTeX source on any real block", () => {
    const leaking = blocks
      .map((block) => ({ block, html: latexToHtml(block.latex) }))
      .filter(({ html }) => {
        const visible = visibleText(html);
        return RAW_LATEX.test(visible) || ARGUMENT_DEBRIS.test(visible);
      });

    expect(
      leaking.map((entry) => `${entry.block.id} (${entry.block.kind})`),
    ).toEqual([]);
  });

  it("renders a diagram as a placeholder, not as markup text", () => {
    const html = latexToHtml(
      "\\begin{center}\n\\begin{tikzpicture}\\draw (0,0) -- (1,1);\\end{tikzpicture}\n\\end{center}",
    );
    expect(html).toContain('class="tex-figure"');
    expect(html).not.toContain("&lt;span");
    expect(html).not.toContain("tikzpicture");
  });

  it("typesets inline maths", () => {
    const html = latexToHtml("Le vecteur $\\overrightarrow{AB}$ est nul.");
    expect(html).toContain("katex");
    expect(html).toContain("Le vecteur");
  });

  // The maths template asks for continued calculations to be grouped in an
  // `aligned` environment rather than centred on their own line. The preview
  // has to typeset that, or the rule would look broken before it is exported.
  it("typesets a calculation aligned on its equals signs", () => {
    const html = latexToHtml(
      "2) $\\begin{aligned}[t]\n-2x(4x-1) &= -2x \\times 4x - (-2x) \\times 1 \\\\\n&= -8x^2 + 2x\n\\end{aligned}$",
    );
    expect(html).toContain("katex");
    // KaTeX ignores the [t] option and would otherwise typeset it as text.
    // It is checked in the visible half: the source is echoed verbatim in the
    // MathML annotation, where its presence means nothing.
    const visible = html.slice(html.indexOf('katex-html'));
    expect(visible).not.toContain("[");
    expect(html).not.toContain("ParseError");
  });

  it("turns itemize into a list", () => {
    const html = latexToHtml(
      "Caractérisé par :\n\\begin{itemize}\n\\item une direction\n\\item un sens\n\\end{itemize}",
    );
    expect(html).toContain("<ul");
    expect((html.match(/<li>/g) ?? []).length).toBe(2);
  });

  it("escapes anything that looks like markup in the prose", () => {
    const html = latexToHtml("Soit a <script>alert(1)</script> b");
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });

  it("drops an unhandled environment together with its arguments", () => {
    const html = latexToHtml(
      "\\begin{minipage}[t]{0.48\\textwidth}\nRelation de Chasles\n\\end{minipage}",
    );
    expect(html).toContain("Relation de Chasles");
    expect(html).not.toContain("0.48");
    expect(html).not.toContain("[t]");
  });

  it("resolves nested commands without leaking braces", () => {
    const html = latexToHtml(
      "\\noindent\\mcul{mcProp}{\\textbf{Relation de Chasles :}} suite",
      { mcProp: "#117A65" },
    );
    expect(html).toContain("<u");
    expect(html).toContain("<strong>Relation de Chasles :</strong>");
    expect(html).toContain("#117A65");
    expect(html).not.toContain("{");
    expect(html).not.toContain("mcProp<");
  });

  it("drops spacing commands with their arguments", () => {
    const html = latexToHtml("gauche \\hfill\\vspace{2cm}\\hfill droite");
    expect(html).toContain("gauche");
    expect(html).toContain("droite");
    expect(html).not.toContain("2cm");
  });

  it("removes LaTeX comments but keeps an escaped percent", () => {
    const html = latexToHtml("fin de boîte\\end{minipage}%\nsuite 50\\% du total");
    expect(html).not.toMatch(/%\s*%/);
    expect(html).toContain("suite 50% du total");
  });

  it("draws a rule instead of dropping it", () => {
    const html = latexToHtml("gauche \\rule{0.4pt}{6cm} droite");
    expect(html).toContain('class="tex-rule"');
    expect(html).toContain("width:0.4pt");
    expect(html).toContain("height:6cm");
  });

  it("keeps common text symbols readable", () => {
    expect(latexToHtml("un vecteur \\ldots")).toContain("…");
  });
});
