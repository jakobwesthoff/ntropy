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

`ntropy new --empty <title>` creates the note's file with no content, for
a script or an agent that writes the whole note itself with
`ntropy write`. How the two work together is in
[Scripting and the shell](01M28Q32N56MS70ETPES4794HJ-scripting-and-the-shell.md).
