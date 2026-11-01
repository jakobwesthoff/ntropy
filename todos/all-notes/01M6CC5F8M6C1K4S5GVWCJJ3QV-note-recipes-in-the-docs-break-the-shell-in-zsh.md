---
title: "Note recipes in the docs break the shell in zsh"
kind: bug
component: project
impact: high
origin: review
---
# Note recipes in the docs break the shell in zsh

The website docs and the agent skill keep a new note's path in a shell
variable named `path`. In zsh, the default shell on macOS, `path` is tied
to `PATH`, so the assignment replaces the command search path and the very
next command fails. Found on 2026-11-01 in the review of the "Making a
front page" guide, reproduced with ntropy 2.9.0.

## Problem

```
$ zsh -c 'path=$(echo /tmp/x.md); ntropy --version'
zsh:1: command not found: ntropy
```

The same two lines work in bash. A reader on a Mac who copies the recipe

```bash
path=$(ntropy new --empty -p "Welcome")
ntropy write "$path" <<'NOTE'
```

gets "command not found: ntropy", and every later command in that shell
fails the same way until the shell is restarted. `git grep -n 'path='`
finds the pattern in `templates-and-daily-notes`, the new "Making a front
page", `skills/ntropy/SKILL.md` (several places),
`skills/ntropy/references/querying.md`, `site.md` and `writing-notes.md`.

## Suggested fix

Name the variable `note` everywhere, and check that no doc or skill
example assigns `path` (or another zsh special such as `fpath`,
`cdpath`, `manpath`).
