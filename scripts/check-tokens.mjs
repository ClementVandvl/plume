/**
 * Guardrail for the token system.
 *
 * Two rules:
 *  1. every `var(--x)` in use must be defined in tokens.css — an unresolved
 *     var() silently voids the WHOLE declaration containing it (a padding that
 *     vanishes, not an error);
 *  2. no component reads the primitive layer `--p-*`, otherwise a visual
 *     overhaul would have to go through the components again;
 *  3. every className a component uses has a rule somewhere. Deleting a CSS
 *     section is silent otherwise — the element simply loses its layout and
 *     nobody finds out until it looks wrong on screen.
 */
import { readFileSync, readdirSync } from "node:fs";
import { join, extname } from "node:path";

const STYLES = "src/styles/tokens.css";
const defined = new Set(
  [...readFileSync(STYLES, "utf8").matchAll(/^\s*(--[\w-]+)\s*:/gm)].map((m) => m[1]),
);

function cssFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const path = join(dir, e.name);
    if (e.isDirectory()) return cssFiles(path);
    return extname(e.name) === ".css" && path !== STYLES ? [path] : [];
  });
}

const problems = [];
for (const file of cssFiles("src")) {
  const source = readFileSync(file, "utf8");
  for (const [, name] of source.matchAll(/var\((--[\w-]+)/g)) {
    if (!defined.has(name)) problems.push(`${file} : ${name} is not defined anywhere`);
    else if (name.startsWith("--p-")) problems.push(`${file} : ${name} is a primitive`);
  }
}

// Rule 3: className without a matching rule.
const declared = new Set(
  cssFiles("src")
    .flatMap((file) => [...readFileSync(file, "utf8").matchAll(/^\.([a-zA-Z0-9_-]+)/gm)])
    .map((match) => match[1]),
);

function tsxFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const path = join(dir, e.name);
    if (e.isDirectory()) return tsxFiles(path);
    return extname(e.name) === ".tsx" ? [path] : [];
  });
}

const used = new Set();
for (const file of tsxFiles("src")) {
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
  for (const p of [...new Set(problems)]) console.error("  ✗ " + p);
  process.exit(1);
}
console.log(
  `✓ styles — ${defined.size} tokens, ${declared.size} classes, no leaks, no orphans`,
);
