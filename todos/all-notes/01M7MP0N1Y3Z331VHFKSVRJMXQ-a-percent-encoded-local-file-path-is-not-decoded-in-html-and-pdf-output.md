---
title: "A percent-encoded local file path is not decoded in HTML and PDF output"
kind: bug
component: render
origin: review
---
# A percent-encoded local file path is not decoded in HTML and PDF output

A note that links a vault file with a space in its name the usual Markdown
way, `![enc](my%20pic.png)`, does not get the file in its outputs. The
HTML export looks for a file literally named `my%20pic.png`, warns, and
writes a dead link; the PDF render fails. Found on 2026-11-17 during the
asset-handling research (todo 01M6E0CWM5KS1FRX3DTB3VREB7).

## Problem

Reproduced with the debug build of `ed914a3` on a scratch vault holding
`all-notes/my pic.png` and a note with `![enc](my%20pic.png)`:
`render -n --to html` warns
`referenced file not found in the vault: <vault>/all-notes/my%20pic.png`
and writes `src="<stem>_files/files/all-notes/my%20pic.png"` with nothing
copied. A researcher's lab showed the PDF render failing on the same link
(lab vault `haz`); not re-run by hand.

CommonMark allows percent-encoded link destinations, and editors that
paste images may write `%20` for spaces (from memory, not verified per
editor), so notes from such tools can carry these links. Note links already decode percent escapes in their
anchor part (`src/link/mod.rs`); file paths do not.

## Suggested fix

Decode percent escapes in a local file path before resolving it against
the vault, in the shared path resolution of the HTML, site and Typst
engines, with tests for a space, a `#` written as `%23` and an invalid
escape left as written.

## Relations

- Relates to: [HTML and PDF output keep a line ending in a link target unencoded](01M6CQRJHDSP2D0G33T4JHBFM5-html-and-pdf-output-keep-a-line-ending-in-a-link-target-unencoded.md), the other direction of link-target encoding
