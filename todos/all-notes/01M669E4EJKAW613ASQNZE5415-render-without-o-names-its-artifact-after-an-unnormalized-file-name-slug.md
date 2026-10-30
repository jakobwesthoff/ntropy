---
title: "render without -o names its artifact after an unnormalized file-name slug"
kind: bug
component: cli
impact: low
origin: review
tags: [security]
---
# render without -o names its artifact after an unnormalized file-name slug

Found on 2026-10-30 in the plan review for terminal-output escaping (ADR
0071). Without `-o`, `render` writes its artifact to `<slug>.<ext>` in
the working directory, taking the slug from the note's file name as it
is on disk. ntropy does not normalize file names it did not create, so a
note file whose name holds control characters, such as one shipped in a
cloned vault, makes `render` create a file with those characters in its
name.

## Problem

`src/bin/ntropy/run/render.rs`, `run`, builds the default with
`default_output_path(&note.slug, &extension)`. Read in the code on
2026-10-30, not reproduced. The `Rendered` line and `render -p` are
covered by ADR 0071: a path that would need escaping is never printed,
and `render -p` refuses before the engine runs. Without `-p` the
artifact is still created.

## Suggested fix

Apply the same rule as ADR 0071 to the default name: when the slug-based
default would need escaping, refuse before the engine runs with an
escaped error that names `ntropy reconcile` (which renames the note to
its canonical slug) and `-o`. Deriving the default from the title
instead would change the names that artifacts use to link to each other
(ADR 0044), so it is not the fix.
