# 85. Page templates render with minijinja 3

Date: 2026-11-01

## Status

Accepted

Amends [ADR 0050](0050-page-templates-with-minijinja.md), which names
minijinja 2.24.0, its dependencies `serde` and `memo-map`, and a
serde-serializable context.

## Context

minijinja 3 makes the `serde` feature optional and drops `memo-map`.
`context!` and `Template::render` convert values through `Into<Value>`;
a struct converts through the `minijinja::value::Serde` wrapper. The
formatter callback receives `&mut State`.

minijinja 3 also follows Jinja2 more closely in arithmetic, filters, tests
and the text form of lists and maps. A theme's templates
([ADR 0058](0058-theme-templates-overriding-the-built-in-ones-by-name.md))
run on that behaviour. Neither the built-in templates nor the
documentation site's theme use a construct that renders differently.

## Decision

ntropy uses minijinja 3 with its default features and `serde`. The
default features stay because `multi_template` provides the `extends`,
`include` and `import` of ADR 0058 and `builtins` provides the filters
theme templates call.

The page context passes strings by reference and every `Vec` and struct
through `Serde`, so a template sees the same values as from the
serde-serializable context of ADR 0050. The HTML-escaping formatter keeps
its behaviour and takes `&mut State`.

A theme's templates follow minijinja 3's semantics. The changelog lists
the differences from minijinja 2 that can change a custom theme's output.
