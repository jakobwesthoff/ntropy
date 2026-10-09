---
title: "Filter the notes a view includes with a query"
kind: feature
component: view
origin: discussion
---
# Filter the notes a view includes with a query

Deferred during the v1 view design (ADR 0009). v1 views project the whole
note set, grouped by a field. They cannot be restricted to a subset.

## Proposal

Let a view definition carry a query (the query language, ADR 0012) that
limits which notes the view includes. For example, a `by-status` view could
cover only the notes matching `tag:work`.

## Open questions

- What is the config shape for the per-view filter in `.ntropy/config.toml`?
- How does the filter interact with incremental refresh and `reconcile`?
