# Work order: static website export of an ntropy vault

Source: the user's request that opened this research effort, 2026-09-10.
This document restates that request in structured form. Every requirement
below traces to that request; nothing has been added to it. Refinements
and answers to open questions are recorded in [decisions.md](decisions.md),
not here.

## Goal

Export the current state of an ntropy vault as a **statically
web-renderable site**: a set of files that a browser can render when served
by any plain file host, with no ntropy-specific or otherwise special
server software involved. In the user's words: "essentially a static site
generator based on an ntropy vault".

The exported site is meant to be usable as a **documentation website**,
comparable to the documentation systems commonly in use, with a proper
entry point.

## Requirements

### R1. Static output

- The export is a set of static files, transferable as-is.
- No specific server software may be required to view it.
- Client-side JavaScript is permitted, and expected, because features such
  as search need it.

### R2. Navigation parity with the vault

The site must let a reader structure and reach notes through **all the
navigation ways ntropy otherwise offers**, explicitly including:

- tags,
- full-text search,
- the other views ntropy materializes in the filesystem (the configured
  `by-<field>` views).

### R3. Documentation-site entry point

The site has an entry point that makes it work as a documentation website
in the way established documentation-site generators do.

### R4. Themes

- A user can **choose** between different themes.
- A user can **create** their own theme.

### R5. Visual design is a separate step

The visual design of the output is explicitly deferred: "we can make the
design awesome in another step". This effort is about the export
mechanism, its structure, and its extension points.

## Forward-looking constraint: the planned desktop app

The user plans a **Tauri-based desktop application** that accesses, and
possibly edits, ntropy notes live from a vault. The design of the export
feature must keep that in mind so that as much rendering logic as
possible can be reused by that application.

This is a constraint on the design, not a deliverable of this effort.

## Working method

The user asked for the following process, in this order:

1. **Structure the request** as this work order. (Done: this document.)
2. **Analyze** the existing project thoroughly against the goal.
3. **Grill the user** on every open question and remark, using the
   interactive question tool rather than prose.
4. **Document everything** in `docs/research/`, self-contained, in
   documents laid out at the assistant's discretion.
5. **Update incrementally** as questions are answered and decisions are
   made, so that the effort can be interrupted and resumed in a fresh
   context at any time without significant information loss.

## Out of scope for this effort

- The visual design of the site (R5).
- Implementing the Tauri application.

## Deliverables of this research phase

- This work order.
- A validated analysis of the codebase's relevant parts
  ([codebase-analysis.md](codebase-analysis.md)).
- A log of questions, answers, and decisions ([decisions.md](decisions.md)).
- A design direction that follows from the decisions
  ([design-notes.md](design-notes.md)).

Implementation, ADRs, and design docs come after this phase and are not
part of it unless the user decides otherwise.
