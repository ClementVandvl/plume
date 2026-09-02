import katex from "katex";

/**
 * Turns a block's LaTeX body into HTML for the review preview.
 *
 * This is an *approximation*, deliberately. The authoritative render is the PDF
 * produced by the real compiler; this exists so the teacher can read their
 * course as a document while reviewing, instead of scanning a list of boxes.
 *
 * Two rules, both earned by bugs:
 *  - every fragment of HTML we inject (maths, figures) is parked in a slot
 *    BEFORE the text is escaped, and restored at the very end. Injecting HTML
 *    earlier gets it escaped and shown to the user as literal markup;
 *  - commands are read with a brace-balancing scanner, not regexes. Flat
 *    patterns break the moment the model nests them — as in
 *    `\mcul{mcProp}{\textbf{Relation de Chasles :}}` — and leak stray braces.
 */

const SLOT = (index: number) => `@@PLUME_${index}@@`;

const escapeHtml = (text: string) =>
  text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Text-mode commands worth keeping as characters. */
const SYMBOLS: Record<string, string> = {
  ldots: "…",
  dots: "…",
  cdots: "⋯",
  times: "×",
  circ: "°",
  degree: "°",
  euro: "€",
  bullet: "•",
};

/**
 * Commands carrying no content: dropped along with their arguments.
 *
 * `\rule` and the spacing commands are page layout, which belongs to the
 * template rather than to a block.
 */
const DROPPED = new Set([
  "begin",
  "end",
  "noindent",
  "par",
  "medskip",
  "smallskip",
  "bigskip",
  "vspace",
  "hspace",
  "hfill",
  "vfill",
  "centering",
  "raggedright",
  "rule",
  "textwidth",
  "linewidth",
  "columnwidth",
  "quad",
  "qquad",
]);

/** `command` -> the tag it becomes, and which argument holds its content. */
const WRAPPERS: Record<string, { tag: string; content: number; colour?: number }> = {
  textbf: { tag: "strong", content: 0 },
  emph: { tag: "em", content: 0 },
  textit: { tag: "em", content: 0 },
  underline: { tag: "u", content: 0 },
  // The template's own coloured underline: \mcul{colour}{text}.
  mcul: { tag: "u", content: 1, colour: 0 },
};

const SAFE_COLOUR = /^#[0-9a-fA-F]{3,8}$/;

/** Lengths TeX and CSS agree on, so a rule can be drawn as-is. */
const SAFE_LENGTH = /^-?\d*\.?\d+(pt|mm|cm|in|em|ex|px)$/;

const ESCAPED_PERCENT = "@@PLUME_PCT@@";

/**
 * Removes LaTeX comments.
 *
 * An unescaped `%` comments out the rest of its line, which is how authors glue
 * two boxes together without a space — `\end{minipage}%`. Left in, the percent
 * signs surface in the prose as stray characters.
 */
function stripComments(text: string): string {
  return text
    .replace(/\\%/g, ESCAPED_PERCENT)
    .replace(/%[^\n]*/g, "")
    .replace(new RegExp(ESCAPED_PERCENT, "g"), "\\%");
}

/**
 * Smooths over what KaTeX does not implement.
 *
 * `\begin{aligned}[t]` is how a continued calculation stays on the baseline of
 * the line that introduces it — without it, LaTeX centres the block vertically
 * and the list number floats to its middle. KaTeX ignores the option and
 * typesets a literal "[t]" instead, so it is dropped for the preview only. The
 * exported LaTeX keeps it, and `hangFromFirstRow` puts back what it bought.
 */
function forKatex(source: string): string {
  return source.replace(/(\\begin\{(?:aligned|gathered|alignedat)\})\[[tbc]\]/g, "$1");
}

/** A calculation written as `\begin{aligned}[t]`, and nothing before it. */
const TOP_ALIGNED = /^\\begin\{(?:aligned|gathered|alignedat)\}\[t\]/;

/**
 * Re-applies the `[t]` that `forKatex` had to drop, on the rendered HTML.
 *
 * KaTeX centres an alignment block on the maths axis, so the `<li>` marker of
 * the item introducing it lands halfway down the calculation rather than beside
 * its first line. The rows carry their own geometry: each sits at
 * `top: -(pstrut + baseline)`, so the first row's baseline is `|top| - pstrut`
 * above the block's own. Lowering the whole box by that much is exactly what
 * `[t]` does — and the em here is KaTeX's own, since `.katex-base` inherits the
 * font size the offsets were measured in.
 */
function hangFromFirstRow(html: string): string {
  const row = /<span style="top:(-[\d.]+)em;">/.exec(html);
  const strut = /<span class="pstrut" style="height:([\d.]+)em;">/.exec(html);
  if (!row || !strut) return html;

  const shift = -Number(row[1]) - Number(strut[1]);
  if (!(shift > 0)) return html;

  return html.replace(
    '<span class="katex-base">',
    `<span class="katex-base" style="vertical-align:-${shift.toFixed(4)}em;">`,
  );
}

function math(source: string, display: boolean): string {
  try {
    const html = katex.renderToString(forKatex(source), {
      displayMode: display,
      throwOnError: false,
      strict: false,
    });
    // Only inline: a displayed block owns its line, so it has no marker to miss.
    return !display && TOP_ALIGNED.test(source.trim()) ? hangFromFirstRow(html) : html;
  } catch {
    return `<code class="tex-raw">${escapeHtml(source)}</code>`;
  }
}

/** Reads the balanced group starting at `open`, which must be a `{`. */
function readGroup(text: string, open: number): { body: string; end: number } | null {
  if (text[open] !== "{") return null;
  let depth = 0;
  for (let index = open; index < text.length; index += 1) {
    if (text[index] === "{") depth += 1;
    else if (text[index] === "}") {
      depth -= 1;
      if (depth === 0) return { body: text.slice(open + 1, index), end: index + 1 };
    }
  }
  return null;
}

/**
 * Walks the text, resolving commands at their real argument boundaries.
 *
 * An unknown command hands back its last argument — right far more often than
 * wrong, `\text{foo}` keeping `foo` — while a bare unknown command disappears
 * rather than reaching the reader as source code.
 */
function applyCommands(text: string, colours: Record<string, string>): string {
  let out = "";
  let index = 0;

  while (index < text.length) {
    if (text[index] !== "\\") {
      out += text[index];
      index += 1;
      continue;
    }

    const name = /^\\([a-zA-Z]+)\*?/.exec(text.slice(index));
    if (!name) {
      // An escaped character: \% \& \_ and friends.
      out += text[index + 1] ?? "";
      index += 2;
      continue;
    }

    let cursor = index + name[0].length;
    const groups: string[] = [];

    for (;;) {
      let at = cursor;
      while (text[at] === " " || text[at] === "\n") at += 1;

      if (text[at] === "[") {
        const close = text.indexOf("]", at);
        if (close === -1) break;
        cursor = close + 1;
        continue;
      }

      const group = readGroup(text, at);
      if (!group) break;
      groups.push(group.body);
      cursor = group.end;
    }

    const command = name[1];
    const wrapper = WRAPPERS[command];

    if (command === "rule" && groups.length >= 2) {
      // Drawn rather than dropped: the preview should show the same separator
      // the PDF will, even when the surrounding layout cannot be reproduced.
      const [width, height] = groups;
      out +=
        SAFE_LENGTH.test(width.trim()) && SAFE_LENGTH.test(height.trim())
          ? `<span class="tex-rule" style="width:${width.trim()};height:${height.trim()}"></span>`
          : "";
    } else if (DROPPED.has(command)) {
      // Nothing emitted; the arguments were consumed above.
    } else if (wrapper && groups.length > wrapper.content) {
      const inner = applyCommands(groups[wrapper.content], colours);
      const named = wrapper.colour !== undefined ? groups[wrapper.colour] : undefined;
      const colour = named ? colours[named] : undefined;
      const style = colour && SAFE_COLOUR.test(colour) ? ` style="color:${colour}"` : "";
      out += `<${wrapper.tag}${style}>${inner}</${wrapper.tag}>`;
    } else if (SYMBOLS[command] !== undefined) {
      out += SYMBOLS[command];
    } else if (groups.length > 0) {
      out += applyCommands(groups[groups.length - 1], colours);
    }

    index = cursor;
  }

  return out;
}

export function latexToHtml(latex: string, colours: Record<string, string> = {}): string {
  const slots: string[] = [];
  const keep = (html: string) => {
    slots.push(html);
    return SLOT(slots.length - 1);
  };

  // 1. Park everything that must survive as HTML.
  let text = stripComments(latex).replace(
    /\\begin\{tikzpicture\}[\s\S]*?\\end\{tikzpicture\}/g,
    () => keep('<span class="tex-figure">Schéma — visible dans le PDF</span>'),
  );
  text = text.replace(/\\\[([\s\S]*?)\\\]/g, (_, body) => keep(math(body, true)));
  text = text.replace(/\$\$([\s\S]*?)\$\$/g, (_, body) => keep(math(body, true)));
  text = text.replace(/\$([^$]+)\$/g, (_, body) => keep(math(body, false)));

  // A maths environment written on its own, outside any `$` or `\[`. The model
  // emits these for a multi-line calculation, and without this they fell through
  // to the prose branch: the alignment tabs showed up as a literal "&=" and
  // `\frac{3}{5}` was eaten down to "5". Run after the delimiters above, so only
  // genuinely top-level environments are left to match.
  text = text.replace(
    /\\begin\{(align\*?|alignat\*?|gather\*?|gathered|aligned|equation\*?|multline\*?|flalign\*?|split|cases)\}[\s\S]*?\\end\{\1\}/g,
    (whole) => keep(math(whole, true)),
  );

  // 2. Escape whatever is left: it is the author's prose.
  let html = escapeHtml(text);

  // 3. Structure, before commands are resolved — these read `\begin{...}`.
  html = html.replace(
    /\\begin\{(itemize|enumerate)\}([\s\S]*?)\\end\{\1\}/g,
    (_, kind: string, body: string) => {
      const tag = kind === "itemize" ? "ul" : "ol";
      const items = body
        .split(/\\item\s*/)
        .slice(1)
        .map((item) => `<li>${item.trim()}</li>`)
        .join("");
      return `<${tag} class="tex-list">${items}</${tag}>`;
    },
  );
  html = html.replace(
    /\\begin\{center\}([\s\S]*?)\\end\{center\}/g,
    '<div class="tex-center">$1</div>',
  );
  html = html.replace(/\\\\/g, "<br>");

  // 4. Commands, at their real argument boundaries.
  html = applyCommands(html, colours);

  // 5. Paragraphs, then put the parked HTML back.
  html = html
    .split(/\n\s*\n/)
    .map((chunk) => chunk.trim())
    .filter(Boolean)
    .map((chunk) => (/^<(ul|ol|div)/.test(chunk) ? chunk : `<p>${chunk}</p>`))
    .join("");

  return html.replace(/@@PLUME_(\d+)@@/g, (_, index) => slots[Number(index)] ?? "");
}

export const hasFigure = (latex: string) => /\\begin\{tikzpicture\}/.test(latex);

/**
 * Page-layout constructs the recogniser is told not to emit.
 *
 * The preview stacks content in one column, so a block using them will not look
 * the same in the PDF — which is exactly worth saying out loud.
 */
/** Environments where `&` is an alignment tab rather than a mistake. */
const ALIGNING =
  "align\\*?|aligned|alignat\\*?|gather\\*?|gathered|split|cases|array|" +
  "[pbvV]?matrix|smallmatrix|tabularx?|flalign\\*?|multline\\*?|eqnarray\\*?";

/**
 * An `&` outside any environment that gives it a meaning.
 *
 * Mirrors `render::has_stray_alignment`. The model reaches for alignment tabs on
 * its own when a calculation runs over several lines; emitted bare they are a
 * LaTeX error and the preview shows them as literal text. Flagged rather than
 * repaired: the block belongs to the teacher, and the correction pass is where
 * it gets fixed.
 */
export const hasStrayAlignment = (latex: string) =>
  latex
    .replace(/\\&/g, "")
    .replace(
      new RegExp(`\\\\begin\\{(${ALIGNING})\\}[\\s\\S]*?\\\\end\\{\\1\\}`, "g"),
      "",
    )
    .includes("&");

/**
 * Whether a passage lays itself out, in ways a single column cannot reproduce.
 *
 * A fixed list of constructs, not a comparison of the two renderings: telling
 * "this will look different" in general would mean typesetting the page twice
 * and diffing it. So it flags what is known to place things — columns, struts,
 * page breaks — and stays quiet about anything else.
 *
 * Kept identical to `render::LAYOUT_COMMANDS`, which writes the console
 * warning: the two disagreed once, and a passage was reported in one place and
 * silent in the other.
 */
export const hasLayout = (latex: string) =>
  /\\begin\{(minipage|multicols|tabular)\}|\\rule\{|\\hfill|\\newpage|\\vspace/.test(
    latex,
  );

export type Segment = { kind: "text"; latex: string } | { kind: "figure"; tikz: string };

/**
 * Splits a block around its diagrams so the preview can render each one with
 * the real LaTeX engine instead of a placeholder.
 */
export function splitFigures(latex: string): Segment[] {
  const pattern = /\\begin\{tikzpicture\}[\s\S]*?\\end\{tikzpicture\}/g;
  const segments: Segment[] = [];
  let cursor = 0;

  for (const match of latex.matchAll(pattern)) {
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
