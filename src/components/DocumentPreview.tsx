import { useMemo } from "react";
import {
  hasLayout,
  hasStrayAlignment,
  latexToHtml,
  splitFigures,
} from "../preview/latexToHtml";
import { Figure } from "./Figure";
import { DOUBT_THRESHOLD, KIND_LABEL, type Block, type Template, type Transcript } from "../types";

/**
 * The course as a document, not as a list of boxes.
 *
 * Every block is a real element here, so hovering and selecting come for free —
 * which is the point: the teacher reads the course, and the blocks that need
 * attention stand out inside it rather than beside it.
 */

type Props = {
  documentId: string;
  transcript: Transcript;
  /** `all` | `doubt` | `teacher` | `student` */
  filter: string;
  template: Template | undefined;
  selectedId: string | null;
  onSelect: (blockId: string) => void;
};

const ROMAN = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
const LETTERS = "abcdefghijklmnopqrstuvwxyz";

/** LaTeX colour names used by the template, mapped to their semantic key. */
const LATEX_COLOURS: Record<string, string> = {
  mcChapitre: "chapter",
  mcPartie: "part",
  mcSousPartie: "subpart",
  mcParagraphe: "paragraph",
  mcDef: "definition",
  mcProp: "property",
  mcTheo: "theorem",
  mcMethode: "method",
  mcExemple: "example",
  mcApp: "application",
  mcRemarque: "remark",
  mcDemo: "proof",
  mcTexte: "body",
};

const BRACKETED = new Set(["definition", "property", "theorem", "method"]);
const LABELLED = new Set([
  "definition",
  "property",
  "theorem",
  "method",
  "example",
  "application",
  "remark",
  "proof",
]);

export function DocumentPreview({
  documentId,
  transcript,
  filter,
  template,
  selectedId,
  onSelect,
}: Props) {
  const colours = useMemo(() => {
    const map: Record<string, string> = {};
    for (const key of template?.keys ?? []) {
      if (key.key.startsWith("color.")) map[key.key.slice("color.".length)] = key.value;
    }
    return map;
  }, [template]);

  const labels = useMemo(() => {
    const map: Record<string, string> = {};
    for (const key of template?.keys ?? []) {
      if (key.key.startsWith("label.")) map[key.key.slice("label.".length)] = key.value;
    }
    return map;
  }, [template]);

  // `\mcul{mcProp}{...}` needs the real colour behind the LaTeX name.
  const latexColours = useMemo(() => {
    const map: Record<string, string> = {};
    for (const [name, key] of Object.entries(LATEX_COLOURS)) {
      if (colours[key]) map[name] = colours[key];
    }
    return map;
  }, [colours]);

  // Headings are renumbered exactly as the template does, so what the teacher
  // reads here matches the PDF rather than the handwritten numbering.
  const numbered = useMemo(() => {
    let chapter = 0;
    let part = 0;
    let subpart = 0;
    let paragraph = 0;
    const out: { block: Block; page: number; number: string | null }[] = [];

    for (const page of transcript.pages) {
      for (const block of page.blocks) {
        let number: string | null = null;
        if (block.kind === "chapter") {
          chapter += 1;
          number = String(chapter);
        } else if (block.kind === "part") {
          part += 1;
          subpart = 0;
          paragraph = 0;
          number = ROMAN[part - 1] ?? String(part);
        } else if (block.kind === "subpart") {
          subpart += 1;
          paragraph = 0;
          number = String(subpart);
        } else if (block.kind === "paragraph") {
          paragraph += 1;
          number = LETTERS[paragraph - 1] ?? String(paragraph);
        }
        out.push({ block, page: page.number, number });
      }
    }
    return out;
  }, [transcript]);

  const kindColour = (kind: string) => {
    const map: Record<string, string> = {
      chapter: "chapter",
      part: "part",
      subpart: "subpart",
      paragraph: "paragraph",
      definition: "definition",
      property: "property",
      theorem: "theorem",
      method: "method",
      example: "example",
      application: "application",
      remark: "remark",
      proof: "proof",
    };
    return colours[map[kind] ?? "body"] ?? colours.body ?? "#1A1A1A";
  };

  return (
    <div className="paper">
      {numbered
        .filter(({ block }) => {
          if (filter === "doubt")
            return block.confidence < DOUBT_THRESHOLD && !block.reviewed;
          if (filter === "teacher")
            return block.audience.length > 0 && !block.audience.includes("student");
          if (filter === "student")
            return block.audience.length > 0 && !block.audience.includes("teacher");
          return true;
        })
        .map(({ block, page, number }) => {
        const flagged = block.confidence < DOUBT_THRESHOLD && !block.reviewed;
        const colour = kindColour(block.kind);
        const label = labels[block.kind];
        const teacherOnly =
          block.audience.length > 0 && !block.audience.includes("student");
        const studentOnly =
          block.audience.length > 0 && !block.audience.includes("teacher");
        // The preview is a single column; a block doing its own layout will not
        // look the same in the PDF, and saying so beats a silent difference.
        const layout = hasLayout(block.latex);
        // An alignment tab with no environment around it is a LaTeX error, so
        // the export would fail outright. Saying so here is the only chance to
        // fix it before that.
        const strayTab = hasStrayAlignment(block.latex);

        return (
          <div
            key={block.id}
            className={`pblock ${flagged ? "pblock--flagged" : ""} ${
              block.note ? "pblock--noted" : ""
            } ${teacherOnly ? "pblock--teacher" : ""} ${
              studentOnly ? "pblock--student" : ""
            } ${layout || strayTab ? "pblock--layout" : ""} ${
              selectedId === block.id ? "pblock--selected" : ""
            }`}
            onClick={() => onSelect(block.id)}
            role="button"
            tabIndex={0}
            onKeyDown={(event) => event.key === "Enter" && onSelect(block.id)}
          >
            <span className="pblock__tag">
              <span className="pblock__tag-kind">{KIND_LABEL[block.kind] ?? block.kind}</span>
              <span className="pblock__tag-conf">
                {Math.round(block.confidence * 100)} %
              </span>
              <span className="pblock__tag-page">p.{page}</span>
              {block.note && <span className="pblock__tag-note">annoté</span>}
              {teacherOnly && <span className="pblock__tag-note">version prof</span>}
              {studentOnly && <span className="pblock__tag-note">version élève</span>}
              {layout && <span className="pblock__tag-note">mise en page</span>}
            </span>

            {teacherOnly && <span className="pblock__aside">prof</span>}
            {studentOnly && (
              <span className="pblock__aside pblock__aside--student">élève</span>
            )}

            <div className="pblock__body">
              {block.kind === "chapter" ? (
                <h1 className="tex-chapter" style={{ color: colour, borderColor: colour }}>
                  {/* Only the label is underlined, as `\chapitre` does. */}
                  <span className="tex-chapter__label">{`Chapitre ${number} :`}</span>{" "}
                  {block.title || block.latex}
                </h1>
              ) : block.kind === "part" || block.kind === "subpart" || block.kind === "paragraph" ? (
                <p
                  className={`tex-heading tex-heading--${block.kind}`}
                  style={{ color: colour, borderColor: colour }}
                >
                  {block.kind === "paragraph"
                    ? `${number}) ${block.title || block.latex}`
                    : `${number} – ${block.title || block.latex}`}
                </p>
              ) : (
                <div
                  className={`tex-env ${BRACKETED.has(block.kind) ? "tex-env--bracket" : ""}`}
                  style={{ borderColor: colour }}
                >
                  {LABELLED.has(block.kind) && (
                    <span className="tex-label" style={{ color: colour, borderColor: colour }}>
                      {label ?? KIND_LABEL[block.kind]}
                      {block.title ? ` (${block.title})` : ""}
                    </span>
                  )}
                  <div
                    className={`tex-body ${block.kind === "proof" ? "tex-body--proof" : ""}`}
                  >
                    {layout && (
                      <p className="tex-layout-note">
                        Ce bloc contient sa propre mise en page. L'aperçu l'empile en
                        une colonne — le PDF, lui, la respectera et peut déborder.
                      </p>
                    )}
                    {strayTab && (
                      <p className="tex-layout-note">
                        Ce bloc aligne sur un « &amp; » sans environnement d'alignement
                        autour. Le PDF ne compilera pas — corrigez le bloc.
                      </p>
                    )}
                    {splitFigures(block.latex).map((segment, index) =>
                      segment.kind === "figure" ? (
                        <Figure
                          key={`${block.id}-f${index}`}
                          documentId={documentId}
                          tikz={segment.tikz}
                        />
                      ) : (
                        <span
                          key={`${block.id}-t${index}`}
                          dangerouslySetInnerHTML={{
                            __html: latexToHtml(segment.latex, latexColours),
                          }}
                        />
                      ),
                    )}
                  </div>
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
