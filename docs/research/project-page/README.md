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

The request is recorded. Questions Q1 to Q6 are answered: the vault's
location, the source of the exporter binary in the pages workflow, the
organizing principle of the documentation, and the extra pages the site
carries. The split of the README into pages and the depth of the
sidebar are deferred until the overall structure is agreed.

The user decided that the template override for site themes has to land
before the page work starts; that feature belongs to the site export and
its questions continue the log in
[../web-export/decisions.md](../web-export/decisions.md).

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
