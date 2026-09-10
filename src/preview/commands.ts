import { SAFE_COLOUR, SAFE_LENGTH } from "./html";

/**
 * LaTeX commands in prose: `\textbf{…}`, `\ldots`, `\vspace{2cm}`.
 *
 * One table, one scanner. Every command the converter knows is an entry in
 * `COMMANDS`, a name paired with what it becomes; a command with a story gets
 * that story on its handler. The scanner reads arguments at their real brace
 * boundaries, because flat patterns break the moment the model nests them —
 * `\mcul{mcProp}{\textbf{Relation de Chasles :}}` — and leak stray braces.
 *
 * To teach the converter a command, add a line to the table. Nothing else.
 */

/** Colours named by the charte, `mcProp` -> `#117A65`. */
export type Colours = Record<string, string>;

/** Converts nested LaTeX, so a handler can resolve its own arguments. */
type Resolve = (latex: string) => string;

/** What a command becomes, given its brace arguments in order. */
type Handler = (args: string[], resolve: Resolve, colours: Colours) => string;

/** The command's last argument, resolved — or nothing when it had none. */
const lastArgument: Handler = (args, resolve) =>
  args.length > 0 ? resolve(args[args.length - 1]) : "";

/**
 * `\textbf{x}` -> `<strong>x</strong>`. Without an argument, nothing: a bare
 * `\textbf` in the source was a slip, and the tag alone would show as one.
 */
const wrap =
  (tag: string): Handler =>
  ([content], resolve) =>
    content === undefined ? "" : `<${tag}>${resolve(content)}</${tag}>`;

/**
 * The charte's own coloured underline: `\mcul{mcProp}{text}`.
 *
 * The colour is looked up by the name the charte gave it; an unknown name
 * underlines without colouring, which is what the PDF would do too.
 */
const colouredUnderline: Handler = ([name, content], resolve, colours) => {
  if (content === undefined) return name === undefined ? "" : resolve(name);
  const colour = colours[name];
  const style = colour && SAFE_COLOUR.test(colour) ? ` style="color:${colour}"` : "";
  return `<u${style}>${resolve(content)}</u>`;
};

/**
 * `\rule{0.4pt}{6cm}`, drawn rather than dropped: the preview should show the
 * same separator the PDF will, even when the layout around it cannot be
 * reproduced. A length CSS cannot share leaves nothing.
 */
const drawRule: Handler = ([width = "", height = ""]) =>
  SAFE_LENGTH.test(width.trim()) && SAFE_LENGTH.test(height.trim())
    ? `<span class="tex-rule" style="width:${width.trim()};height:${height.trim()}"></span>`
    : "";

/** A text-mode command worth keeping as a character. */
const symbol =
  (character: string): Handler =>
  () =>
    character;

/**
 * A command carrying no content, dropped along with its arguments. Spacing and
 * struts are page layout, which belongs to the charte rather than to a block.
 */
const drop: Handler = () => "";

const table = (names: string[], handler: Handler): Record<string, Handler> =>
  Object.fromEntries(names.map((name) => [name, handler]));

const COMMANDS: Record<string, Handler> = {
  textbf: wrap("strong"),
  emph: wrap("em"),
  textit: wrap("em"),
  underline: wrap("u"),
  mcul: colouredUnderline,
  rule: drawRule,
  ldots: symbol("…"),
  dots: symbol("…"),
  cdots: symbol("⋯"),
  times: symbol("×"),
  circ: symbol("°"),
  degree: symbol("°"),
  euro: symbol("€"),
  bullet: symbol("•"),
  ...table(
    [
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
      "textwidth",
      "linewidth",
      "columnwidth",
      "quad",
      "qquad",
    ],
    drop,
  ),
};

/**
 * An unknown command hands back its last argument — right far more often than
 * wrong, `\text{foo}` keeping `foo` — while a bare unknown command disappears
 * rather than reaching the reader as source code.
 */
const unknownCommand: Handler = lastArgument;

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
 * Reads the arguments following a command: optional `[…]` ones are consumed
 * and forgotten, brace groups are collected in order.
 */
function readArguments(text: string, from: number): { args: string[]; end: number } {
  const args: string[] = [];
  let cursor = from;

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
    args.push(group.body);
    cursor = group.end;
  }

  return { args, end: cursor };
}

/** Walks the text, resolving every command at its real argument boundaries. */
export function applyCommands(text: string, colours: Colours): string {
  const resolve: Resolve = (latex) => applyCommands(latex, colours);
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

    const { args, end } = readArguments(text, index + name[0].length);
    const handler = COMMANDS[name[1]] ?? unknownCommand;
    out += handler(args, resolve, colours);
    index = end;
  }

  return out;
}
