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
    .flatMap((file) => [...readFileSync(file, "utf8").matchAll(/^\.([a-zA-Z0-9_-]+)/gm)])
    .map((match) => match[1]),
);

const used = new Set();
for (const file of filesWith("src", ".tsx")) {
  const source = readFileSync(file, "utf8");
  for (const [, value] of source.matchAll(/className=[{"`]([^"`}]*)/g)) {
    for (const token of value.split(/[\s${}?:]+/)) {
      const name = token.replace(/["'`]/g, "");
      if (/^[a-zA-Z][a-zA-Z0-9_-]*$/.test(name)) used.add(name);
    }
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
