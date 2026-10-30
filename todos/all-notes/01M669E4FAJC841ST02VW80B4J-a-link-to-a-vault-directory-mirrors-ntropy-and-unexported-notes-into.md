---
title: "A link to a vault directory mirrors .ntropy and unexported notes into html and site output"
kind: bug
component: site
status: needs-discussion
impact: high
origin: review
tags: [security]
---
# A link to a vault directory mirrors .ntropy and unexported notes into html and site output

Found on 2026-10-30 while fixing export paths that climb out of the
vault (ADR 0072). A note link to a directory inside the vault mirrors
that directory's whole tree into the export. A link to `/` or `/.ntropy`
therefore copies `.ntropy/` into `render --to html` or `site` output,
including the configuration and, in an encrypted vault, the
passphrase-protected identity. A link to `/all-notes` copies every note,
also those the site export was told to leave out. Someone who publishes
the export publishes those files.

## Problem

`src/site/build.rs`, `mirrored`, copies a linked directory with
`files_under`, and the html engine does the same
(`src/render/html/engine.rs`, around the `build::files_under` call).
Nothing excludes `.ntropy/` or notes outside the export. Read in the
code on 2026-10-30 by the agent fixing ADR 0072; not reproduced
separately.

## Impact

A note, possibly written by someone else, can make an export carry the
vault's configuration and identity file and notes the person meant to
keep private.

## Options

- Never mirror `.ntropy/`, and skip note files outside the exported set
  when mirroring a directory (with a warning).
- Refuse links to directories that contain notes or `.ntropy/`, and
  keep directory mirroring for asset folders only.
- Drop directory mirroring entirely and mirror only linked files.

## Open questions

- Which of the options keeps the use case directory mirroring exists
  for (ADR 0057, `docs/design/html-engine.md` and
  `docs/design/site-export.md` describe the files directory).
- Whether a vault's own `assets/`-style folders need an explicit list.
