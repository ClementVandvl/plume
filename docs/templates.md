# Templates

> Written in English, like the rest of the technical content.

A template is the house style: what the LaTeX looks like once the semantic IR
has been read. It has three parts, and the editor has one tab for each.

| Part | What it holds | Where it lives |
|---|---|---|
| **Keys** | Values substituted into the preamble — colours, paper size, environment labels | `template.json`, `keys` |
| **Preamble** | The LaTeX skeleton, with `{{key}}` placeholders | `preamble.tex.tmpl` |
| **Blocks** | How each IR block kind is written | `template.json`, `blocks` |

Both files sit in `~/Documents/Plume/Templates/<id>/`.

## The bundled template is read-mostly

`charte-maths` ships inside Plume, and [`seed`](../src-tauri/src/templates.rs)
rewrites its `preamble.tex.tmpl` whenever the bundled version rises. So:

- **Key values survive an upgrade.** `seed` carries them across, and they stay
  editable. Changing a colour is safe forever.
- **Everything else does not.** An edited preamble, a renamed template, a
  changed block mapping would be overwritten at some future release —
  silently, months later, with nothing to explain where the work went.

Rather than let that happen, `write_preamble`, `delete` and the structural half
of `save` refuse the bundled id and say why. **Duplicating** is the supported
path: a copy gets a fresh id, `version: 1`, and is never touched by `seed`
again. `upgrading_never_touches_a_personal_template` is the test that keeps
this true.

## Placeholders

`render_preamble` substitutes `{{key}}` with the key's value, dropping the `#`
from colours because `\definecolor{...}{HTML}{...}` does not want it.

Placeholders usually sit **inside** a LaTeX group:

```latex
\definecolor{mcChapitre}{HTML}{{{color.chapter}}}
```

That is three opening braces: the group, then the placeholder. Any validation
of the preamble has to skip the extra brace before reading the key name — the
first version of `write_preamble` did not, and rejected every valid preamble by
reading the key as `{color.chapter`. Ordinary doubled braces such as
`{{\bfseries x}}` are not placeholders and are left alone.

Saving a preamble that names a key no template defines is refused, with the key
named. Otherwise the `{{typo}}` survives substitution, reaches the compiler as
literal braces, and fails far from its cause.

## Checking

**Vérifier** compiles the template on its own, against a probe document that
exercises every environment and command the block mappings declare. A broken
`\newtcolorbox` therefore surfaces in the editor rather than on the one block
that happened to use it — and long before an export, which is after a reading
has been paid for.

`the_check_compiles_a_healthy_template_and_rejects_a_broken_one` runs both
directions; it is `#[ignore]`d because it needs the engine, so run it by hand
after touching the probe:

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored the_check_compiles
```

## Block mappings

Each IR block kind maps to one of four forms:

| Mode | Output |
|---|---|
| `command` | `\name{content}` |
| `numbered` | `\name{number}{title}` — headings |
| `environment` | `\begin{name}[title] content \end{name}` |
| `raw` | the block's LaTeX, unchanged |
| `centered` | wrapped in `center` |

`numbered` exists because **Plume numbers nothing itself**. A course
photographed from the middle of a notebook opens on "Chapitre 3", and
renumbering it to 1 would contradict every other document the class holds. The
number read from the page is passed through as written — "3", "II", "1", "a" —
and the template decides how to show it. An unnumbered heading is passed an
empty first argument and shows no number: none is invented. The bundled macros
use `\ifblank` for that, and the section counters were dropped in favour of the
starred forms, which keep the formatting without numbering.

A kind with no mapping falls back to raw output and loses its environment;
`bundled_template_maps_every_block_kind` makes sure the bundled template never
does that.

## Conventions

A template carries its own typesetting instructions, compiled into the
recogniser's prompt after the teacher's standing conventions and before
anything specific to the course. The distinction is scope: marker rules and
standing conventions describe *how this teacher writes* whatever template they
use; these describe *what this house style wants of the LaTeX*, and follow the
template when a course changes style.

The bundled one ships a single rule, `align-equals`. A calculation continued
over several lines must be grouped in one `aligned` environment with `&=`
alignment points, rather than emitted as separate display equations:

```latex
2) $\begin{aligned}[t]
-2x(4x-1) &= -2x \times 4x - (-2x) \times 1 \\
&= -8x^2 + 2x
\end{aligned}$
```

The instruction names the delimiters explicitly. Left vaguer, the model emits
bare alignment tabs — `A = \frac{3}{5} &= \frac{9}{15} \\ &= ...` with no
environment at all. That is a LaTeX error, *and* the preview showed it as a
literal "&=" beside a fraction mangled down to "5", which is exactly what
reached the screen once. `render::has_stray_alignment` and its preview twin now
flag such a block during review, in both places, rather than letting it reach an
export that cannot compile.

Without the rule, the continuation becomes `\[ = -8x^2 + 2x \]` and is flung to
the centre of the page, far from the line it continues. The `[t]` keeps the block on
the baseline of the text that introduces it — drop it and the list number floats
to the middle of the block. KaTeX ignores that option and would typeset it as a
literal "[t]", so `forKatex` strips it **for the preview only**; the exported
LaTeX keeps it.

A third, `inline-comment`, keeps a remark written beside a calculation beside
it, in ordinary type:

```latex
C &= \dfrac{-3}{4} \times \dfrac{5}{-7} && \text{(on multiplie les numérateurs)}
```

`&&` opens a further column, so the remark shares the line instead of dropping
below it. `\text{…}` matters more than it looks: maths mode sets the phrase in
italic, **removes its spaces and drops its accents** — "(on multiplie les
numérateurs)" came out as "(onmultiplielesnumrateurs)".

Unlike the preamble, conventions on the bundled template **are** editable, and an
upgrade tells an edit from an untouched entry rather than guessing. Each shipped
convention records its wording as delivered, in `shipped`:

- `text == shipped` — the teacher never touched it, so a corrected instruction
  replaces it.
- `text != shipped` — they reworded it; theirs wins and survives.
- `enabled` is theirs either way, and entries they wrote (with generated ids) are
  never touched.

The field exists because assuming every installed entry was the teacher's
stranded a workbook on a superseded instruction while its version number moved
on, so nothing ever looked again. Corrections therefore ride on the version
bump, like every other change to the bundled template.

## Deleting

A deleted template moves to `Corbeille/Modeles/` rather than being erased — the
same courtesy courses get. Courses referring to it will need another one.
