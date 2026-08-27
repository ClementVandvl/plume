/**
 * Guardrail for the token system.
 *
 * Three rules:
 *  1. every `var(--x)` in use must be defined in tokens.css — an unresolved
 *     var() silently voids the WHOLE declaration containing it (a padding that
 *     vanishes, not an error);
 *  2. no component reads the primitive layer `--p-*`, otherwise a visual
 *     overhaul would have to go through the components again;
 *  3. every className a component uses has a rule somewhere. Deleting a CSS
 *     section is silent otherwise — the element simply loses its layout and
 *     nobody finds out until it looks wrong on screen.
 *
 * Paths are compared with forward slashes throughout. `join` yields backslashes
 * on Windows, so comparing against a hard-coded "src/styles/tokens.css" let the
 * token file audit itself in CI and report all 55 of its own primitives.
 */
import { readFileSync, readdirSync } from "node:fs";
import { join, extname } from "node:path";

const slashed = (path) => path.split(/[\\/]/).join("/");

const STYLES = "src/styles/tokens.css";

function readTokens() {
  try {
    return readFileSync(STYLES, "utf8");
  } catch {
    console.error(`Style problems:\n  ✗ ${STYLES} is missing — run from the project root.`);
    process.exit(1);
  }
}

const defined = new Set(
  [...readTokens().matchAll(/^\s*(--[\w-]+)\s*:/gm)].map((m) => m[1]),
);

function filesWith(dir, extension) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return filesWith(path, extension);
    return extname(entry.name) === extension ? [slashed(path)] : [];
  });
}

const allCss = filesWith("src", ".css");

// The token file defines the primitives; auditing it against rule 2 would flag
// every one of them. Fail loudly if it stops being found, rather than drowning
// the output in false positives again.
if (!allCss.includes(STYLES)) {
  console.error(`Style problems:\n  ✗ ${STYLES} was not found — the exclusion is stale.`);
  process.exit(1);
}
const componentCss = allCss.filter((file) => file !== STYLES);

const problems = [];
for (const file of componentCss) {
  const source = readFileSync(file, "utf8");
  for (const [, name] of source.matchAll(/var\((--[\w-]+)/g)) {
    if (!defined.has(name)) problems.push(`${file} : ${name} is not defined anywhere`);
    else if (name.startsWith("--p-")) problems.push(`${file} : ${name} is a primitive`);
  }
}

const declared = new Set(
  componentCss
    // An element-qualified selector declares the class too: `textarea.code-area`
    // exists precisely to outrank `textarea.input`, and reading only bare `.x`
    // selectors reported it as an orphan.
    .flatMap((file) => [
      ...readFileSync(file, "utf8").matchAll(/^(?:[a-zA-Z][a-zA-Z0-9]*)?\.([a-zA-Z0-9_-]+)/gm),
    ])
    .map((match) => match[1]),
);

/**
 * The static string literals inside a JSX expression.
 *
 * Template literals are walked rather than pattern-matched: `${...}` spans are
 * skipped (and mined for literals of their own, so a ternary's branches still
 * count), leaving only text the author actually wrote. A crude regex here read
 * `${consoleOpen ? "` as a literal and invented a `.consoleOpen` class.
 */
function literalsIn(source) {
  const out = [];
  let i = 0;
  while (i < source.length) {
    const ch = source[i];

    if (ch === '"' || ch === "'") {
      const end = source.indexOf(ch, i + 1);
      if (end === -1) break;
      // The operand of a comparison is a value being tested, not a class:
      // `state.kind === "available"` names a state, and reporting `.available`
      // as a missing rule is noise that hides the real orphans.
      const before = source.slice(0, i).trimEnd();
      if (!before.endsWith("=")) out.push(source.slice(i + 1, end));
      i = end + 1;
      continue;
    }

    if (ch === "`") {
      i += 1;
      let text = "";
      while (i < source.length && source[i] !== "`") {
        if (source[i] === "$" && source[i + 1] === "{") {
          const start = i + 1;
          let depth = 0;
          for (; i < source.length; i += 1) {
            if (source[i] === "{") depth += 1;
            else if (source[i] === "}") {
              depth -= 1;
              if (depth === 0) {
                i += 1;
                break;
              }
            }
          }
          out.push(...literalsIn(source.slice(start + 1, i - 1)));
          // A space, so `scan__row--${state}` does not fuse with what follows.
          text += " ";
        } else {
          text += source[i];
          i += 1;
        }
      }
      out.push(text);
      i += 1;
      continue;
    }

    i += 1;
  }
  return out;
}

/**
 * Every class named in a `className`, including inside template literals.
 *
 * Reading only up to the first brace missed `className={`deep ${x}`}`
 * entirely, which is how a console panel shipped with no height rule at all:
 * the class was there, the rule was not, and nothing complained.
 */
function classNamesIn(source) {
  const names = [];
  const marker = "className=";
  for (let at = source.indexOf(marker); at !== -1; at = source.indexOf(marker, at + 1)) {
    let i = at + marker.length;
    let literals;

    if (source[i] === '"' || source[i] === "'") {
      const end = source.indexOf(source[i], i + 1);
      if (end === -1) continue;
      literals = [source.slice(i + 1, end)];
    } else if (source[i] === "{") {
      let depth = 0;
      const start = i;
      for (; i < source.length; i += 1) {
        if (source[i] === "{") depth += 1;
        else if (source[i] === "}") {
          depth -= 1;
          if (depth === 0) break;
        }
      }
      literals = literalsIn(source.slice(start + 1, i));
    } else {
      continue;
    }

    for (const literal of literals) {
      for (const token of literal.split(/\s+/)) {
        // A name left open by an interpolation -- `scan__row--` -- cannot be
        // checked, so it is not claimed to be missing either.
        if (token && !token.endsWith("-")) names.push(token);
      }
    }
  }
  return names;
}

const used = new Set();
for (const file of filesWith("src", ".tsx")) {
  const source = readFileSync(file, "utf8");
  for (const name of classNamesIn(source)) {
    if (/^[a-zA-Z][a-zA-Z0-9_-]*$/.test(name)) used.add(name);
  }
}

for (const name of [...used].sort()) {
  if (!declared.has(name) && !name.startsWith("katex")) {
    problems.push(`class .${name} is used but has no rule`);
  }
}

if (problems.length) {
  console.error("Style problems:");
  for (const problem of [...new Set(problems)]) console.error("  ✗ " + problem);
  process.exit(1);
}

console.log(
  `✓ styles — ${defined.size} tokens, ${declared.size} classes, no leaks, no orphans`,
);
