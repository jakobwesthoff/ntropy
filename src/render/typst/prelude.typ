// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Engine-owned definitions inlined ahead of every emitted document. They are
// the seam a theme replaces: a theme is a different prelude defining `note`,
// `callout`, `notelink`, and `task`, while the converted body below stays
// identical. Inlining (rather than importing) keeps the emitted `typst`
// artifact a single self-contained file that compiles next to the note.
//
// The default theme builds only on the fonts the typst binary bundles
// (Libertinus Serif, New Computer Modern, DejaVu Sans Mono), so a rendered
// note looks the same on every machine.

// Render a frontmatter value as legible inline text. Strings pass through as
// themselves; arrays and dictionaries recurse so nested metadata stays
// readable rather than collapsing to a Typst repr; every other scalar (int,
// float, bool, none, datetime) falls back to its repr, which is faithful and
// short for leaf values.
#let fmt-value(value) = {
  if type(value) == str {
    value
  } else if type(value) == array {
    value.map(fmt-value).join(", ")
  } else if type(value) == dictionary {
    value.pairs().map(((k, v)) => k + ": " + fmt-value(v)).join("; ")
  } else {
    repr(value)
  }
}

// A value that carries nothing worth displaying. The template still receives
// such fields; skipping them is purely a presentation choice of this theme.
#let is-empty(value) = {
  value == none or value == "" or value == () or value == (:)
}

// Per-kind callout identity. The palette follows the GitHub admonition
// colors, so notes rendered here read like they do on github.com.
#let callout-styles = (
  note: (color: rgb("#0969da"), label: "Note"),
  tip: (color: rgb("#1a7f37"), label: "Tip"),
  important: (color: rgb("#8250df"), label: "Important"),
  warning: (color: rgb("#9a6700"), label: "Warning"),
  caution: (color: rgb("#cf222e"), label: "Caution"),
)

// A callout is a GFM admonition. Every kind renders through the same form —
// colored left bar, tinted fill, colored bold lead-in — so an unrecognized
// kind is handled by the same code path in neutral grey with its own
// capitalized name as the lead-in.
#let callout(kind: "note", body) = {
  let style = callout-styles.at(
    kind,
    default: (color: luma(90), label: upper(kind.slice(0, 1)) + kind.slice(1)),
  )
  block(
    width: 100%,
    stroke: (left: 3pt + style.color),
    fill: style.color.lighten(94%),
    radius: (right: 4pt),
    inset: (left: 1em, rest: 0.7em),
    {
      text(weight: "bold", fill: style.color, style.label)
      linebreak()
      body
    },
  )
}

// A resolved note-to-note link: the target's current title, set apart from
// ordinary emphasis by the link color.
#let notelink(body) = {
  text(fill: rgb("#0969da"), emph(body))
}

// A task-list checkbox, drawn rather than taken from a glyph: one identical
// box for both states, so checked and unchecked always match optically
// regardless of font coverage.
#let task(done: false) = {
  box(
    width: 0.85em,
    height: 0.85em,
    stroke: 0.7pt + luma(110),
    radius: 1.5pt,
    baseline: 0.1em,
    if done {
      align(center + horizon, text(size: 0.75em, fill: luma(40), sym.checkmark))
    },
  )
  h(0.4em)
}

// The document template. It receives the note title, the complete frontmatter
// mapping, and the paper size, and lays them out as the default look: the
// title prominent and centered, a subdued metadata line (tags as chips, other
// fields as quiet key-value pairs, empty values skipped), a separating rule,
// then the converted body.
#let note(title: none, frontmatter: (:), paper: "a4", body) = {
  // A conditional set rule: a `set` inside an `if` block would be scoped to
  // that block and never reach the document. The guard also avoids
  // `set document(title: none)`, which would be a type error.
  set document(title: title) if title != none

  set page(paper: paper, margin: (x: 2.2cm, y: 2.4cm))
  set text(size: 11pt)
  set par(justify: true, leading: 0.7em)

  // Heading rhythm: air above, tight below, restrained size steps.
  show heading: set block(above: 1.6em, below: 0.8em)
  show heading.where(level: 1): set text(size: 1.45em)
  show heading.where(level: 2): set text(size: 1.2em)
  show heading.where(level: 3): set text(size: 1.05em)

  // Inline code as a subtle chip; block code as a filled panel.
  show raw.where(block: false): it => box(
    fill: luma(245),
    stroke: 0.4pt + luma(225),
    radius: 2pt,
    inset: (x: 3pt, y: 0pt),
    outset: (y: 3pt),
    it,
  )
  show raw.where(block: true): it => block(
    width: 100%,
    fill: luma(248),
    stroke: 0.4pt + luma(230),
    radius: 4pt,
    inset: 10pt,
    text(size: 0.9em, it),
  )

  // Quotes: quiet grey rule, softened text, no fill.
  show quote.where(block: true): it => block(
    stroke: (left: 2.5pt + luma(210)),
    inset: (left: 1em, y: 0.3em),
    text(fill: luma(60), it.body),
  )

  // Tables: horizontal rules only, bold header row, centered on the page
  // while keeping their content width.
  set table(
    stroke: (x, y) => if y == 0 {
      (bottom: 0.8pt + luma(120))
    } else {
      (bottom: 0.4pt + luma(210))
    },
    inset: 7pt,
  )
  show table.cell.where(y: 0): set text(weight: "bold")
  show table: it => align(center, it)

  // Links in classic muted blue; thematic breaks in light grey.
  show link: set text(fill: rgb("#0969da"))
  set line(stroke: 0.6pt + luma(190))

  if title != none {
    align(center, text(size: 1.7em, weight: "bold", title))
    v(0.3em)
  }

  let fields = frontmatter
    .pairs()
    .filter(((key, value)) => key != "title" and not is-empty(value))
  if fields.len() > 0 {
    align(
      center,
      {
        set text(size: 0.85em, fill: luma(90))
        // Tags render as chips; other fields as quiet key-value pairs.
        let parts = ()
        for (key, value) in fields {
          if key == "tags" and type(value) == array {
            parts.push(
              value
                .map(tag => box(
                  fill: luma(240),
                  radius: 6pt,
                  inset: (x: 6pt, y: 2pt),
                  text(size: 0.95em, tag),
                ))
                .join(h(0.4em)),
            )
          } else {
            parts.push([#text(weight: "semibold", key + ":") #fmt-value(value)])
          }
        }
        parts.join(h(1.2em))
      },
    )
  }
  v(0.6em)
  line(length: 100%, stroke: 0.6pt + luma(200))
  v(0.9em)

  body
}
