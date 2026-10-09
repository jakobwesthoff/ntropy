# `ntropy skill`: ship the agent skill inside the binary

## Why

The agent skill lives in the repository under `skills/ntropy/`, and the
documented ways to install it both go through GitHub
(`docs/website/all-notes/01M28Q32NS466VK7NWS6BFB2YP-agent-skill.md`):
`npx skills add jakobwesthoff/ntropy`, or cloning the repository and
copying `skills/ntropy` into a skills directory by hand. Either way the
installed skill reflects the repository's current `main`, not the ntropy
version on the machine, so an agent can be taught flags or commands its
binary does not have, or miss ones it does.

asqr solves the same problem with an `asqr skill` command (asqr 0.9.1,
`src/cli/skill.rs`). The skill is compiled into the binary with
`include_str!`, so "the installed skill always matches the installed
asqr". `asqr skill` prints it, and `asqr skill --install <DIR>` writes it to
`<DIR>/asqr/SKILL.md`, for example `asqr skill --install .claude/skills`.
That works well in practice and fits ntropy just as well.

## Scope

A `skill` subcommand on the `ntropy` binary, modelled on asqr's:

- `ntropy skill` prints the skill.
- `ntropy skill --install <DIR>` writes it under `<DIR>/ntropy/`, e.g.
  `ntropy skill --install ~/.claude/skills`, and prints the path it
  installed to.
- Neither needs a vault. The command works outside one and ignores vault
  resolution.

## What differs from asqr

asqr's skill is a single `SKILL.md`. ntropy's is a tree: `SKILL.md` plus
six files under `references/` (`writing-notes.md`, `querying.md`,
`vaults.md`, `views.md`, `site.md`, `site-themes.md`), linked from
`SKILL.md` by relative paths such as `references/site.md`. That shapes
most of the open questions below.

## Open questions

- **Embedding the tree.** ADR 0039 already embeds the vault seed content
  as real files with `include_str!` and writes them by iterating a
  `SEEDED_FILES` manifest (`src/vault/seed.rs`, `src/ops/init.rs`). The
  same pattern fits here. A test that compares the manifest with the
  contents of `skills/ntropy/` would catch a new reference file that was
  added to the directory but not to the binary.
- **What printing means.** Printing only `SKILL.md` leaves its
  `references/...` links pointing nowhere. Options: print `SKILL.md`
  alone and accept that; print the whole tree concatenated with file
  markers; or take an optional argument naming one file, e.g.
  `ntropy skill references/querying.md`.
- **Reinstalling over an older version.** asqr overwrites its one file.
  With a tree, a reference removed in a newer ntropy would linger from the
  old install. Replacing the whole `<DIR>/ntropy/` directory fixes that,
  but deletes anything a user added there. Decide which, and whether
  either needs a `--force` when the directory already exists.
- **Licence header.** The skill files carry no MPL-2.0 header, and
  `skills/` is not among the header exceptions listed in `CLAUDE.md`
  (seed content, generated frontend output, theme fonts and icons).
  Embedding them makes them part of the build, so `skills/` belongs on
  that list, perhaps with a unit test like the one `vault::seed` has.
- **Docs.** The agent skill page should lead with
  `ntropy skill --install`, keep `npx skills add` and the manual copy as
  alternatives, and the skill's own command reference should list
  `skill`.
- **ADR.** A new command changes the CLI surface of ADR 0018 and wants an
  amendment or a new ADR.
