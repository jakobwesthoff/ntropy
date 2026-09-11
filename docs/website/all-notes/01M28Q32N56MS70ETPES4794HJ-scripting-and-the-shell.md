---
title: Scripting and the shell
tags: [docs/integrate]
site:
  order: 2
---
Every ntropy command has a non-interactive mode that prints plain text and nothing else, which is what makes it usable from scripts, cron, and command substitutions. This page describes that output, how to parse it, how it behaves against an encrypted vault, and the shell function that ships with the repository.

## Non-interactive mode

Pass `-n` (`--non-interactive`) and ntropy drops the interactive parts: no picker, no editor, only plain text on stdout. Piping alone does not do that. ntropy stays interactive as long as it can reach your terminal, so scripts and command substitutions must say `-n`. Where no terminal exists, as under cron or CI, the same happens on its own.

## The note table

`search -n` prints one note per line as a space-aligned `ID DATE TITLE TAGS PATH` table. Each column is padded to its widest cell in Unicode display width and separated from the next by two or more spaces, except the last column, which stays unpadded so no line carries trailing whitespace. Rows are newest first, tags are comma-joined, and an uppercase header row leads the output so it describes itself. `tags` and `view list` print headers too.

Split it with `awk -F'  +'` (the field separator is a run of two or more spaces), for example `awk -F'  +' '{print $1}'` for the ID column; `tail -n +2` drops the header. One caveat: a cell can itself contain a two-space run, since a title with doubled spaces is printed verbatim, and that shifts the fields after it. Treat TITLE and the columns after it as best-effort rather than a positional contract. ID and DATE never contain spaces and are always safe.

## Paths and content

File paths need no parsing at all. `search -n -p` prints every match as one path per line (`ntropy search -n -p tag:work | xargs grep -l deadline`), and `new -p` and `today -p` print the created note's path.

To read a note rather than locate it, `search -P` (`--print-content`) writes its text to stdout. It must resolve to exactly one note. Unlike `-p` it works the same in an [encrypted vault](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md): the path there names a ciphertext file, the content does not.

```bash
ntropy search -n -P 01J8Z9K…            # the note, either kind of vault
for id in $(ntropy search -n tag:work | tail -n +2 | awk '{print $1}'); do
    ntropy search -n -P "$id"
done
```

## Encrypted vaults

Against an [encrypted vault](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md), `--identity <path>` or `$NTROPY_IDENTITY` supplies the key without the OS credential store, and `--passphrase-file <path>` supplies a passphrase without a prompt. That is what makes an encrypted vault usable from cron or CI. Note that `-p` there prints the ciphertext file's path: fine for `stat` or `xargs rm`, not for handing to an editor.

## Exit codes

A `search` that matches nothing exits non-zero, so `if ntropy search -n tag:urgent; then …` branches on "did anything match" without parsing a single line. Where a note has to be named back to you, as in a delete prompt or an ambiguous match, it appears as `date  title  [tags]  (id)`.

## Shell integration

`ntropy info --print` prints the active vault's path and nothing else, resolved by the [usual rules](01M28Q32CAAZA2PWAP4RR44GMA-finding-the-vault.md) and always absolute. That is the hook for shell functions, and the repository ships one for bash and zsh: [`contrib/shell/ntropy.sh`](https://github.com/jakobwesthoff/ntropy/blob/main/contrib/shell/ntropy.sh) defines `ncd`, which changes directory to the active vault.

```bash
# In ~/.bashrc or ~/.zshrc
source /path/to/ntropy/contrib/shell/ntropy.sh
```

```bash
ncd                   # cd to whichever vault is active here
ncd --vault ~/notes   # or name one; every global flag is forwarded
```

It has to be a shell function rather than an `ntropy cd` subcommand, because a process cannot change its parent shell's working directory. An unresolvable vault leaves the shell where it was. Nothing installs the file for you; source it yourself.
