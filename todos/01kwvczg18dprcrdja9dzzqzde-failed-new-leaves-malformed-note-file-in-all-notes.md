# Failed `new` leaves the malformed note file in all-notes/

When the title passed to `ntropy new` breaks the YAML produced by the
template's `title: {{title}}` substitution, the command errors but the note
file has already been written and stays in `all-notes/`. Every subsequent
command then skips it with a warning until it is deleted or repaired by hand.

Reproduced on 1.3.0:

    $ ntropy new --no-edit "Q3: Planning kickoff"
    error: while creating the note: the frontmatter is not valid YAML:
    mapping values are not allowed in this context at line 1 column 10
    $ ntropy search -n
    warning: skipped `01KWVCJEMRJFJTM45SHE8G1J96-q3-planning-kickoff.md`:
    the frontmatter is not valid YAML

Same pattern for other YAML-breaking titles: `"[draft] roadmap"` (invalid
YAML) and `"#hashtag first"` (title becomes a comment, so "no `title` field");
each failed run strands its file.

Two independent aspects to decide:

1. Cleanup: a `new` that fails validation should not leave the file behind
   (validate before writing, or remove on failure).
2. Ergonomics: `{{title}}` is substituted verbatim into frontmatter; YAML-
   escaping/quoting the substituted value would make such titles work instead
   of erroring at all.

Per the bug-fix workflow, start with a failing test (run `new` with a
YAML-breaking title, assert no stray file remains in `all-notes/`).
