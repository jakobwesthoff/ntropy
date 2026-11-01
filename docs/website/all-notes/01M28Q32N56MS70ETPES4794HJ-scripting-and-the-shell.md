---
title: Scripting and the shell
tags: [docs/integrate]
site:
  index: true
  label: Integrations
  order: 6
---
Scripts, editors and coding agents all drive the same ntropy command
line. This page is the plain-text side: the non-interactive mode, writing
a note from a script, reading the output, and the `ncd` shell function.

## Non-interactive mode

Pass `-n` (`--non-interactive`) and ntropy drops the interactive parts:
no picker, no editor, no prompt, only plain text on stdout. Piping alone
does not do that. ntropy stays interactive as long as it can reach your
terminal, so scripts and command substitutions must say `-n`. Where no
terminal exists, as under cron or CI, the same happens on its own.

## Write a note from a script

`ntropy new --empty` creates a note's file with no content in it: ntropy
picks the ULID, the location and the file name, and the text is yours to
write. `ntropy write` stores a note's whole text from stdin, then realigns
the file name, refreshes the views and prints the resulting path. The
two author a note without an editor and without reading anything back,
in a plaintext and an encrypted vault alike:

```bash
note=$(ntropy new --empty -p Quarterly review)
ntropy write "$note" <<'EOF'
---
title: Quarterly review
tags: [work, planning]
---
# Quarterly review

Numbers go here.
EOF
```

`write` takes the whole note, frontmatter first, and refuses text that is
not a well-formed note. It names its target, a full ULID, the note's file
name, or its path in `all-notes/`, and never searches for it, so it opens
no picker and asks no question. Until frontmatter lands in an `--empty`
file, the file is not a well-formed note, and ntropy skips it with a
warning. `--template` is refused alongside `--empty`.

## The note table

`search -n` prints one note per line as a space-aligned
`ID DATE TITLE TAGS PATH` table. Each column is padded to its widest cell
in Unicode display width and separated from the next by two or more
spaces. The last column stays unpadded, so no line carries trailing
whitespace. Rows are newest first, tags are comma-joined, and an
uppercase header row leads the output. `tags` and `view list` print
headers too.

Split it with `awk -F'  +'`, a field separator of two or more spaces:
`awk -F'  +' '{print $1}'` prints the ID column, and `tail -n +2` drops
the header. One caveat: a title with doubled spaces is printed verbatim,
so a cell can hold a two-space run that shifts the fields after it.
Treat TITLE and the columns after it as best-effort. ID and DATE never
contain spaces and are always safe.

A control character in a cell, such as a line break or an escape
character in a title or file name, is shown escaped as `\n` or
`\u{1b}`, and so are the bidirectional override and isolate characters.
A row stays one line, and a cell cannot send commands to your terminal,
also when the vault came from someone else.

## Paths and content

File paths need no parsing at all. `search -n -p` prints every match as
one path per line (`ntropy search -n -p tag:work | xargs grep -l
deadline`), and `new -p` and `today -p` print the created note's path.

A path is printed exactly as it is on disk. A path that holds a control
or bidi character would act on your terminal, so it is not printed at
all. A command that prints one path fails instead. `search -n -p` leaves
such a path out with a warning, prints the rest and exits non-zero.
`ntropy reconcile` renames such a note to its canonical name.

To read a note rather than locate it, `search -P` (`--print-content`)
writes its text to stdout, for exactly one note. It reads the same in an
encrypted vault, where `-p` names the ciphertext file; see
[Encrypted vaults](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md), which
also covers `--identity` and `--passphrase-file` for scripts.

```bash
ntropy search -n -P 01J8Z9K…            # the note, either kind of vault
for id in $(ntropy search -n tag:work | tail -n +2 | awk '{print $1}'); do
    ntropy search -n -P "$id"
done
```

`show -n` is not the way to a note's text. It prints the note laid out
for reading: plain text when stdout is a pipe, wrapped at 80 columns or
at `--width`, with the Markdown markers gone. Use it to display a note,
and `-P` to process one.

## Exit codes

A `search` that matches nothing exits non-zero, so
`if ntropy search -n tag:urgent; then …` branches on "did anything match"
without parsing a single line. `delete` under `-n` asks nothing, so it
refuses to run without `-f` (`--force`). Where a note has to be named
back to you, as in a delete prompt or an ambiguous match, it appears as
`date  title  [tags]  (id)`.

## Shell integration

`ntropy info --print` prints the active vault's path and nothing else,
always absolute. That is the hook for shell functions, and the
repository ships one for bash and zsh:
[`contrib/shell/ntropy.sh`](https://github.com/jakobwesthoff/ntropy/blob/main/contrib/shell/ntropy.sh)
defines `ncd`, which changes directory to the active vault.

```bash
# In ~/.bashrc or ~/.zshrc, from a checkout of the repository
source /path/to/ntropy/contrib/shell/ntropy.sh

# Installed with Homebrew, the formula has put the script here
source "$(brew --prefix)/opt/ntropy/share/ntropy/ntropy.sh"
```

```bash
ncd                   # cd to whichever vault is active here
ncd --vault ~/notes   # or name one; every global flag is forwarded
```

It has to be a shell function rather than an `ntropy cd` subcommand,
because a process cannot change its parent shell's working directory.
An unresolvable vault leaves the shell where it was. A `cargo install`
or a downloaded binary does not put the script on disk, so source it
from a checkout of the repository.
