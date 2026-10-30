---
title: "View grouping looks up its field case-sensitively"
kind: bug
component: view
impact: low
origin: review
tags: [ux]
---
# View grouping looks up its field case-sensitively

Query field predicates ignore the letter case of the field name (ADR 0078), but a view grouping by a frontmatter field still looks the field up exactly. A note with `Status: open` is found by `ntropy search status:open`, yet it is missing from a view defined over `status`. Found by the plan review of the case-insensitive field lookup on 2026-10-30; confirmed by reading the code, not run.

## Problem

`group_values` in `src/view/materialize.rs` reads `note.frontmatter.get(Value::from(field))`, an exact key lookup. The query evaluator folds case and matches any key equal to the predicate's name under that fold.

## Suggested fix

Collect the values of every key that equals the view's field under the same case fold the query evaluator uses (one shared helper), so a view and the query `<field>:<value>` select the same notes. Twin keys (`Status` and `status`) contribute all their values. Read the values with `scalar_text` (`src/note/frontmatter.rs`) instead of the local `scalar_to_string` copy in `src/view/materialize.rs`. That copy returns nothing for a YAML-tagged value (`status: !x open`), so such a note is left out of the view while the shared helper reads it as `open`. Pin it with a materialize test and a CLI test of a view over `status` holding a note keyed `Status`, and say in the view docs that the field name ignores case.
