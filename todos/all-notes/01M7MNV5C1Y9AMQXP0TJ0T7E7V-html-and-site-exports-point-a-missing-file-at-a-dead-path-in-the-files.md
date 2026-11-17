---
title: "HTML and site exports point a missing file at a dead path in the files directory"
kind: bug
component: render
impact: low
origin: review
---
# HTML and site exports point a missing file at a dead path in the files directory

A note that references a file missing from the vault gets its link
rewritten to `<stem>_files/files/<vault path>` in `render --to html`, a
path that does not exist in the output. `docs/design/html-engine.md` says
such a file "is a warning and stays unresolved", which is what happens to a
file outside the vault. Found on 2026-11-17 during the asset-handling
research (todo 01M6E0CWM5KS1FRX3DTB3VREB7).

## Problem

Reproduced with the debug build of `ed914a3` on a scratch vault: a note
with `![gone](/Users/nobody/missing.png)`, `![rel](nothere.png)` and
`![root](/shared/nothere.png)` renders with
`render -n --to html -o out2.html <note>`. Each one warns
`referenced file not found in the vault: <vault>/...`, and the page carries

```
src="out2_files/files/Users/nobody/missing.png"
src="out2_files/files/all-notes/nothere.png"
src="out2_files/files/shared/nothere.png"
```

A file outside the vault (`![out](../../outside.png)`) keeps its target as
written. The site export shares the copy rules (ADR 0073) and very likely
behaves the same; not checked.

## Suggested fix

Decide which is right, the code or the design doc. Leaving a missing
file's target as written matches the doc and the outside-the-vault case,
and keeps the link working if the file turns up beside the output.

## Relations

- Relates to: [Give each note an asset folder that ntropy manages](01M6E0CWM5KS1FRX3DTB3VREB7-give-each-note-an-asset-folder-that-ntropy-manages.md), how exports treat asset links
