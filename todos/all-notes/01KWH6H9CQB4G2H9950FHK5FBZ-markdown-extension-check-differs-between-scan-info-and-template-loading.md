---
title: "Markdown extension check differs between scan, info and template loading"
kind: bug
component: vault
origin: review
---
# Markdown extension check differs between scan, info and template loading

Found in the 2026-07-02 codebase review of operations and templates. Three modules decide whether a file is Markdown, and they disagree. A note named `<ULID>-x.MD` is silently ignored, `info` can list template names that `new --template` cannot load, and results depend on the filesystem.

## Problem

Read in the code during the review:

- `scan::is_markdown` (`src/scan.rs`, around lines 129 to 131) compares the extension case-sensitively (`== Some("md")`, documented as "case-sensitive, as on disk"). A note file named `<ULID>-x.MD` is silently ignored as a resource. No warning appears, since only `.md` files get warnings.
- `ops::info::template_names` (`src/ops/info.rs`, around lines 94 to 109) matches the extension case-insensitively, so `info` lists a template file `Meeting.MD` under the name `Meeting`.
- `template::load_named` (`src/template.rs`, around lines 93 to 108) appends a literal `.md` to the requested stem. On a case-sensitive filesystem, `ntropy new --template Meeting` then fails with `NotFound` for the `Meeting.MD` file that `info` just listed. On the default case-insensitive APFS on macOS, it happens to load.

## Impact

`info` advertises template names that `new --template` cannot load. Behaviour differs between Linux and macOS, and an uppercase-extension note vanishes from every command without a warning.

## Suggested fix

Pick one convention and apply it everywhere. The simplest coherent choice is strict lowercase `.md`, matching the documented behaviour of scan. That makes `template_names` the same check as `is_markdown`. If case-insensitive matching is preferred instead, scan and `load_named` must both learn it, and the ADR 0019 warning set should cover near-miss extensions.

## Done when

- One shared definition, or at least one documented convention, of the Markdown extension check serves scan, info and template loading.
- `info`'s template list and `new --template` agree on any filesystem.
