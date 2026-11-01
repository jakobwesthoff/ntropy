---
title: "A TOML datetime in site vars reaches templates as an internal map"
kind: bug
component: site
origin: implementation
---
# A TOML datetime in site vars reaches templates as an internal map

A date or datetime value in `[site.vars]` of the vault config reaches the
site's page templates as a map with the single key
`$__toml_private_datetime`, so `{{ vars.when }}` prints that map instead of
the date. Found on 2026-11-01 during the move to minijinja 3; it is the same
under minijinja 2.24 and 3, as it comes from how `toml::Value::Datetime`
serializes through serde.

## Problem

`src/site/page.rs` passes `vars` (a `toml::Table`) to the templates through
serde. The `toml` crate serializes a `Datetime` as a struct with the private
field name `$__toml_private_datetime`, which only its own deserializer
understands. The characterization test
`a_template_reads_every_kind_of_value_the_context_carries` in
`src/site/page.rs` pins today's behaviour (`{{ vars.when is mapping }}` with
`when = 2026-11-01`).

## Decision

Lead, 2026-11-01: a TOML date, time or datetime reaches the templates as its
text as written in the config (`2026-11-01`, `2026-11-01T12:00:00Z`), the
form a theme author can print or compare. The same applies wherever else a
`toml::Value` reaches a template (for example TOML frontmatter values), if
it goes the same way. The characterization test changes with the fix.
