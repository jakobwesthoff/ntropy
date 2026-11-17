# 105. Linked files are attached to the PDF

Date: 2026-11-17

## Status

Accepted

Builds on [ADR 0098](0098-a-notes-assets-live-in-a-folder-named-like-the-note.md)
and [ADR 0101](0101-renders-and-exports-serve-only-a-notes-own-assets.md), and
applies the export rules of
[ADR 0072](0072-export-paths-stay-inside-the-vault-and-the-output.md) and
[ADR 0073](0073-what-html-and-site-exports-copy-from-the-vault.md) to what a
PDF embeds.

## Context

A link to a non-image file in a note's own asset folder renders in the PDF as a
plain link to a path beside the artifact. typst's `pdf.attach` stores a file in
the PDF at document level and draws nothing, so the page has to show that a
file is attached. A viewer lists such files in its attachments panel. The link
itself cannot open the attached file: `/GoToE` targets only an embedded PDF and
viewers support it unevenly, and the clickable form, a FileAttachment
annotation, is not something typst 0.15.1 draws.

## Decision

The `typst` and `pdf` formats attach every non-image file that a link of the
note names in the note's own asset folder, up to a limit per file. The limit is
`[render] attach_max`, 25 MiB by default, an integer in bytes or a size string
such as `"25 MiB"`; `0` turns attaching off. A per-file limit makes the result
independent of the order of the links. The `#pdf.attach` calls are written once
each, after the whole body, and not beside the links: typst evaluates a table
header repeated on every page, a heading an `outline()` draws again and a
footnote more than once, and fails a compile that attaches a path twice. A theme
that lays out the body twice breaks attachments for the same reason. A file
linked several times is attached
once.

After the link text the page shows a badge. A file within the limit gets
`attached · 1.2 MiB`, tinted in the link colour. A file above the limit is not
attached and gets `not attached · 240 MiB`, in muted grey, and the render warns
"`<path>` is 240 MiB, above attach_max (25 MiB); not attached", which `--strict`
fails on. The badges are the prelude functions `attachbadge` and
`attachbadge-off`, which a document theme may redefine. Images are not
attached, nor are links into another note's folder (ADR 0101) or to files
outside `all-notes/_assets/`.

The typst engine touches no filesystem, so the host judges each file through
`RenderContext::attachment`, which answers with the file's size, a refusal or
nothing. A file is attached only when it passes the rules of ADR 0072 and
ADR 0073: it resolves, inside the vault, to a regular file whose path holds no
entry that exports never copy, and not into another note's asset folder. A
symlink in the note's asset folder that leads outside the vault, such as to a
private key, is refused. A refusal is a warning "`<path>` is not attached:
`<reason>`", which `--strict` fails on, and the link renders without a badge.
A missing file and a directory are not attached and raise no warning of their
own, since typst fails the whole compile on either.

The attachment's name in the viewer is the file's path from the vault root, such
as `all-notes/_assets/<stem>/minutes.pdf`.

The host approves a file's path and size when the emitter asks, and typst reads
the file later, when it compiles. A process that can write into the vault between
the two can replace the file with a symlink to another file, or grow it beyond
`attach_max`, and typst attaches what it then finds. The vault is trusted for
writes during a render: a render of a vault that others can write to while it
runs is not covered.
