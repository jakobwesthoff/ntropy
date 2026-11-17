---
title: Agent skill
tags: [docs/integrate]
site:
  order: 2
---
LLM coding agents can drive ntropy well, since the CLI is fully
scriptable, but they have to know the house rules. The repository ships
an agent skill that teaches them.

## Install

The skill is compiled into ntropy, so an agent reads the skill of the
ntropy that runs. `ntropy skill` prints it, and `ntropy skill --install`
writes it where your agent loads skills from. For Claude Code, every
project at once:

```bash
ntropy skill --install ~/.claude/skills
```

This writes `~/.claude/skills/ntropy/`. For one project only, install into
`<project>/.claude/skills`. The install replaces the copy's `references/`
directory and leaves any other file in `ntropy/` alone.

The skill's description marks it as relevant to any task involving
ntropy, a note vault, or `.ntropy-vault` files, so an agent with the
skill installed picks it up on its own the moment a task touches one.

## What the skill teaches

[`skills/ntropy/`](https://github.com/jakobwesthoff/ntropy/tree/main/skills/ntropy)
holds a `SKILL.md` with the vault model, rules for agents, a do/don't
table, and a command reference. Beside it are reference docs on writing
notes, querying, vaults, views, the website export with its navigation
settings, site themes, and document themes. `ntropy skill <part>` prints
one of them: `writing-notes`, `querying`, `vaults`, `views`, `site`,
`site-themes` or `document-themes`.

The rules are the part an agent gets wrong on its own:

- Always pass `-n`, and `--print` (or `--id`) on `new` and `today`.
  Without them ntropy opens a picker or the editor and blocks whenever a
  terminal exists; capturing the output does not prevent that.
- Never hand-create files in `all-notes/`. ntropy allocates the ULID,
  the location, and the file name; only the content is the agent's.
- Author a note with `ntropy new --empty -p <title>` followed by
  `ntropy write <path>`, feeding the whole note on stdin.
- Run `ntropy reconcile` after editing a note file directly instead of
  through `write`.
- Check the active vault with `ntropy info` before changing anything,
  never write into the view directories, and never store `id` or dates
  in frontmatter.
