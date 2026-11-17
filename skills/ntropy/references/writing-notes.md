Part of the ntropy skill. Read SKILL.md first, which holds the rules for every task; `ntropy skill` prints it.

# Writing notes

A note is a Markdown file with a YAML frontmatter block, stored flat as
`all-notes/<ulid>-<slug>.md`. The leading 26-character ULID is the note's
identity; the slug is a readable echo of the title. Filing happens through
metadata, not folders: put a note's dimensions (tags, status, project, …) in
frontmatter and let queries and views do the organizing.

## Creating a note

The core's rules 3 and 4 are the whole flow: `new --empty -p`, then `write` the
whole note. This section holds what they leave out.

Trailing arguments of `new` join into the title. `--print` (short form `-p`)
makes `new` print the path instead of opening an editor. `--id` prints the new
note's ULID instead, for a script that refers to the note by it
(`id=$(ntropy new -n --id -- "Quarterly review")`); it conflicts with `-p`.
`--empty` makes it stamp no template; `--template` is rejected alongside it,
since one stamps content and the other writes none.

### What `write` accepts

The target is named, never searched for. All three of these reach the same note:

```bash
ntropy write -n 01J8ZA2ABCDEFGHJKMNPQRSTVW < note.md              # full ULID
ntropy write -n 01J8ZA2ABCDEFGHJKMNPQRSTVW-q3-planning.md < note.md   # filename
ntropy write -n /vault/all-notes/01J8ZA2ABCDEFGHJKMNPQRSTVW-q3-planning.md < note.md
```

The note must already exist, and a path must point into the vault's
`all-notes/`, directly or through a symlink such as a view entry. Nothing is
prompted for and no picker opens, so `write` is safe in a pipeline. A target
naming no note is an error, and so is one naming more than one: a ULID that a
copy shares names both files, so pass the filename of the one you mean (or run
`ntropy reconcile` first, as the core's rule on copies says).

**Feed it the whole note, not a fragment.** The text you pipe in becomes the
file, so the "Frontmatter rules" below are your contract:

- The text **starts** with `---`, the YAML block, and a closing `---`. Anything
  without a frontmatter block is refused and nothing is written.
- `title` is required, and should be the title you passed to `new` — the
  filename slug was derived from it. A different title is not an error; `write`
  realigns the filename to it and prints the new path.
- `tags`, if present, is a flat list of strings (a number or boolean reads as
  its text); no `id`, `created` or
  `modified` fields; YAML-quote any value containing `: ` or starting with YAML
  syntax.
- Then the Markdown body, conventionally opening with `# <title>`.

### Rewriting an existing note

`search -P` and `write` are inverses, and both behave the same whether or not
the vault is encrypted:

```bash
ntropy search -n -P 01J8ZA2ABCDEFGHJKMNPQRSTVW > /tmp/note.md
# edit /tmp/note.md
ntropy write -n 01J8ZA2ABCDEFGHJKMNPQRSTVW < /tmp/note.md
```

`write` replaces the note's entire content, so send back the whole file, not
just the part you changed.

### Starting from a template instead

When you want the vault's own skeleton — a meeting note whose shape the user
defined — create from the template and edit the file directly:

```bash
note=$(ntropy new -n --print -t meeting Standup)
# edit "$note" with your file tools
ntropy reconcile -n
```

The reconcile is the core's rule on direct edits: after this edit the filename slug can drift from
the title and the materialized views go stale until it runs.

### YAML-special titles

`ntropy new` quotes or escapes the title as needed when it substitutes into
the template's `title: {{title}}` line, so a title with `: ` inside it or a
leading YAML syntax character (`[`, `{`, `#`, `&`, `*`, `"`, `'`) creates
successfully:

```bash
ntropy new -n --print "Q3: Planning kickoff"   # OK: frontmatter is quoted
ntropy new -n --print "[draft] roadmap"        # OK: frontmatter is quoted
```

A title that starts with `-` is read by the command line as a flag, and `new`
fails before it creates anything: put `--` before the title
(`ntropy new -n -p -- "-1 day"`); the quoting in the frontmatter is then
automatic.

This covers template placeholder substitution into frontmatter. When you
write frontmatter by hand — a custom field in a note, or a template of your
own — the usual YAML rule applies: quote any value that contains `: ` or
starts with YAML syntax. Interior quotes in an otherwise plain value
(`He said "go"`) are fine unquoted.

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
  `--strict`). A number or boolean title reads as its text, and YAML does not
  keep how a number was written (`1.50` reads as `1.5`), so quote a title whose
  spelling matters. A list or mapping title is malformed too.
- **`tags` is a flat YAML list of strings** (numbers and booleans read as their
  text, `1.50` as `1.5`; quote a tag whose spelling matters). A `/` inside a tag denotes
  hierarchy: `area/roadmap` is one tag with two levels, understood by both
  queries and views. Tags are lowercase-normalized, so `Rust` and `rust` are
  the same tag; punctuation stays (`milestone/0.13`, `c++`, `node.js`). Never
  nest YAML structures inside `tags`: a list or mapping entry makes the note
  malformed.
- **Every other field is free** (`status`, `project`, `author`, anything) and
  becomes queryable and view-groupable just by existing. Unknown fields are
  preserved untouched when ntropy rewrites a note.
- **NEVER store `id`, `created`, or `modified` in frontmatter.** Identity is
  the filename ULID; the creation date is derived from it. Duplicating them
  creates state that can drift.
- **`site` is reserved.** A `site` mapping (`order`, `label`, `hidden`,
  `index`, `listing`, `related`, `template`) shapes the exported website's navigation
  and is hidden from the page; it is not a place for content. See
  [site.md](site.md), or `ntropy skill site`.

For clean vaults, reuse field names and values consistently: `field:value`
queries match values exactly (case-sensitive, the field name ignores case),
and views group per distinct value, so
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

To link a heading of the other note, add its id after a `#`:
`[Inside tmux](01j8za2abcdefghjkmnpqrstvw-q3-planning.md#inside-tmux)`. The id
follows GitHub's rule: lowercase, each space a hyphen (not collapsed, so
`Pros & Cons` is `pros--cons`), punctuation other than `-` and `_` dropped,
letters of any script kept (`Über` is `über`, not the filename's `ueber`); a
repeated heading gets `-1`, `-2`. When unsure, export with `ntropy site`: it
warns when no heading has the id. `reconcile` keeps the `#...` part.

Get the filename from a search (`ntropy search -n tag:planning`, PATH column)
and use its basename. A link is recognized by its leading 26-character ULID, so
it survives retitles: `ntropy reconcile` rewrites the slug portion of stale
link targets after a rename (links inside code, whether inline, fenced or
indented, are left alone).

## Templates

`ntropy new` stamps a note from a template in `<vault>/.ntropy/templates/`
unless `--empty` says otherwise; the filename minus `.md` is the template name.
Name template files with a lowercase `.md`: `ntropy info` lists no other file
as a template, and `new --template` opens `<name>.md`:

```bash
ntropy new -n --print Standup --template meeting   # uses .ntropy/templates/meeting.md
ntropy new -n --print Some thought                 # uses default.md
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

`ntropy today -n --print` prints the path of today's note, creating it from
`.ntropy/templates/today.md` on first use each day (the note is identified by
its title being today's date). `ntropy today -n --id` prints its ULID instead.
Shape daily notes by editing that template. It
must exist; a vault predating it just needs `ntropy init <vault>` re-run, which
seeds missing pieces without touching existing ones.

## Assets

A note's files, images, transcripts and any other attachment live in
`all-notes/_assets/<note file name without extension>/`, a folder named like the
note. Put them there with ntropy:

- `ntropy asset add -n <id> <file…>` copies the files in (`--move` moves them)
  and prints one Markdown link per file on stdout, `![alt](_assets/<folder>/<file>)`
  for an image and `[name](...)` for any other file, with the target in angle
  brackets when the name needs them. `added` lines go to stderr. It never edits
  the note: place the link in the body yourself. `--as <name>` stores one file
  under another name, `--force` replaces a file of that name.
- `ntropy asset list -n <id>` lists the files with their size and whether the
  note links them, and `broken` for a link to a file the folder lacks. `--path`
  prints the folder, also when it does not exist yet.

Both select exactly one note as `render` does; several matches are an error
under `-n`. They refuse to work in an encrypted vault.

A note links only its own folder. A link into another note's folder is refused
by renders and exports and reported by `reconcile`, and `--strict` fails on it,
as it does on a link to a missing file or to a file outside `all-notes/_assets/`,
which still renders. After a retitle the folder keeps its old name until
`reconcile` renames it and rewrites the links into it. `delete` removes the
folder with the note. A `#` in a link target always starts the fragment, so a
file with a `#` in its name cannot be linked.

## Slug and normalization facts

Knowing what filenames will look like helps you predict paths: slugs are
lowercase ASCII with `-` for whitespace and for every other ASCII character
outside `a-z0-9-` (`Node.js` becomes `node-js`) except the apostrophe, which is
dropped (`Don't` becomes `dont`), German-aware transliteration
(`ä→ae`, `ö→oe`, `ü→ue`, `ß→ss`), other non-ASCII best-effort transliterated or
dropped, capped near 72 characters, and `untitled` when nothing survives. The
full title always remains in frontmatter, so never shorten a title for the
filename's sake. Because the millisecond-precise ULID leads every filename, a
plain lexical sort of `all-notes/` is chronological.

End of references/writing-notes.md.
