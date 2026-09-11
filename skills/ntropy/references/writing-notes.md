---
name: writing-notes
description: >-
  How to create and author well-behaving ntropy notes: the creation workflow,
  frontmatter rules, tags, inter-note links, templates, and when to reconcile.
metadata:
  tags: notes, frontmatter, tags, links, templates, reconcile, authoring
---

# Writing notes

A note is a Markdown file with a YAML frontmatter block, stored flat as
`all-notes/<ulid>-<slug>.md`. The leading 26-character ULID is the note's
identity; the slug is a readable echo of the title. Filing happens through
metadata, not folders: put a note's dimensions (tags, status, project, …) in
frontmatter and let queries and views do the organizing.

## Creating a note (the only correct way)

ntropy owns the parts you must not invent: the ULID, the location and the
filename. You own the content. `new` hands you the first, and `write` takes the
second:

```bash
path=$(ntropy new --empty -p Refactor the parser)   # trailing args join into the title
ntropy write "$path" <<'EOF'
---
title: Refactor the parser
tags: [engineering, refactor]
status: in progress
---
# Refactor the parser

Why, and what changes.
EOF
```

That is the whole flow. No read-back, no template to work around, no
`reconcile` afterwards, and it is the same in an encrypted vault as in a
plaintext one.

`--print` (short form `-p`) makes `new` print the path instead of opening an
editor. `--empty` makes it stamp no template; `--template` is rejected alongside
it, since one stamps content and the other writes none.

**FORBIDDEN: hand-creating files in `all-notes/`.** You would have to invent a
ULID; a wrong or duplicate one corrupts note identity.

### What `write` accepts

The target is named, never searched for. All three of these reach the same note:

```bash
ntropy write 01J8ZA2ABCDEFGHJKMNPQRSTVW < note.md              # full ULID
ntropy write 01J8ZA2ABCDEFGHJKMNPQRSTVW-q3-planning.md < note.md   # filename
ntropy write /vault/all-notes/01J8ZA2ABCDEFGHJKMNPQRSTVW-q3-planning.md < note.md
```

The note must already exist, and a path must point into the vault's
`all-notes/`. Nothing is prompted for and no picker opens, so `write` is safe in
a pipeline; a target naming no note, or more than one, is an error.

**Feed it the whole note, not a fragment.** The text you pipe in becomes the
file, so the "Frontmatter rules" below are your contract:

- The text **starts** with `---`, the YAML block, and a closing `---`. Anything
  without a frontmatter block is refused and nothing is written.
- `title` is required, and should be the title you passed to `new` — the
  filename slug was derived from it. A different title is not an error; `write`
  realigns the filename to it and prints the new path.
- `tags`, if present, is a flat list of strings; no `id`, `created` or
  `modified` fields; YAML-quote any value containing `: ` or starting with YAML
  syntax.
- Then the Markdown body, conventionally opening with `# <title>`.

**Never leave an `--empty` note unwritten.** Until frontmatter lands in it,
every ntropy command warns about it (`--strict` fails outright) and no search
returns it. Write it in the same step you created it.

### Rewriting an existing note

`search -P` and `write` are inverses, and both behave the same whether or not
the vault is encrypted:

```bash
ntropy search -n -P 01J8ZA2ABCDEFGHJKMNPQRSTVW > /tmp/note.md
# edit /tmp/note.md
ntropy write 01J8ZA2ABCDEFGHJKMNPQRSTVW < /tmp/note.md
```

`write` replaces the note's entire content, so send back the whole file, not
just the part you changed.

### Starting from a template instead

When you want the vault's own skeleton — a meeting note whose shape the user
defined — create from the template and edit the file directly:

```bash
path=$(ntropy new --print -t meeting Standup)
# edit "$path" with your file tools
ntropy reconcile
```

**MUST run `ntropy reconcile` after directly editing a note's frontmatter or
title.** ntropy realigns automatically when it launched the editor itself and
when you go through `ntropy write`. After any other out-of-band edit the
filename slug can drift from the title and the materialized views go stale until
reconcile runs. It is cheap and idempotent; when in doubt, run it.

### YAML-special titles

`ntropy new` quotes or escapes the title as needed when it substitutes into
the template's `title: {{title}}` line, so a title with `: ` inside it or a
leading YAML syntax character (`[`, `{`, `#`, `-`, `&`, `*`, `"`, `'`) creates
successfully:

```bash
ntropy new --print "Q3: Planning kickoff"   # OK: frontmatter is quoted
ntropy new --print "[draft] roadmap"        # OK: frontmatter is quoted
```

This covers template placeholder substitution into frontmatter. When you
write frontmatter by hand — a custom field in a note, or a template of your
own — the usual YAML rule applies: quote any value that contains `: ` or
starts with YAML syntax. Interior quotes in an otherwise plain value
(`He said "go"`) are fine unquoted.

### Retitling a note

Never rename the file. Edit the frontmatter `title`, then run
`ntropy reconcile`: it renames the file to the new slug (the ULID stays, so
identity and inbound links survive) and rewrites the slug portion of links in
other notes.

## Frontmatter rules

```markdown
---
title: Q3 Planning
tags: [work, planning, area/roadmap]
status: in progress
due: 2026-07-01
---
# Q3 Planning

Body is ordinary Markdown.
```

- **`title` is required** and canonical: full case, punctuation, Unicode. A
  note without one is malformed (skipped with a warning; an error under
  `--strict`).
- **`tags` is a flat YAML list of strings.** A `/` inside a tag denotes
  hierarchy: `area/roadmap` is one tag with two levels, understood by both
  queries and views. Tags are lowercase-normalized, so `Rust` and `rust` are
  the same tag. Never nest YAML structures inside `tags`.
- **Every other field is free** (`status`, `project`, `author`, anything) and
  becomes queryable and view-groupable just by existing. Unknown fields are
  preserved untouched when ntropy rewrites a note.
- **NEVER store `id`, `created`, or `modified` in frontmatter.** Identity is
  the filename ULID; the creation date is derived from it. Duplicating them
  creates state that can drift.
- **`site` is reserved.** A `site` mapping (`order`, `label`, `hidden`,
  `index`, `listing`, `related`) shapes the exported website's navigation
  and is hidden from the page; it is not a place for content. See
  [site.md](site.md).

For clean vaults, reuse field names and values consistently: `field:value`
queries match exactly (case-sensitive), and views group per distinct value, so
`status: done` and `status: Done` behave as different query values even though
a view folds them into one normalized directory. Pick one spelling and stick to
it. Before inventing a new tag or field value, check what the vault already
uses: `ntropy tags -n` lists every tag with its count.

## Linking between notes

Links are ordinary Markdown links whose target is the note's filename, nothing
custom:

```markdown
See [the Q3 plan](01j8za2abcdefghjkmnpqrstvw-q3-planning.md) for the numbers.
```

Get the filename from a search (`ntropy search -n tag:planning`, PATH column)
and use its basename. A link is recognized by its leading 26-character ULID, so
it survives retitles: `ntropy reconcile` rewrites the slug portion of stale
link targets after a rename (links inside fenced or inline code are left
alone). DO NOT link by title or by relative folder paths into views.

## Templates

`ntropy new` stamps a note from a template in `<vault>/.ntropy/templates/`
unless `--empty` says otherwise; the filename minus `.md` is the template name:

```bash
ntropy new --print Standup --template meeting   # uses .ntropy/templates/meeting.md
ntropy new --print Some thought                 # uses default.md
```

A named template that does not exist is an error, never a silent fallback.

Author a template by writing the file; placeholders are substituted at creation
time and anything unrecognized is left untouched:

| Placeholder | Becomes |
|-------------|---------|
| `{{title}}` | the title passed to `new` |
| `{{id}}`    | the note's ULID |
| `{{date}}`  | creation date, `YYYY-MM-DD`, local time |
| `{{slug}}`  | the slugified title |

```markdown
<!-- .ntropy/templates/meeting.md -->
---
title: {{title}}
date: {{date}}
tags: [meeting]
status: notes
---
# {{title}}

## Attendees

## Notes

## Action items
```

Keep `title: {{title}}` in custom templates. The filename slug is derived from
the title passed on the command line, so a template that hardcodes a different
title creates immediate slug drift that the next reconcile has to rename away.

## Daily notes

`ntropy today --print` prints the path of today's note, creating it from
`.ntropy/templates/today.md` on first use each day (the note is identified by
its title being today's date). Shape daily notes by editing that template. It
must exist; a vault predating it just needs `ntropy init <vault>` re-run, which
seeds missing pieces without touching existing ones.

## Attachments

Non-`.md` files and subdirectories inside `all-notes/` are ignored by ntropy,
so images and other attachments live right next to the notes and are referenced
with ordinary relative Markdown syntax (`![diagram](diagram.png)`). Only
inter-note links get the ULID-based refresh treatment.

## Slug and normalization facts

Knowing what filenames will look like helps you predict paths: slugs are
lowercase ASCII with `-` for whitespace, German-aware transliteration
(`ä→ae`, `ö→oe`, `ü→ue`, `ß→ss`), other non-ASCII best-effort transliterated or
dropped, capped near 72 characters, and `untitled` when nothing survives. The
full title always remains in frontmatter, so never shorten a title for the
filename's sake. Because the millisecond-precise ULID leads every filename, a
plain lexical sort of `all-notes/` is chronological.

## Well-behaved note checklist

- Created via `ntropy new`, never hand-placed in `all-notes/`.
- Content supplied through `ntropy write`, or edited in the file and followed by
  `ntropy reconcile`.
- No file left empty: an `--empty` note is written in the same step it is created.
- Frontmatter has `title`; `tags` is a flat string list; no `id`/date fields.
- YAML values containing `: ` or starting with YAML syntax are quoted.
- Field names and values reused consistently with the rest of the vault.
- Links target `<ulid>-<slug>.md` filenames.
- `ntropy reconcile` run after any direct edit to frontmatter or title (`write`
  does this itself).
