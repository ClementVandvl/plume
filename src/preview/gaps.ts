/**
 * The words a teacher has marked to disappear from an adapted copy.
 *
 * A pupil working under a PAP (plan d'accompagnement personnalisé) gets the
 * same lesson with some of its words left blank, to fill in during the hour
 * instead of copying the whole page. The teacher marks those words in Plume,
 * and the mark is written into the passage's own LaTeX as `\trou{les mots}`.
 *
 * In the body rather than in a field beside it: a field would name a span of
 * text by position, and every edit to the passage moves those. Inside, the
 * mark travels with the words it holds. `render::apply_gaps` resolves it at
 * export — printed in the ordinary copy, a ruled space in the adapted one.
 *
 * This module is the round trip between that LaTeX and something a teacher
 * can click: `tokenise` cuts a passage into words, `build` puts it back. Every
 * change goes through both, so what is shown and what is stored cannot drift.
 */

export const GAP = "\\trou";

/**
 * What a piece of a passage is, for the purpose of hiding it.
 *
 * - `word` — a word or a formula: the teacher may hide it.
 * - `separator` — a space or a punctuation mark: not hidden on its own, but
 *   swept into a hole that runs on either side of it.
 * - `fixed` — a diagram, or the scaffolding of a list: never hidden, and it
 *   ends a run rather than being swallowed by one. A hole cannot span a
 *   figure, and `\trou{a \item b}` would not compile.
 */
export type Role = "word" | "separator" | "fixed";

export type Token = {
  /** The source, verbatim. Joined back in order, the tokens are the passage. */
  text: string;
  role: Role;
  /** Whether it sits inside a gap as things stand. */
  gap: boolean;
};

/** Whether the teacher may hide this piece. */
export const hideable = (token: Token) => token.role === "word";

/** Commands that hold the passage together rather than saying something. */
const STRUCTURAL = new Set(["begin", "end", "item"]);

/** Punctuation that reads better left standing beside a hole than inside it. */
const PUNCTUATION = new Set([",", ".", ";", ":", "!", "?", "(", ")", "«", "»", "—", "–"]);

/** A diagram: shown, never hidden — a hole the size of a figure helps nobody. */
const FIGURE = /^\\begin\{tikzpicture\}[\s\S]*?\\end\{tikzpicture\}/;

/** The balanced group `text` opens with, if it opens with one. */
function group(text: string): string | null {
  if (!text.startsWith("{")) return null;
  let depth = 0;
  for (let at = 0; at < text.length; at += 1) {
    const char = text[at];
    if (char === "\\") at += 1;
    else if (char === "{") depth += 1;
    else if (char === "}") {
      depth -= 1;
      if (depth === 0) return text.slice(1, at);
    }
  }
  return null;
}

/** A `$…$` span, closing on the first delimiter outside a brace group. */
function maths(text: string): string | null {
  if (!text.startsWith("$")) return null;
  let depth = 0;
  for (let at = 1; at < text.length; at += 1) {
    const char = text[at];
    if (char === "\\") at += 1;
    else if (char === "{") depth += 1;
    else if (char === "}") depth -= 1;
    else if (char === "$" && depth <= 0) return text.slice(0, at + 1);
  }
  return null;
}

/** A command with its arguments: `\textbf{les deux}`, `\item`, `\vec{u}`. */
function command(text: string): string | null {
  const name = /^\\([a-zA-Z]+\*?|.)/.exec(text);
  if (!name) return null;
  let taken = name[0].length;
  for (;;) {
    const rest = text.slice(taken);
    if (rest.startsWith("[")) {
      const close = rest.indexOf("]");
      if (close === -1) break;
      taken += close + 1;
      continue;
    }
    const body = group(rest);
    if (body === null) break;
    taken += body.length + 2;
  }
  return text.slice(0, taken);
}

/** Cuts a passage into the pieces a teacher points at. */
export function tokenise(latex: string): Token[] {
  const tokens: Token[] = [];
  const push = (text: string, role: Role, gap: boolean) => {
    if (text) tokens.push({ text, role, gap: gap && role === "word" });
  };

  const scan = (source: string, gap: boolean) => {
    let rest = source;
    while (rest.length > 0) {
      // A mark already in place: its contents are tokenised as everything
      // else, and carry the flag that shows them highlighted.
      if (rest.startsWith(GAP)) {
        const inner = group(rest.slice(GAP.length));
        if (inner !== null) {
          scan(inner, true);
          rest = rest.slice(GAP.length + inner.length + 2);
          continue;
        }
      }

      const space = /^\s+/.exec(rest);
      if (space) {
        push(space[0], "separator", false);
        rest = rest.slice(space[0].length);
        continue;
      }

      const figure = FIGURE.exec(rest);
      if (figure) {
        push(figure[0], "fixed", false);
        rest = rest.slice(figure[0].length);
        continue;
      }

      const formula = maths(rest);
      if (formula) {
        push(formula, "word", gap);
        rest = rest.slice(formula.length);
        continue;
      }

      if (rest.startsWith("\\")) {
        const whole = command(rest);
        if (whole) {
          const name = /^\\([a-zA-Z]+)/.exec(whole)?.[1] ?? "";
          push(whole, STRUCTURAL.has(name) ? "fixed" : "word", gap);
          rest = rest.slice(whole.length);
          continue;
        }
      }

      if (PUNCTUATION.has(rest[0])) {
        push(rest[0], "separator", false);
        rest = rest.slice(1);
        continue;
      }

      // A plain word, up to the next space or anything handled above.
      let at = 0;
      while (at < rest.length && !/\s/.test(rest[at]) && !"\\$".includes(rest[at]) && !PUNCTUATION.has(rest[at])) {
        at += 1;
      }
      if (at === 0) at = 1; // a brace of its own, and anything unforeseen
      push(rest.slice(0, at), "word", gap);
      rest = rest.slice(at);
    }
  };

  scan(latex, false);
  return tokens;
}

/**
 * The runs the tokens form: for each token, whether the rebuilt LaTeX puts it
 * inside a `\trou{…}`.
 *
 * Neighbouring holes become one hole, spaces and punctuation between them
 * included: three words marked in a row are one blank the teacher drew in one
 * gesture, not three the renderer would have to guess how to join. A word that
 * is not a hole ends the run, and so does anything fixed — a figure caught
 * inside a `\trou` would vanish from the copy.
 *
 * Shared by `build` and by the page that shows the marking, so the highlight
 * the teacher sees is the hole the PDF will have, down to the space between
 * two marked words.
 */
export function runs(tokens: Token[]): boolean[] {
  const inside = tokens.map(() => false);
  let at = 0;

  while (at < tokens.length) {
    if (!tokens[at].gap) {
      at += 1;
      continue;
    }
    let last = at;
    for (let cursor = at; cursor < tokens.length; cursor += 1) {
      if (tokens[cursor].gap) last = cursor;
      else if (tokens[cursor].role !== "separator") break;
    }
    for (let cursor = at; cursor <= last; cursor += 1) inside[cursor] = true;
    at = last + 1;
  }

  return inside;
}

/** Writes the tokens back as LaTeX, one `\trou{…}` per run. */
export function build(tokens: Token[]): string {
  const inside = runs(tokens);
  let out = "";
  let at = 0;

  while (at < tokens.length) {
    if (!inside[at]) {
      out += tokens[at].text;
      at += 1;
      continue;
    }
    let end = at;
    while (end + 1 < tokens.length && inside[end + 1]) end += 1;
    out += `${GAP}{${tokens.slice(at, end + 1).map((token) => token.text).join("")}}`;
    at = end + 1;
  }

  return out;
}

/**
 * Marks or clears every selectable token between `from` and `to`.
 *
 * Marking is a toggle on the whole stretch: dragging back over words that are
 * already holes clears them, which is how the teacher takes a marking back
 * without a second control to find.
 */
export function mark(tokens: Token[], from: number, to: number): Token[] {
  const [start, end] = from <= to ? [from, to] : [to, from];
  const touched = tokens.slice(start, end + 1).filter(hideable);
  if (touched.length === 0) return tokens;

  const gap = !touched.every((token) => token.gap);
  return tokens.map((token, at) =>
    at >= start && at <= end && hideable(token) ? { ...token, gap } : token,
  );
}

/** How many holes a passage holds — runs, as the teacher drew them. */
export function countGaps(latex: string): number {
  let count = 0;
  let at = latex.indexOf(GAP);
  while (at >= 0) {
    if (group(latex.slice(at + GAP.length)) !== null) count += 1;
    at = latex.indexOf(GAP, at + GAP.length);
  }
  return count;
}

/** The passage with every marking taken off. */
export function clearGaps(latex: string): string {
  return build(tokenise(latex).map((token) => ({ ...token, gap: false })));
}

/** Whether a passage has anything a teacher could hide. */
export const adaptable = (latex: string) => tokenise(latex).some(hideable);
