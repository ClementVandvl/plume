import { route, splitFigures, type Segment } from "../preview/detect";
import { latexToHtml } from "../preview/latexToHtml";
import type { Colours } from "../preview/commands";
import { EngineImage } from "./EngineImage";

/**
 * The body of a block in the review, by whichever renderer it needs.
 *
 * `detect.route` decides: a block with its own layout goes whole to the
 * engine, and falls back to the HTML rendering if the engine cannot; anything
 * else is converted in the page, its diagrams alone going to the engine.
 */
type Props = { documentId: string; latex: string; colours: Colours };

export function BlockBody({ documentId, latex, colours }: Props) {
  const plan = route(latex);

  if (plan.renderer === "engine") {
    return (
      <EngineImage
        documentId={documentId}
        kind="passage"
        source={latex}
        fallback={<HtmlSegments documentId={documentId} segments={splitFigures(latex)} colours={colours} />}
      />
    );
  }

  return <HtmlSegments documentId={documentId} segments={plan.segments} colours={colours} />;
}

function HtmlSegments({
  documentId,
  segments,
  colours,
}: {
  documentId: string;
  segments: Segment[];
  colours: Colours;
}) {
  return (
    <>
      {segments.map((segment, index) =>
        segment.kind === "figure" ? (
          <EngineImage key={`f${index}`} documentId={documentId} kind="figure" source={segment.tikz} />
        ) : (
          <span
            key={`t${index}`}
            dangerouslySetInnerHTML={{ __html: latexToHtml(segment.latex, colours) }}
          />
        ),
      )}
    </>
  );
}
