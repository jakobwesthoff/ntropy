# 92. Ship the agent skill in the binary and print it with ntropy skill

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0018](0018-cli-command-surface.md), whose command surface gains
`skill`.

## Context

AI coding agents run ntropy for the people they work with. An agent needs to
know how ntropy works, and that knowledge has to match the ntropy that is
installed. A skill taken from the repository's `main`, through `npx skills
add` or by copying the directory, can teach flags or commands the installed
binary does not have, or leave out ones it does.

## Decision

ntropy ships an agent skill in the Agent Skills format at `skills/ntropy/`:
a core, `SKILL.md`, and seven parts under `references/`: `writing-notes`,
`querying`, `vaults`, `views`, `site`, `site-themes` and `document-themes`.
The core opens by telling the agent to read it to its last line, names each
part by its file and by its command, and ends with `End of the ntropy
skill.`. Each part opens with a line that sends the reader to the core and
ends with its own end line.

The skill is compiled into the binary. `ntropy skill` prints the core,
`ntropy skill <part>` prints one part, and `ntropy skill --install <dir>`
writes `<dir>/ntropy/SKILL.md` and every part to `<dir>/ntropy/references/`,
naming each file on stdout as it is written. `--install` replaces
`references/` whole and leaves any other file in `<dir>/ntropy/` alone. An
unknown part is a usage error that lists the parts. `skill` needs no vault
and resolves none.

The frontmatter of `SKILL.md` carries `metadata.ntropy-version`, the crate
version. A test fails until it matches, so every copy of the skill says which
ntropy it belongs to. The stamp is the only version reference in the skill,
which documents the current CLI.

The docs name `ntropy skill --install` as the way to install the skill.

The skill files carry no MPL header: they are copied verbatim into agents'
skills directories. `skills/` joins the exceptions listed in `CLAUDE.md`, and
a unit test in `skill` enforces it.

## Consequences

The published crate carries `skills/`, which the binary embeds.

Raising the crate version fails the stamp test until `ntropy-version`
follows.
