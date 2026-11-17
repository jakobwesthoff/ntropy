# 93. Warn from ntropy skill and ntropy --version when an installed copy of the skill is older

Date: 2026-11-17

## Status

Accepted

Builds on [ADR 0092](0092-ship-the-agent-skill-in-the-binary-and-print-it-with-ntropy-skill.md).

## Context

`ntropy skill --install` copies the skill once (ADR 0092). A copy stays as it
was when ntropy is updated, and an agent runner loads the copy from its skills
directory in later sessions.

Agent runners read skills from directories of their own: `.agents/skills`,
`.claude/skills`, `.gemini/skills`, `.github/skills`, `.cursor/skills` and
`.codex/skills` in a project, and `~/.agents/skills`, `~/.claude/skills`,
`~/.gemini/skills`, `~/.copilot/skills`, `~/.cursor/skills` and
`~/.codex/skills` for a person. `CLAUDE_CONFIG_DIR`, `CODEX_HOME`,
`COPILOT_HOME` and `GEMINI_CLI_HOME` move some of the personal ones.

## Decision

`ntropy skill`, printing the core or a part, and `ntropy --version` compare
the `ntropy-version` of installed copies with their own version and warn when
a copy is older. No other command checks, and `skill --install` does not
either.

A copy is the directory `ntropy` with a `SKILL.md` in one of the skills
directories above: the project ones in the current directory and in each
directory above it up to the git root, the personal ones in the home
directory and below a moving variable that holds an absolute path. Without a
git root only the current directory is searched. A copy without
`ntropy-version` counts as older. A copy of the same or a newer version, one
that cannot be read as a regular file and one whose stamp is no
`MAJOR.MINOR.PATCH` version give no warning. Only the first 16 KiB of
`SKILL.md` are read. Copies are told apart by the real path of their
`SKILL.md`, and one warning names every older copy.

The warning is a `warning:` line on stderr, after the skill or the version.
Its fix names `ntropy skill` and the `ntropy skill --install` command for each
copy, and says to ask the user whether to update it. `--version` is a clap
error of its own kind, which the binary prints and then follows with the
warning.

## Consequences

Each `ntropy skill` without `--install` and each `ntropy --version` looks for
up to six skills directories per directory from the current directory to the
git root, six in the home directory and one for each moving variable that is
set.

A copy older than the ntropy that runs goes unnoticed until someone runs
`ntropy skill` or `ntropy --version`.
