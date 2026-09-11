---
title: Templates and daily notes
tags: [docs/notes]
site:
  order: 3
---
Every new note starts from a template, so a note can begin with the
frontmatter and skeleton it should have instead of a blank file you furnish
by hand each time. Define the shape of a "meeting note" or a "book review"
once, and `ntropy new` stamps it out for you. This page covers where
templates live, the placeholders they can use, how to skip them, and the
`today` command built on top of them.

## Template files

Templates are Markdown-with-frontmatter files in `<vault>/.ntropy/templates/`.
The filename without `.md` is the template's name:

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

`init` seeds a `default.md`, used whenever you do not pass `--template`. If
that file is missing, ntropy falls back to a built-in copy of it. Asking for
a named template that does not exist is an error rather than a silent
fallback, so a typo never quietly hands you the wrong shape. A template name
may not be empty or contain a path separator.

## Placeholders

The `{{...}}` placeholders are filled in at creation time:

| Placeholder | Becomes |
|-------------|---------|
| `{{title}}` | the title you passed to `new` |
| `{{id}}` | the note's ULID |
| `{{date}}` | the creation date as `YYYY-MM-DD`, local time |
| `{{slug}}` | the slugified title |

Anything ntropy does not recognize is left untouched, so a stray
`{{mustache}}` in your prose survives intact.

Inside the frontmatter block, substitution respects the YAML around it. A
placeholder that is the whole value of a line, as in `title: {{title}}`, is
written as a YAML-safe scalar, quoted when the value needs it, so a title
like `Q3: Planning kickoff` or `[draft] roadmap` does not break the file. A
placeholder inside a quoted string is escaped for that quote style. In the
body, substitution is verbatim.

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
path=$(ntropy new --empty -p Quarterly review)
ntropy write "$path" <<'EOF'
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

## Daily notes with today

`init` also seeds a `today.md` template, which powers `ntropy today`:

```markdown
---
title: {{date}}
tags: [daily]
---
# {{date}}
```

`ntropy today` opens today's note, identified by its title being today's
local date. The first time you run it on a given day it creates the note
from this template; later runs reopen the same note. If several notes carry
today's date as their title, the newest one opens. Edit `today.md` to shape
your daily note however you like. The template has to exist; a vault created
before this feature can re-run `ntropy init` to seed it.
