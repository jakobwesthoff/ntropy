# Research: the project page as an ntropy export

Working notes for replacing the generated project page at
<https://ntropy.westhoffswelt.de> with a website that ntropy exports from
a documentation vault inside this repository. These documents are kept
self-contained so that work can be interrupted and resumed in a fresh
context without losing state.

Started 2026-09-11 on the branch `new-project-page`.

## Files

| File | Purpose |
| :--- | :--- |
| [work-order.md](work-order.md) | The request, structured: goals, stages, constraints. Source of truth for *what* is being asked. |
| [decisions.md](decisions.md) | Every open question, the answer the user gave, and every decision taken, in order. The resumption log. |

## State on 2026-09-11

The request is recorded. Questions Q1 to Q17 are answered: the vault's
location, the source of the exporter binary in the pages workflow, the
organizing principle of the documentation, the extra pages the site
carries, the starter page's parts, and the six sections with their
pages (the list stands at the end of the decided questions in
`decisions.md`). Next is stage 2 of the work order: the vault under
`docs/website/` and the transfer of the README's content into its
notes.

The user decided that the template override for site themes has to land
before the page work starts; that feature belongs to the site export, its
questions continue the log in
[../web-export/decisions.md](../web-export/decisions.md) (Q91 to Q98),
and it is implemented (ADR 0058).

## How to resume

1. Read `work-order.md` for the goal and the stages.
2. Read `decisions.md` top to bottom; the "Open" section at its end lists
   what is still unanswered.
3. Continue by asking the next open question.

## Conventions

- A decision enters `decisions.md` only when the user confirmed it in
  conversation. Rationale is recorded only when the user stated it.
- Nothing here is a design doc or an ADR; when the design settles, the
  outcome moves to `docs/adr/` and `docs/design/` and this folder is
  either deleted or reduced to what remains research.
