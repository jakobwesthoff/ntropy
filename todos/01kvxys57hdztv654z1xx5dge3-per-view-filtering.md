# Per-view filtering (post-v1)

Deferred from the v1 view model (ADR 0009). v1 views project the whole note set
grouped by a field; they cannot be restricted to a subset.

## To decide later

- Let a view definition carry a query (the DSL, ADR 0012) that limits which
  notes the view includes (e.g. a `by-status` view over only `tag:work` notes).
- Config shape for the per-view filter in `.ntropy/config.toml`.
- Interaction with incremental refresh and `reconcile`.
