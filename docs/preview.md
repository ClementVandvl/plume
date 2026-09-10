# The review preview

How a block's LaTeX becomes what the teacher sees while reviewing, and where
to add things without growing a thicket.

## Two renderers, one decision

The preview draws each block with one of two renderers.

- **The HTML converter** (`src/preview/latexToHtml.ts`) runs in the page. It is
  instant, follows the theme, and its text can be selected. It knows prose,
  maths (KaTeX) and lists. It stacks everything in one column and knows
  nothing about layout, on purpose.
- **The engine** (`src/components/EngineImage.tsx`, `src-tauri/src/figures.rs`)
  is the real LaTeX compiler. It is exact and costs a second or two per
  request, once — results are cached beside the document, keyed by source and
  preamble. It draws diagrams (`tikzpicture`, on their own) and *passages*: a
  whole block that lays itself out, set in a minipage of the charte's text
  width so it shows as it will print, line breaks included. Both are compiled
  with the charte's full preamble and cropped by the `preview` package. A
  figure once got a lighter `standalone` document carrying only the charte's
  colours, and a brace drawn with a decoration library the charte loads was
  refused in the review: what compiles in the PDF must compile here, and the
  only way to be sure is the same preamble.

`src/preview/detect.ts` is the only place that decides between the two, in
`route`. A block containing a layout construct (`tabular`, `minipage`,
`multicols`, `\rule`, `\hfill`, `\newpage`, `\vspace` — the same list as
`render::LAYOUT_COMMANDS`) goes whole to the engine. Anything else is cut
around its diagrams: prose to the converter, diagrams to the engine.
`BlockBody.tsx` applies the decision and provides the fallback: when the engine
cannot render a passage — no engine installed, a block that does not compile —
the HTML rendering is shown instead, under a line saying why.

The rule that keeps this maintainable: **the converter never learns layout.**
LaTeX has more ways of making a table than a converter will ever cover, and the
model finds a new one each course. A construct the converter cannot draw
faithfully is added to the layout list, not to the converter.

## The converter is a pipeline

`latexToHtml` is a fixed sequence of passes, each a small function with a doc
comment that names the symptom it treats:

1. `stripComments`
2. **Parking** — `parkFigures`, `parkDisplayMaths`, `parkInlineMaths`,
   `parkMathsEnvironments`: every fragment of HTML produced here is parked in a
   slot (`html.ts`) before the prose is escaped, and restored at the very end.
   Injected in place, it was escaped and shown as literal markup.
3. `escapeHtml`
4. **Shaping** — `lists`, `centred`, `lineBreaks`: structure read off
   `\begin{…}` and `\\`, before commands are resolved.
5. `applyCommands` (`commands.ts`) — one table, `COMMANDS`, mapping a command
   name to what it becomes, and one scanner that reads arguments at their real
   brace boundaries. A command with a story carries it on its handler.
6. `paragraphs`, then the slots are restored.

To add a behaviour:

- a new command: one line in `COMMANDS`;
- a new structure: one pass, appended to `SHAPING` (or `PARKING` if it
  produces HTML that must survive escaping), with its symptom in the comment;
- a KaTeX disagreement: `maths.ts`;
- never a branch inside an existing pass for a special case.

The tests in `latexToHtml.test.ts` run every pass over real blocks from a real
reading (`__fixtures__/blocks.json`) and assert that no LaTeX source reaches
the reader; `detect.test.ts` pins the routing.
