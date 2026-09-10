import { applyCommands, type Colours } from "./commands";
import { escapeHtml, SAFE_LENGTH, Slots } from "./html";
import { renderMaths } from "./maths";

/**
 * Turns a block's LaTeX body into HTML for the review preview.
 *
 * This is an *approximation*, deliberately. The authoritative render is the PDF
 * produced by the real compiler; this exists so the teacher can read their
 * document as a document while reviewing, instead of scanning a list of boxes.
 * It knows prose, maths and lists, and nothing about layout: a block that lays
 * itself out is routed to the engine by `detect.ts` before it gets here, so no
 * table syntax is ever added to this file.
 *
 * The conversion is a fixed sequence of passes, each a small function named
 * for what it does and documented with the symptom that earned it. Adding a
 * behaviour means adding a pass to the right list, or a line to the command
 * table in `commands.ts` — never a branch inside another pass.
 */

type Context = { colours: Colours; keep: (html: string) => string };
type Pass = (text: string, context: Context) => string;

const ESCAPED_PERCENT = "@@PLUME_PCT@@";

/**
 * Symptom: stray `%` characters in the prose.
 *
 * An unescaped `%` comments out the rest of its line, which is how authors glue
 * two boxes together without a space — `\end{minipage}%`. The escaped form is
 * kept.
 */
const stripComments: Pass = (text) =>
  text
    .replace(/\\%/g, ESCAPED_PERCENT)
    .replace(/%[^\n]*/g, "")
    .replace(new RegExp(ESCAPED_PERCENT, "g"), "\\%");

// ---- Parking: HTML produced before the escaping, kept aside until the end.

/**
 * A diagram met by the converter itself, outside the preview's routing —
 * an insert panel, an import. Named for what it is, not shown as markup.
 */
const parkFigures: Pass = (text, { keep }) =>
  text.replace(/\\begin\{tikzpicture\}[\s\S]*?\\end\{tikzpicture\}/g, () =>
    keep('<span class="tex-figure">Schéma — visible dans le PDF</span>'),
  );

const parkDisplayMaths: Pass = (text, { keep }) =>
  text
    .replace(/\\\[([\s\S]*?)\\\]/g, (_, body) => keep(renderMaths(body, true)))
    .replace(/\$\$([\s\S]*?)\$\$/g, (_, body) => keep(renderMaths(body, true)));

/**
 * Symptom: half a formula parked as maths, the other half on the page as raw
 * LaTeX.
 *
 * A remark set beside a calculation carries its own maths — the
 * `inline-comment` convention writes `&& \text{on met les $x$ d'un côté}` — and
 * those inner delimiters sit inside the `\text` group. A flat `$[^$]+$` closed
 * on the first of them. So each `$…$` closes on the first delimiter at brace
 * depth zero; backslash escapes are stepped over, so `\$` and `\{` do not move
 * the depth.
 */
const parkInlineMaths: Pass = (text, { keep }) => {
  let out = "";
  let index = 0;

  for (let open = text.indexOf("$", index); open >= 0; open = text.indexOf("$", index)) {
    let depth = 0;
    let close = -1;
    for (let cursor = open + 1; cursor < text.length; cursor++) {
      const char = text[cursor];
      if (char === "\\") cursor += 1;
      else if (char === "{") depth += 1;
      else if (char === "}") depth -= 1;
      else if (char === "$" && depth <= 0) {
        close = cursor;
        break;
      }
    }
    if (close < 0) break;

    out += text.slice(index, open) + keep(renderMaths(text.slice(open + 1, close), false));
    index = close + 1;
  }

  return out + text.slice(index);
};

/**
 * Symptom: a literal "&=" in the prose and `\frac{3}{5}` eaten down to "5".
 *
 * A maths environment written on its own, outside any `$` or `\[`: the model
 * emits these for a multi-line calculation. Runs after the delimiters above,
 * so only genuinely top-level environments are left to match.
 */
const parkMathsEnvironments: Pass = (text, { keep }) =>
  text.replace(
    /\\begin\{(align\*?|alignat\*?|gather\*?|gathered|aligned|equation\*?|multline\*?|flalign\*?|split|cases)\}[\s\S]*?\\end\{\1\}/g,
    (whole) => keep(renderMaths(whole, true)),
  );

// ---- Shaping: structure read off `\begin{…}` and `\\`, before commands go.

/** A blank line is a paragraph break, wherever it falls. */
const splitParagraphs = (text: string) =>
  text
    .split(/\n\s*\n/)
    .map((chunk) => chunk.trim())
    .filter(Boolean);

/**
 * Symptom: a `<p>` opened in one list item and closed in the next.
 *
 * The paragraph pass runs on the assembled HTML and cannot see where a list
 * starts: an item written as several paragraphs — a calculation, then its
 * conclusion — had its blank lines cut the `<ol>` itself. Splitting here
 * instead leaves the list a single blank-line-free chunk, which that pass
 * then passes over. A one-paragraph item stays bare, so the marker sits beside
 * its text rather than against a block of its own.
 */
function itemParagraphs(item: string): string {
  const chunks = splitParagraphs(item);
  return chunks.length > 1 ? chunks.map((chunk) => `<p>${chunk}</p>`).join("") : (chunks[0] ?? "");
}

/**
 * Symptom: sub-questions run together on one line while the PDF set them in
 * three columns.
 *
 * `questions` is the exercise charte's own list — `a)`, `b)`, `c)` in the
 * number of columns its option gives.
 */
const lists: Pass = (html) =>
  html.replace(
    /\\begin\{(itemize|enumerate|questions)\}(?:\[(\d)\])?([\s\S]*?)\\end\{\1\}/g,
    (_, kind: string, columns: string | undefined, body: string) => {
      const tag = kind === "itemize" ? "ul" : "ol";
      const alpha = kind === "questions" ? " tex-list--alpha" : "";
      const style = columns && Number(columns) > 1 ? ` style="columns:${columns}"` : "";
      const items = body
        .split(/\\item\s*/)
        .slice(1)
        .map((item) => `<li>${itemParagraphs(item)}</li>`)
        .join("");
      return `<${tag} class="tex-list${alpha}"${style}>${items}</${tag}>`;
    },
  );

const centred: Pass = (html) =>
  html.replace(/\\begin\{center\}([\s\S]*?)\\end\{center\}/g, '<div class="tex-center">$1</div>');

/**
 * Symptom: a literal "[4pt]" printed where a line broke.
 *
 * `\\` ends the line, `\\[4pt]` also asks for leading under it. A TeX
 * dimension always opens on a digit or a sign, which is what tells one from
 * prose that merely starts on a bracket — an interval left in `[0;1]` keeps its
 * brackets. The gap is honoured when CSS shares the unit and dropped
 * otherwise, the way `\vspace` is: page layout belongs to the charte, the
 * break does not.
 */
const lineBreaks: Pass = (html) =>
  html.replace(/\\\\(?:\[(-?[\d.][^\]\n]*)\])?/g, (_, gap?: string) =>
    gap && SAFE_LENGTH.test(gap.trim())
      ? `<span class="tex-break" style="height:${gap.trim()}"></span>`
      : "<br>",
  );

/** Wraps what is not already a block in `<p>`. */
const paragraphs = (html: string) =>
  splitParagraphs(html)
    .map((chunk) => (/^<(ul|ol|div)/.test(chunk) ? chunk : `<p>${chunk}</p>`))
    .join("");

// ---- The pipeline.

/** Before escaping: everything that must survive as HTML is parked. */
const PARKING: Pass[] = [parkFigures, parkDisplayMaths, parkInlineMaths, parkMathsEnvironments];

/** After escaping, before commands: the structure read off `\begin{…}`. */
const SHAPING: Pass[] = [lists, centred, lineBreaks];

const run = (passes: Pass[], text: string, context: Context) =>
  passes.reduce((current, pass) => pass(current, context), text);

export function latexToHtml(latex: string, colours: Colours = {}): string {
  const slots = new Slots();
  const context: Context = { colours, keep: (html) => slots.park(html) };

  const parked = run(PARKING, stripComments(latex, context), context);
  const shaped = run(SHAPING, escapeHtml(parked), context);
  const resolved = applyCommands(shaped, colours);

  return slots.restore(paragraphs(resolved));
}
