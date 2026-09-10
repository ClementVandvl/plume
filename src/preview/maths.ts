import katex from "katex";
import { escapeHtml } from "./html";

/**
 * Maths, typeset by KaTeX — and the two places KaTeX and LaTeX disagree.
 */

/**
 * Symptom: a literal "[t]" printed at the start of a calculation.
 *
 * `\begin{aligned}[t]` is how a continued calculation stays on the baseline of
 * the line that introduces it. KaTeX does not implement the option and typesets
 * it as text, so it is dropped for the preview only. The exported LaTeX keeps
 * it, and `hangFromFirstRow` puts back what it bought.
 */
function forKatex(source: string): string {
  return source.replace(/(\\begin\{(?:aligned|gathered|alignedat)\})\[[tbc]\]/g, "$1");
}

/** A calculation written as `\begin{aligned}[t]`, and nothing before it. */
const TOP_ALIGNED = /^\\begin\{(?:aligned|gathered|alignedat)\}\[t\]/;

/**
 * Symptom: the list number of an item sits halfway down its calculation.
 *
 * KaTeX centres an alignment block on the maths axis, where LaTeX's `[t]` hangs
 * it from its first row. The rows carry their own geometry: each sits at
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

/** Typesets `source`; on a KaTeX failure the source is shown, marked as raw. */
export function renderMaths(source: string, display: boolean): string {
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
