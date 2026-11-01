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

The quickest install is the [`skills` CLI](https://skills.sh), which
places the skill into Claude Code, Cursor, Codex, and most other
SKILL.md-aware agents. It installs into the project by default and for
your user with `-g`:

```bash
npx skills add jakobwesthoff/ntropy
```

Or skip the tooling and copy the directory by hand. For Claude Code it
belongs at `~/.claude/skills/ntropy` (every project) or
`<project>/.claude/skills/ntropy` (just that project):

```bash
git clone https://github.com/jakobwesthoff/ntropy.git
cp -R ntropy/skills/ntropy ~/.claude/skills/ntropy
```

The skill's description marks it as relevant to any task involving
ntropy, a note vault, or `.ntropy-vault` files, so an agent with the
skill installed picks it up on its own the moment a task touches one.

## What the skill teaches

[`skills/ntropy/`](https://github.com/jakobwesthoff/ntropy/tree/main/skills/ntropy)
holds a `SKILL.md` with the vault model, rules for agents, a do/don't
table, and a command reference. Beside it are reference docs on writing
notes, querying, vaults, views, the website export with its navigation
settings, site themes, and document themes.

The rules are the part an agent gets wrong on its own:

- Always pass `-n`, and `--print` on `new` and `today`. Without them
  ntropy opens a picker or the editor and blocks whenever a terminal
  exists; capturing the output does not prevent that.
- Never hand-create files in `all-notes/`. ntropy allocates the ULID,
  the location, and the file name; only the content is the agent's.
- Author a note with `ntropy new --empty -p <title>` followed by
  `ntropy write <path>`, feeding the whole note on stdin.
- Run `ntropy reconcile` after editing a note file directly instead of
  through `write`.
- Check the active vault with `ntropy info` before changing anything,
  never write into the view directories, and never store `id` or dates
  in frontmatter.
