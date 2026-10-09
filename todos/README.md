# Todos

This directory is an [ntropy](https://ntropy.westhoffswelt.de) vault
holding the deferred work of ntropy: bugs, features, refactors and
open questions. Each todo is one Markdown note in `all-notes/`, named
`<ULID>-<slug>.md`. The ULID is the todo's permanent identity.
`by-component/` and `by-tag/` are views, derived symlink trees that git
ignores.

Run every command with the vault pinned and `-n`, from anywhere in the
repository. There is no `.ntropy-vault` pointer file on purpose, so
ntropy commands for other vaults keep working inside this repository.

## Format

```yaml
---
title: "Settings watcher drops a write during reload"
kind: bug
component: view
status: blocked     # requires a Depends on or Blocked by line
impact: high
horizon: next
origin: review
tags: [concurrency]
---
```

- `title` (required): always double-quoted. It names the concrete subject
  and the change or problem, in sentence case; no prefix such as a kind,
  component or ticket label goes before it. The H1 repeats it.
- `kind` (required): `bug` (wrong today, even if unconfirmed),
  `feature` (new capability), `improvement` (better for users or
  operators, not wrong now), `refactor` (restructures code, no
  behaviour change), `docs`, `chore` (no behaviour or logic change:
  dependencies, build, CI, comments, manual release checks),
  `investigation` (a question to settle by research or by the user's
  decision), `plan` (coordinates todos that point at it). When several
  fit, take the first of bug, feature, improvement, refactor, docs,
  chore; `plan` and `investigation` win over those, and a plan wins over
  an investigation.
- `component` (required): one value from the list below.
- `status`: `needs-discussion` (the direction is unsettled; discuss
  before anyone works on it) or `blocked` (waits on another todo or an
  outside event). Absent means ready. `needs-discussion` wins when both
  apply.
- `impact`: `critical` (loses data, breaks core use, or an exploitable
  security hole), `high` (degrades a core feature, fails often, or
  hardens a security boundary), or `low` (polish). Absent means normal.
- `horizon`: `next` or `someday`. Absent means normal backlog. A
  `someday` todo ends with a `## Revisit when` section naming what
  brings it back, or "No trigger known; reconsider at the next sweep."
- `origin`: the first that fits of `incident`, `review`, `upstream`,
  `implementation`, `discussion`, `request`, `idea`; absent means not
  recorded.
- `tags`: topics from the list below.

Values are written exactly as listed. Defaults are never written: no
`status: ready`, no `impact: normal`, no `horizon: backlog`. Never add
`id`, `created`, `modified` or another date field; the ULID carries the
creation time.

The body opens with a paragraph that stands alone. Relations to other
todos go in a final `## Relations` section, one line per target:
*Depends on*, *Part of* and *Relates to* link `[Title](<ULID>-<slug>.md)`,
where the filename is
`basename "$(ntropy --vault "$VAULT" search -n -p <ULID>)"` and the
target must already exist; a todo in another repository is written as
plain text, `<repository> todo <ULID> (<title>)`. *Blocked by* names an
outside event in plain text. A relation is written on the dependent
side only: the todo that depends on, is blocked by, or is part of
another carries the line; *Relates to* goes on the todo with the larger
ULID; a plan never lists its steps. A todo with a *Depends on* line to
an existing todo, or a *Blocked by* that still holds, has
`status: blocked`. Code comments and docs refer to a todo by its ULID
alone.

## Components

- `note`: the note model: frontmatter, filenames, ULIDs, dates, slugs,
  tags and link extraction (`src/note/`, `src/text/`, `src/link/`,
  `src/id.rs`, `src/datetime.rs`)
- `vault`: vault resolution, configuration, scanning, filesystem
  utilities, the ops layer and the library's public API (`src/vault/`,
  `src/config/`, `src/ops/`, `src/scan.rs`, `src/fsutil.rs`,
  `src/session.rs`, `src/error.rs`, `src/lib.rs`)
- `query`: the query language: tokenizer, parser and evaluation
  (`src/query/`)
- `view`: materialized views, reconcile and the vault `.gitignore`
  (`src/view/`, `src/reconcile.rs`, `src/gitignore.rs`)
- `render`: the Typst and HTML render engines (`src/render/`)
- `site`: the static site export and its theme (`src/site/`)
- `crypto`: vault encryption and key handling (`src/crypto/`,
  `src/cipher.rs`, `src/keys/`)
- `cli`: the `ntropy` binary: commands, picker, editor launch and output
  (`src/bin/ntropy/`, except the language server)
- `lsp`: the language server (`src/bin/ntropy/run/lsp/`)
- `project`: build, CI, release, packaging, README, the website, the
  agent skill and product decisions; only when no other component fits

## Tags

- `security`: a finding that an untrusted vault, note or input can
  exploit, or hardening against one
- `performance`: speed or size, for users or in generated output
- `concurrency`: races between processes or threads touching the vault
- `error-handling`: wrong, misleading or missing errors, and panics
- `durability`: data surviving crashes and power loss
- `ux`: how the CLI, picker or output feels to use
- `windows`: Windows support
- `testing`: the test suite itself
- `ci`: GitHub Actions workflows
- `api-design`: the shape of the library's public API
- `encryption`: work on encrypted vaults that lands outside the `crypto`
  component

## Filing and editing a todo

Never create a note file in `all-notes/` by hand: the ULID must come
from ntropy. Compose the whole note first, including the filenames of
the todos it links, then let ntropy allocate it and write it in the same
Bash invocation, since `new` leaves an empty note until `write` runs.
The title goes to `new` in single quotes (an apostrophe inside becomes
`'\''`, as in `'Don'\''t drop writes'`) and into the frontmatter in
double quotes:

```bash
VAULT="$(git rev-parse --show-toplevel)/todos"
note=$(ntropy --vault "$VAULT" new -n --empty -p 'Settings watcher drops a write during reload')
ntropy --vault "$VAULT" write -n "$note" <<'EOF'
---
title: "Settings watcher drops a write during reload"
kind: bug
component: view
---
# Settings watcher drops a write during reload

What is wrong, where, and why it matters.
EOF
```

If `write` fails, fix the text and run `write` again on the same note.
To edit a todo, edit the file `ntropy --vault "$VAULT" search -n -p <ULID>`
prints, then run `ntropy --vault "$VAULT" reconcile -n`. Before
committing, `ntropy --vault "$VAULT" reconcile -n --strict` must exit 0.

## Finding work

```bash
VAULT="$(git rev-parse --show-toplevel)/todos"
ntropy --vault "$VAULT" search -n 'not status:blocked and not status:needs-discussion and not horizon:someday and not kind:plan'
ntropy --vault "$VAULT" search -n status:needs-discussion
ntropy --vault "$VAULT" search -n 'impact:critical or impact:high'
ntropy --vault "$VAULT" search -n component:query
ntropy --vault "$VAULT" search -n -P '<ULID>'     # print one todo
```

## Closing a todo

A finished todo is deleted in the commit that finishes the work, or,
when the work is already committed, in a commit of its own that names
those commits by short SHA. Before deleting it:

1. Find every reference: `ntropy --vault "$VAULT" search -n "text:<ulid>"`
   for other todos, and `rg -il --hidden --glob '!.git' <ulid>` in the
   repository.
2. Remove its *Depends on* line from every todo that depends on it, and
   their `status: blocked` when no blocker line remains. If the todo is
   dropped rather than done, re-evaluate each dependent as well: drop
   it, rewrite it, or set `status: needs-discussion` with a Decisions
   line naming the dropped prerequisite. Update or remove other links
   and the docs that mention it.
3. If it has a *Part of* line, update its plan's Steps prose; when no
   other todo has a *Part of* line to that plan, close the plan too.
4. An investigation's findings go into an ADR, the docs or the todos it
   produced; if none fits, ask whether to keep them at all.
5. Delete it with `ntropy --vault "$VAULT" delete -n -f <ULID>`.

Git history is the archive. Work that landed only partly gets the todo
rewritten to cover what remains instead.
