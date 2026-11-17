# 94. Cut the agent skill into a core of rules and eight parts

Date: 2026-11-17

## Status

Accepted

Amends [ADR 0092](0092-ship-the-agent-skill-in-the-binary-and-print-it-with-ntropy-skill.md),
whose skill had seven parts.

## Context

`render` and `show` had no part of their own: their rules sat in the core
beside rules for every other task.

## Decision

The core, `SKILL.md`, holds the rules that every task needs, each stated
once. A part holds the details of one kind of task, and the core names it by
file and by command in a list at the top and at the step that needs it.

The parts are `writing-notes`, `querying`, `vaults`, `views`, `rendering`,
`site`, `site-themes` and `document-themes`, in this order for `ntropy skill
<part>`, the part list of `--install` and every list in the docs. `rendering`
holds `render` and `show` for every format: where the artifact goes, the
theme and paper that apply, linked sets, the standalone page.

The core is capped at 13,000 bytes, a budget chosen as the size of the
finished core with a little slack, and a part at 30,000 bytes. The core puts
its rules before its command reference.

Tests pin the caps, one phrase for each key rule of the core, and the
command reference of the core against `ntropy --help`.

## Consequences

What a part holds reaches an agent only when it opens the part.
