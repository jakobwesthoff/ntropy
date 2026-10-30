---
title: "Refuse note images that leave the vault through a symlink in PDF renders"
kind: improvement
component: render
impact: high
origin: review
tags: [security]
---
# Refuse note images that leave the vault through a symlink in PDF renders

Found on 2026-10-30 while ADR 0073 was planned, which stops `render --to html`
and `site` from copying files that resolve outside the vault. `render --to pdf`
is not covered: a note's image whose path is, or passes through, a file
symlink inside the vault that points outside it is embedded in the PDF. ADR
0070 lists this as an accepted residual ("typst follows a symlink inside the
vault").

## Problem

ntropy runs `typst compile --root <vault root> - <out>`
(`src/render/typst/engine.rs`), and the emitter writes a note's local image
paths root-absolute (`src/render/typst/emitter.rs`, `resolve_root_relative`).
typst refuses a path that climbs above the root, but follows a file symlink
inside the root to a target outside it. Established on 2026-10-30 with typst
0.15.1: an image path through such a symlink compiled and the outside file
was embedded. A directory symlink was not followed in the same test; the
cause was not investigated. Only files that decode as an image get in this
way; arbitrary text does not.

## Impact

A vault from someone else, such as a cloned or synced one, can make a PDF the
user renders and passes on carry an image file from outside the vault.

## Suggested fix

The emitter sees every local image path. Resolve each one before the compile
and apply the rules of ADR 0073 to it: a path that resolves outside the vault,
or matches the list of refused names, is left out with a warning, as in the
html and site exports. Then amend ADR 0070's residual so it names only what
remains (Typst theme reads).

## Decisions

- 2026-10-30, user: not in release 2.3.0; filed as a todo after the release.
