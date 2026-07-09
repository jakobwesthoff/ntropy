# 37. Render command surface

Date: 2026-07-09

## Status

Accepted

Extends the command surface of
[ADR 0018](0018-cli-command-surface.md) as amended by
[ADR 0031](0031-merge-edit-into-search.md) and
[ADR 0035](0035-generic-print-flag-replaces-no-edit.md). Ambiguity handling
follows [ADR 0025](0025-plain-output-format-ordering-and-edit-ambiguity.md);
interactivity follows
[ADR 0036](0036-interactivity-keyed-to-the-controlling-terminal.md). The
engine behind the command is
[ADR 0038](0038-pluggable-rendering-engine-with-pandoc-and-typst.md).

## Context

Notes only exist as Markdown files; ntropy has no command that produces a
derived-format artifact, so sharing a note as a typeset document means
leaving ntropy. The name `view` is already taken by the materialized
symlink views (ADR 0008), so a document-producing command needs its own
verb.

## Decision

A new top-level `render` subcommand:

    ntropy render [id|query] [--to <format>] [--engine <name>] [-o <path>] [-p]

`render` is chosen over `export`: it matches the rendering-engine naming
and emphasizes the transformation.

- The selector follows the id-or-query rule shared with `search` and
  `delete`. Like `search`, it is optional: omitted, the whole vault feeds
  the picker for fuzzy selection.
- One invocation renders exactly one note. Like `delete`, several matches
  open the picker pre-filtered interactively; under `-n` an ambiguous
  selector errors with the candidate list, and a bare invocation with more
  than one note asks for a selector.
- `--to` names the output format and defaults to `pdf`. `--engine`
  overrides the format's default engine; with only the `pandoc` engine in
  v1 it accepts a single value, and exists so invocations written today
  keep working when alternative engines arrive.
- `--output`/`-o` names the artifact; the default is `./<slug>.pdf`, from
  the slug component of the note's filename. An existing file at the
  target is overwritten.
- `--print`/`-p` prints the artifact's path to stdout as one line on
  success, extending the flag of ADR 0035 to a command that produces a
  file instead of opening the editor. Without it, nothing is written to
  stdout.
- Scan warnings print to stderr and fail the command under `--strict`,
  matching `search`.
- `render` is read-only: no filename realignment and no view refresh.

## Consequences

- The command surface grows by one read-only command; the mutation
  commands are unchanged.
- The selector plumbing (`ops::resolve_selection`) and the generic picker
  serve a third command without modification.
- `open "$(ntropy render -p <selector>)"` composes, in line with stdout as
  a pure data channel (ADR 0036).
- `--engine` is a flag with exactly one legal value until a second engine
  ships.
