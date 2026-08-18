# 42. Empty note creation for machine authors

Date: 2026-08-18

## Status

Accepted

Adds a second creation mode to the new-note flow of
[ADR 0015](0015-editor-integration-and-new-note-flow.md). The template
mechanism of [ADR 0017](0017-note-templates-with-placeholder-substitution.md)
is unchanged; the new mode does not consult it.

## Context

Every `new` stamps a template. A caller that already knows the whole note it
wants gains nothing from the stamped skeleton and has to work around it: the
command prints a path and nothing else, so replacing the content means reading
the file back or overwriting it unseen. Authoring the file directly instead is
not open to such a caller either, because the ULID and the canonical filename
are ntropy's to assign (ADR 0004).

The caller this is for is an LLM agent writing a note it has already composed,
which is where the read-back is pure overhead.

## Decision

`new --empty` creates the canonical file with no content in it and consults no
template. `--template` is rejected alongside it: one stamps content, the other
writes none.

Identity, location and filename stay ntropy's decision. Both creation modes
derive them through a single placement step, so the two cannot disagree about
where a new note goes or what it is called.

Nothing validates what the caller subsequently writes, and no handling is added
for the file in the meantime. An empty file has no frontmatter block, so it is
malformed by the existing rules and skipped with a warning, an error under
`--strict` (ADR 0019).

In the library the mode is `create_empty_note`, returning the path rather than a
`Note`: there is no note to parse until content arrives.

Editor behavior is unchanged. Interactive use opens the empty file, `--print`
prints its path.

## Consequences

- Until the caller writes frontmatter, the vault holds a file every scan warns
  about and `--strict` fails on.
- The printed path is all a caller needs to author a note: one write, no read.
- A `default.md` that could never render a well-formed note does not stand in
  the way of this mode, which never opens it.
- In an encrypted vault the file is an age container that decrypts to the empty
  string, so creation behaves as it does elsewhere, including on a locked vault.
  Writing the note yourself does not carry over: that path is ciphertext.
