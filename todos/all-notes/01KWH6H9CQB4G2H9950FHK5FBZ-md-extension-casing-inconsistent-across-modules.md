# `.md` extension casing is treated differently by scan, info, and template loading

Found during the 2026-07-02 codebase review (units 08/09, ops + templates).

## Problem

Three modules answer "is this a Markdown file?" differently:

- `scan::is_markdown` (`src/scan.rs:129-131`) compares the extension
  case-sensitively (`== Some("md")`, documented "case-sensitive, as on
  disk"). A note file named `<ULID>-x.MD` is silently ignored as a resource
  — no warning, since only `.md` files get warnings.
- `ops::info::template_names` (`src/ops/info.rs:94-109`) matches the
  extension case-insensitively (`eq_ignore_ascii_case("md")`), so `info`
  lists a template file `Meeting.MD` under the name `Meeting`.
- `template::load_named` (`src/template.rs:93-108`) appends a literal
  `.md` to the requested stem. On a case-sensitive filesystem,
  `ntropy new --template Meeting` then fails with `NotFound` for the
  `Meeting.MD` file that `info` just listed. On macOS's default
  case-insensitive APFS it happens to load.

Consequences: `info` can advertise template names that `new --template`
cannot load; behavior differs between Linux and macOS; an uppercase-extension
note vanishes from every command without any warning.

## Suggested resolution

Pick one convention and apply it everywhere. The simplest coherent choice is
strict lowercase `.md` in all three places (matching scan's documented
behavior), which shrinks `template_names` to the same check as
`is_markdown`. If case-insensitive is preferred instead, scan and
`load_named` must both learn it, and the ADR 0019 warning set should cover
near-miss extensions.

## Acceptance

- One shared definition (or at least one documented convention) of the
  Markdown extension check used by scan, info, and template loading.
- `info`'s template list and `new --template` agree on any filesystem.
