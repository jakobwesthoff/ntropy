---
title: "A wide table in a PDF runs its cells into each other"
kind: bug
component: render
impact: high
origin: review
---
# A wide table in a PDF runs its cells into each other

A table wider than the text column shrinks its columns in a PDF until
values that cannot break overlap the cell next to them, and the text
becomes unreadable. Found on 2026-11-01 in the review of the theme test
material, reproduced with ntropy 2.9.0.

## Problem

A ten-column table such as

```markdown
| ID | Pipeline | Region | Start | End | Owner | State | Runs | Failures | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1042 | Release pipeline | eu-central | 2026-10-01 | 2026-11-01 | platform-team | succeeded | 1234 | 12 | nightly |
```

renders as `1042Release pipelineeu-cen-tral`: the ID runs into the
pipeline name, which runs into the region, and the two dates touch. Words
are hyphenated into narrow columns (`Fail-ures`, `plat-form-team`). There
is no warning, under `--strict` either. The prelude sets only stroke and
inset for tables (`src/render/typst/prelude.typ:142-151`).

## Suggested fix

No cell's content may overlap another cell. Possible ways, not decided: a
smaller text size for tables with many columns, a landscape page or a
narrower inset for them, or letting long tokens break. If a table still
cannot fit, a warning names it.
