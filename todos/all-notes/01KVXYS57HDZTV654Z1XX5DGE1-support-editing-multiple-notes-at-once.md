---
title: "Support editing multiple notes at once"
kind: feature
component: cli
origin: discussion
tags: [ux]
---
# Support editing multiple notes at once

Deferred during the v1 design (ADR 0015). v1 edits a single note at a time,
and the interactive picker is single-select. Multi-select would let a user
open several notes in one editor session.

## Proposal

Open every selected note in one editor invocation, and reconcile every
touched note when the editor exits.

## Open questions

- Should the picker allow multi-select?
- Should all selected notes open in one editor invocation, for example
  `nvim file1 file2 ...`?
- Should every touched note be reconciled on the single editor exit?
