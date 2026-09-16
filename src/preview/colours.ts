import type { Colours } from "./commands";
import type { Template } from "../types";

/**
 * The colours a charte defines, under the names its LaTeX uses.
 *
 * `\mcul{mcProp}{...}` names a colour the way the preamble does; the template
 * stores the same colour under the semantic key `color.property`. Anything
 * converting a passage needs the bridge between the two, and there is more
 * than one such place now — the review preview and the adaptation page — so
 * it lives here rather than inside whichever component needed it first.
 */
const LATEX_NAMES: Record<string, string> = {
  mcChapitre: "chapter",
  mcPartie: "part",
  mcSousPartie: "subpart",
  mcParagraphe: "paragraph",
  mcDef: "definition",
  mcVocab: "vocabulary",
  mcProp: "property",
  mcTheo: "theorem",
  mcMethode: "method",
  mcExemple: "example",
  mcApp: "application",
  mcRemarque: "remark",
  mcDemo: "proof",
  mcTexte: "body",
};

/** Every `color.*` value the template holds, by its semantic key. */
export function semanticColours(template: Template | undefined): Record<string, string> {
  const map: Record<string, string> = {};
  for (const key of template?.keys ?? []) {
    if (key.key.startsWith("color.")) map[key.key.slice("color.".length)] = key.value;
  }
  return map;
}

/** Every `label.*` value the template holds, by its semantic key. */
export function semanticLabels(template: Template | undefined): Record<string, string> {
  const map: Record<string, string> = {};
  for (const key of template?.keys ?? []) {
    if (key.key.startsWith("label.")) map[key.key.slice("label.".length)] = key.value;
  }
  return map;
}

/** The same colours under the names the charte's LaTeX calls them by. */
export function latexColours(template: Template | undefined): Colours {
  const semantic = semanticColours(template);
  const map: Colours = {};
  for (const [name, key] of Object.entries(LATEX_NAMES)) {
    if (semantic[key]) map[name] = semantic[key];
  }
  return map;
}
