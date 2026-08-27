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
| `environment` | `\begin{name}[title] content \end{name}` |
| `raw` | the block's LaTeX, unchanged |
| `centered` | wrapped in `center` |

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

Without it, the continuation becomes `\[ = -8x^2 + 2x \]` and is flung to the
centre of the page, far from the line it continues. The `[t]` keeps the block on
the baseline of the text that introduces it — drop it and the list number floats
to the middle of the block. KaTeX ignores that option and would typeset it as a
literal "[t]", so `forKatex` strips it **for the preview only**; the exported
LaTeX keeps it.

Unlike the preamble, conventions on the bundled template **are** editable: `seed`
keeps what is installed and only appends bundled entries whose id is new. What
survives an upgrade is what may be edited — that is the whole rule.

## Deleting

A deleted template moves to `Corbeille/Modeles/` rather than being erased — the
same courtesy courses get. Courses referring to it will need another one.
