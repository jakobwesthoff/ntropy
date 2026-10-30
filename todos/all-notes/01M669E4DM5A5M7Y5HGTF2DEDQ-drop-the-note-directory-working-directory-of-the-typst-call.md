---
title: "Drop the note-directory working directory of the typst call"
kind: refactor
component: render
origin: review
---
# Drop the note-directory working directory of the typst call

Found on 2026-10-30 in the plan review for ADR 0070. The `pdf` engine
starts `typst` with the note's directory as its working directory, but
nothing depends on that any more: the document arrives on stdin, its
asset paths resolve against `--root <vault>` (ADR 0045), and the output
path is absolute. Two safeguards in the binary exist only because of
that working directory, and their rationale in the design doc describes
a situation that no longer arises.

## Goal

`typst` runs in ntropy's own working directory, and the code and docs
that only served the note-directory working directory are simplified or
their rationale is restated for what remains.

## Proposal

- `src/render/typst/engine.rs`, the `Format::Pdf` arm: set the
  `Invocation`'s `cwd` to `None` (or drop the field if no engine needs
  it).
- `src/bin/ntropy/run/render.rs`: `absolutize` and `resolve_program`
  resolve `-o` and the program against the parent's working directory.
  Check whether each is still needed once the child runs in the same
  directory, and keep what still has a reason (for example, the printed
  path and the missing-tool error).
- `docs/design/rendering.md`, the two placement details ("Output-path
  absolutization" and "Program resolution in the parent's context"):
  rewrite or remove to match.

## Open questions

- Whether typst reads anything else relative to its working directory,
  such as fonts or a package cache, that would change with the
  directory. Check with a real render before and after.
