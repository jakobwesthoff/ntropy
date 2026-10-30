---
title: "Float field values compare differently in the CLI and the site search"
kind: bug
component: query
impact: low
origin: review
---
# Float field values compare differently in the CLI and the site search

A frontmatter value `x: 2.0` matches the query `x:2.0` in the CLI but not in the site's search, which reads the value as `2`. The site's evaluator is meant to follow the CLI's semantics (ADR 0052), so the two disagree on every float with a zero fraction. Found by the plan review of the tags-keep-punctuation work on 2026-10-30; confirmed by reading both sides, not run.

## Problem

- CLI: `scalar_to_string` in `src/query/eval.rs` renders `Value::Number` with `n.to_string()`. serde_yaml_ng formats a float through `ryu`, which prints `2.0`.
- Site: `src/site/search.rs` embeds the frontmatter as JSON, so `2.0` arrives as the number 2. `scalarText` in `site/src/search/eval.ts` uses `String(value)`, which gives `2`.

Values like `1.5` agree. Very large or very small floats may differ too (`String(1e21)` is `1e+21`).

## Suggested fix

Use one spelling on both sides. The Rust side spells a YAML number with its `Display` (`2.0`, `1.5`), both in `scalar_to_string` and in the coercion of scalar titles and tags, so the site has to follow: embed each scalar's Rust spelling in the search data (or the frontmatter as strings) instead of JSON numbers, so `scalarText` never formats a number itself. Add the cases to `tests/fixtures/query-corpus.json` so both evaluators are pinned. Changing the site side changes which notes `x:2.0` and `x:2` select in the site search, so name it in the changelog.
