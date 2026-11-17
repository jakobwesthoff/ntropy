---
title: "A link into a folder named like a note is read as a link to that note"
kind: bug
component: note
origin: review
---
# A link into a folder named like a note is read as a link to that note

Link extraction counts a link to a file inside a folder that is named like
a note basename as a link to that note. After a retitle, `reconcile` then
rewrites the link to point at the note itself, and the link to the file is
lost without a warning. Found on 2026-11-17 during the asset-handling
research (todo 01M6E0CWM5KS1FRX3DTB3VREB7).

## Problem

`parse_name` in `src/link/mod.rs` accepts any target that starts with a
ULID, continues with `-` and ends in `.md`. A slash anywhere in between is
not refused, so `01M7MNA207N7HNWA1068CN70B5-pdf-inside-only/transcript.md`
parses as the note `01M7MNA207N7HNWA1068CN70B5`.

Reproduced in a lab vault (`v-hazard`): a folder
`all-notes/01M7MNA207N7HNWA1068CN70B5-pdf-inside-only/` holding
`transcript.md`, a note linking it as
`[transcript](01M7MNA207N7HNWA1068CN70B5-pdf-inside-only/transcript.md)`,
then a retitle and `reconcile`:

```
relinked 01M7MNA207N7HNWA1068CN70B5-pdf-inside-only/transcript.md -> 01M7MNA207N7HNWA1068CN70B5-pdf-inside-only-v2.md
```

The `assets/` prefix of the asset convention in use avoids this by
accident, since such a target does not start with a ULID. Meeting
transcripts are Markdown, so a per-note folder directly in `all-notes/`
holding `.md` files hits it. The renderers are hit too, before any retitle: a second lab vault (`haz`)
rendered such a transcript link as `#link("hazard-note.pdf")[#notelink[Hazard Note]]`
in the emitted Typst. The language server and every other user of
`link::extract` see the same false note link. A folder named by the bare
ULID (`<ULID>/transcript.md`) is not affected, since its rest starts with
`/` and fails the check.

## Suggested fix

Refuse a target whose name part contains a path separator in `parse_name`,
with tests for a slash after the ULID, after the slug, and a percent-encoded
slash, and a reconcile test that leaves such a link alone.

## Relations

- Relates to: [Give each note an asset folder that ntropy manages](01M6E0CWM5KS1FRX3DTB3VREB7-give-each-note-an-asset-folder-that-ntropy-manages.md), any per-note asset folder directly in `all-notes/` needs this fixed first
