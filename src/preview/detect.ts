/**
 * What a block contains, and which renderer it goes to.
 *
 * The review preview has two renderers. The HTML converter (`latexToHtml`) is
 * instant and lives in the page, but knows only prose, maths and lists: it
 * stacks everything in one column. The engine (`EngineImage`) is the real LaTeX
 * compiler — exact, and a second or two per block. This module is the only
 * place that decides which of the two a block gets. The converter never learns
 * layout, and the engine is never asked for prose: a table written any way
 * LaTeX allows goes to the engine, so there is no list of table syntaxes to
 * keep up with.
 */

/** A block's body, cut where the renderer changes. */
export type Segment = { kind: "text"; latex: string } | { kind: "figure"; tikz: string };

/**
 * Where a block is drawn.
 *
 * `engine` hands the whole body to the compiler, with the charte's preamble:
 * what is shown is what will print. `html` converts the prose in the page and
 * sends only the diagrams to the compiler.
 */
export type Route =
  | { renderer: "engine"; reason: "layout" }
  | { renderer: "html"; segments: Segment[] };

const TIKZ = /\\begin\{tikzpicture\}[\s\S]*?\\end\{tikzpicture\}/g;

/**
 * Constructs that place things — columns, tables, struts, page breaks — which
 * a single column cannot reproduce.
 *
 * A fixed list, not a comparison of the two renderings: telling "this will
 * look different" in general would mean typesetting the page twice and diffing
 * it. Kept identical to `render::LAYOUT_COMMANDS`, which writes the console
 * warning: the two disagreed once, and a passage was reported in one place and
 * silent in the other.
 */
const LAYOUT = /\\begin\{(minipage|multicols|tabular)\}|\\rule\{|\\hfill|\\newpage|\\vspace/;

/** Environments where `&` is an alignment tab rather than a mistake. */
const ALIGNING =
  "align\\*?|aligned|alignat\\*?|gather\\*?|gathered|split|cases|array|" +
  "[pbvV]?matrix|smallmatrix|tabularx?|flalign\\*?|multline\\*?|eqnarray\\*?";

const ALIGNING_ENVIRONMENT = new RegExp(
  `\\\\begin\\{(${ALIGNING})\\}[\\s\\S]*?\\\\end\\{\\1\\}`,
  "g",
);

export const hasFigure = (latex: string) => /\\begin\{tikzpicture\}/.test(latex);

/** Whether a passage lays itself out, in ways a single column cannot reproduce. */
export const hasLayout = (latex: string) => LAYOUT.test(latex);

/**
 * An `&` outside any environment that gives it a meaning.
 *
 * Mirrors `render::has_stray_alignment`. The model reaches for alignment tabs
 * on its own when a calculation runs over several lines; emitted bare they are
 * a LaTeX error and the preview shows them as literal text. Flagged rather
 * than repaired: the block belongs to the teacher, and the correction pass is
 * where it gets fixed.
 */
export const hasStrayAlignment = (latex: string) =>
  latex.replace(/\\&/g, "").replace(ALIGNING_ENVIRONMENT, "").includes("&");

/**
 * Splits a block around its diagrams, so each can be compiled on its own
 * while the prose between them stays in the page.
 */
export function splitFigures(latex: string): Segment[] {
  const segments: Segment[] = [];
  let cursor = 0;

  for (const match of latex.matchAll(TIKZ)) {
    const at = match.index ?? 0;
    const before = latex.slice(cursor, at);
    if (before.trim()) segments.push({ kind: "text", latex: before });
    segments.push({ kind: "figure", tikz: match[0] });
    cursor = at + match[0].length;
  }

  const rest = latex.slice(cursor);
  if (rest.trim()) segments.push({ kind: "text", latex: rest });
  return segments.length > 0 ? segments : [{ kind: "text", latex }];
}

/**
 * Decides the renderer for a block.
 *
 * Layout wins over everything: a table with a diagram in every row is one
 * object to the engine, and would be a diagram, some prose with stray `&`, a
 * diagram again to the converter — which is exactly what it used to show.
 */
export function route(latex: string): Route {
  if (hasLayout(latex)) return { renderer: "engine", reason: "layout" };
  return { renderer: "html", segments: splitFigures(latex) };
}
