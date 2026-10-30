# 71. Escaping untrusted text for the terminal

Date: 2026-10-30

## Status

Accepted

## Context

Note titles, file and directory names, link targets in note bodies,
template names, view names and fields, the entries below ntropy's marker in
`.gitignore`, and the vault's own path come from files and directories that
ntropy may have found by walking up from the current directory. ntropy
printed them byte for byte. An escape character in one of them could send
terminal control sequences: retitle the window, clear the screen, move the
cursor, or on some terminals write the clipboard (OSC 52). A line break or
tab split a table row or shifted its columns. The bidirectional override
and isolate characters can reorder a title so that it reads as different
text.

## Decision

Text from the vault is escaped before it is shown, by one function,
`ntropy::text::escape_controls`:

- the control characters (U+0000 to U+001F, U+007F, U+0080 to U+009F) and
  the bidi overrides, embeddings and isolates (U+202A to U+202E, U+2066 to
  U+2069) are escaped as Rust's `char::escape_debug` writes them: `\0`,
  `\t`, `\n`, `\r`, and `\u{hex}` for the rest;
- every other character is kept: the directional marks U+200E, U+200F and
  U+061C and the joiners U+200C and U+200D, which mixed-script titles and
  emoji need; combining marks; and the tag characters U+E0000 to U+E007F,
  which emoji subdivision flags such as England's are made of. A backslash
  or quote is shown as it is.

Each escape is ASCII, and the aligned table and the picker measure, pad
and cut the escaped text.

Escaping applies whether or not the output is a terminal, so the output is
the same everywhere (ADR 0033). It covers table cells, note references in
prompts and messages, warnings, the reconcile and `init` reports, `info`,
the vault's path in status lines and passphrase prompts, error messages,
the picker's rows, whose search text is escaped too, so the match
highlights point at what is drawn, and the language server's completion
labels, inserted link titles, workspace symbol names and the error it
shows for a broken vault pointer. In an error message that ends a command
line breaks are kept, so a message that spans lines, such as an external
tool's output, keeps its lines.

A path printed for a script (`-p`, `write`, the picker's print-path key) is
printed as it is, so it names the file on disk. A path that would need
escaping is not printed at all: a single one is an error, and among several
the command prints the others, warns about each one it leaves out and exits
non-zero. ntropy never prints a path other than the one on disk, and never
prints bytes that drive the terminal. Where the note's file name is the
cause, the message says to run `ntropy reconcile`, which gives the note its
canonical name. `--print-content` prints the note itself, unchanged.

Values the user typed, such as a selector echoed in `no note matches`, are
shown as typed; inside an error message that ends the command they are
escaped with the rest of it.

## Consequences

- A title shown with `\u{1b}` cannot be told apart from one that contains
  those six characters; `--print-content` shows which.
- A line break from the vault inside an error message still starts a new
  line on stderr.
- An external tool's diagnostics show tabs, carriage returns and colour
  sequences escaped.
- Titles can still look alike through homoglyphs or invisible characters
  such as U+200B; the ULID in every note reference tells them apart.
- A note whose file name holds an escaped character has no `-p` path until
  `ntropy reconcile` renames it. In a vault whose own path holds one, no
  `-p` path is printed at all.
- The picker matches the query against the escaped title.
- A link the language server completes for such a title carries the
  escaped title as its display text.
