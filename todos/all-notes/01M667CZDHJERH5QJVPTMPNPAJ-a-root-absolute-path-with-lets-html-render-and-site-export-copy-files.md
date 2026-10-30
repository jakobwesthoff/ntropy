---
title: "A root-absolute path with .. lets html render and site export copy files outside the vault and the output"
kind: bug
component: render
impact: critical
horizon: next
origin: review
tags: [security]
---
# A root-absolute path with .. lets html render and site export copy files outside the vault and the output

Found on 2026-10-30 in the plan review for the trust-model ADR. An image
or link destination that starts with `/` and contains `..` passes
through unchecked. `render --to html` and `site` then copy a file from
outside the vault and write the copy outside the output's files
directory. A vault someone else prepared can therefore make an export
read files beside or above the vault and write into directories the
person never chose.

## Problem

`src/render/markdown/mod.rs`, `resolve_root_relative`, returns a `dest`
that starts with `/` as is; only relative destinations are normalized
and refused when they climb out of the root. `src/site/build.rs`,
`mirrored`, joins that path onto the vault root as the source and
formats `files{path}` as the target. A destination naming a directory
is mirrored with its whole tree (`files_under`). `site::build::write`
and the html engine's `ProcessContext::file_path`
(`src/bin/ntropy/run/render.rs`) join the target onto the output
directory without a containment check.

Reproduced on 2026-10-30 with a debug build at `4049071`: a note body
with `![out](/../outside.svg)` and `[out2](/../../render-b/outside.svg)`,
rendered with `render --to html -o render-b/out/x.html`, exited 0
without a warning. It wrote `out/x_files/outside.svg`, a copy of a file
beside the vault, and `out/render-b/outside.svg`, outside `x_files/`.
The HTML referenced `x_files/files/../outside.svg`.

Not tested: whether the right number of `..` makes the source and the
target the same file, or reaches any path the person can write. Read in
the code: a link to `/..` names a directory, so the whole parent of the
vault would be mirrored.

## Impact

Exporting an untrusted vault can overwrite files outside the output
directory with copies of other files, and can pull files from outside
the vault into a site that is then published.

## Suggested fix

Normalize root-absolute destinations in `resolve_root_relative` the
same way as relative ones, so a `/..` that climbs out of the root
returns `None` and takes the existing "climbs out" path. Independently
of that, refuse in `site::build::write` and in the html engine's
`file_path` any target that does not stay under the output (or files)
directory after joining. Tests: one per entry point (html render, site
export) with `/../` in an image and in a link, plus a link to `/..`.

Symlinks inside the vault that point outside it are a separate matter:
typst and the site export's file copies follow them, which ADR 0070
lists as an accepted residual of reading.
