---
title: Templates and daily notes
tags: [docs/notes]
site:
  order: 1
---
Every new note starts from a template, so it begins with the frontmatter
and skeleton it should have. Define the shape of a meeting note or a
book review once, and `ntropy new` stamps it out for you. `ntropy today`
builds a daily note the same way.

## Make a template

A template is a Markdown file with frontmatter in
`<vault>/.ntropy/templates/`, and its file name without `.md` is the
template's name:

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

```bash
ntropy new Standup --template meeting   # uses meeting.md
ntropy new Some thought                  # uses default.md
```

The fields in it are ordinary frontmatter, described in
[Note format](01M28Q32ACTNYR0FBYG9CAZMKG-note-format.md).

`init` seeds a `default.md`, which `new` uses without `--template`. If
that file is missing, ntropy falls back to a built-in copy of it. A named
template that does not exist is an error, never a silent fallback, so a
typo cannot hand you the wrong shape. A template name may not be empty
or contain a path separator.

Give the file a lowercase `.md`. `ntropy info` lists no other file as a
template. `--template meeting` opens `meeting.md`, which finds
`Meeting.MD` only on a file system that ignores case, as macOS does by
default.

## Placeholders

The `{{...}}` placeholders are filled in when the note is created:

| Placeholder | Becomes |
|-------------|---------|
| `{{title}}` | the title you passed to `new` |
| `{{id}}` | the note's ULID |
| `{{date}}` | the creation date as `YYYY-MM-DD`, local time |
| `{{slug}}` | the slugified title |

Anything ntropy does not recognize is left untouched, so a stray
`{{mustache}}` in your prose survives.

Inside the frontmatter, substitution respects the YAML around it. A
placeholder that is the whole value of a line, as in `title: {{title}}`,
is written as a YAML-safe value and quoted when it needs it. A title like
`Q3: Planning kickoff` or `[draft] roadmap` therefore does not break the
file. A placeholder inside a quoted string is escaped for that quote
style. In the body, substitution is verbatim.

## Daily notes with today

`init` also seeds a `today.md` template, which `ntropy today` uses:

```markdown
---
title: {{date}}
tags: [daily]
---
# {{date}}
```

`ntropy today` opens today's note, the note whose title is today's local
date. The first run of a day creates it from this template, and later
runs open the same note. If several notes carry today's date as their
title, the newest one opens. Edit `today.md` to shape your daily note.
The template has to exist; if it is missing, `ntropy init` on the vault
seeds it again.

## Skipping the template

`ntropy new --empty <title>` creates the file with no content in it. ntropy
still picks the ULID, the location, and the filename; what goes inside is
yours to write. `--template` is rejected alongside `--empty`. This mode is
mainly for scripts and agents, which would otherwise have to parse and
rewrite around a stamped skeleton. Until frontmatter lands in the file it is
not a well-formed note, so ntropy skips it with a warning meanwhile.

`ntropy write` is the other half of that. It takes a note's whole text on
stdin, refuses it if it is not a well-formed note, and stores it, then
realigns the filename and refreshes the views for you and prints the
resulting path. The two together author a note without ever reading one
back, and without an editor, in a plaintext and an encrypted vault alike:

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

`write` names its target (a full ULID, the note's filename, or its path in
`all-notes/`) and never searches for it, so it opens no picker and asks no
question.
