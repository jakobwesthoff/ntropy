# Research: static website export of an ntropy vault

Working notes for the feature "export a vault as a static, web-renderable
site". These documents are kept self-contained so that work can be
interrupted and resumed in a fresh context without losing state.

Started 2026-09-10.

## Files

| File | Purpose |
| :--- | :--- |
| [work-order.md](work-order.md) | The request, structured: goals, requirements, constraints, working method. Source of truth for *what* is being asked. |
| [codebase-analysis.md](codebase-analysis.md) | What ntropy already has that this feature builds on or must respect. Validated against the code at the stated commit. |
| [decisions.md](decisions.md) | Every open question, the answer the user gave, and every decision taken, in order. The resumption log. |
| [design-notes.md](design-notes.md) | The evolving design direction derived from the decisions. |
| [external-facts.md](external-facts.md) | Facts about external libraries (Shiki, search libraries, WebAssembly tooling) with sources and dates. |

## State on 2026-09-11

Every question round is decided (Q1 to Q86); no question is pending.
`design-notes.md` is the consolidated result of Q1 to Q54; the design
pass on the pages (Q55 to Q66) is recorded in `decisions.md` alone. The
settled design lives in ADRs 0046 to 0055 and in
`docs/design/site-export.md`, `html-engine.md`, and `site-frontend.md`.
The implementation stages (the Markdown walk with the HTML emitter, the
`html` render format with site themes, the `site` command, the browser
toolchain with highlighting, scheme switch, and outline, the client-side
search, and the design pass with the theme layout) are committed on the
branch `site-generation`, as is the sidebar definition (Q77 to Q84,
ADR 0056).

## How to resume

1. Read `work-order.md` for the goal.
2. Read `decisions.md` top to bottom; the "Open" section at its end lists
   what is still unanswered.
3. Read `design-notes.md` for the current design state.
4. Continue by asking the next open question or by refining the design.

## Conventions

- Facts about the codebase name their source (file path, ADR id) at least
  once in `codebase-analysis.md`.
- A decision enters `decisions.md` only when the user confirmed it in
  conversation. Rationale is recorded only when the user stated it.
- Nothing here is a design doc or an ADR; when the design settles, the
  outcome moves to `docs/adr/` and `docs/design/` and this folder is
  either deleted or reduced to what remains research.
