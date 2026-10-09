---
title: "Filename parse panics on a multibyte character straddling byte 26"
kind: bug
component: note
impact: high
origin: review
tags: [security]
---
# Filename parse panics on a multibyte character straddling byte 26

Found in the 2026-07-02 security review of filename parsing. `filename::parse` splits the file stem at byte 26, the ULID width. When a multibyte UTF-8 character straddles that byte, the split panics. The panic does not abort cleanly. It leaves the scanner in a CPU-burning livelock, so one crafted file in `all-notes/` stops every scanning command until the process is killed. The fix is a one-line change, and the tests are ready below.

## Impact

The security review rated this medium (about CVSS 5.5, local, denial of service only). There is no memory unsafety and no data corruption. Three things make it worse than the rating suggests: a single crafted file bricks every scanning command, the failure is a hang rather than an abort, and it breaks ADR 0019's promise that one malformed note never breaks a query. Combined with walk-up vault discovery, which adopts a vault from any ancestor directory that has a `.ntropy/` directory, the trigger is "run any ntropy command beneath a tree that contains `.ntropy/` and a hostile `all-notes/*.md`". Fix it promptly. It is not an emergency.

## Problem

Read in the code during the 2026-07-02 review. `src/note/filename.rs`, `parse` (around lines 58-83), splits the stem at a fixed byte position:

```rust
let (id_part, rest) = stem.split_at(ULID_LEN);   // line 69
```

`str::split_at` panics when the index is not a UTF-8 character boundary. The guard before it (around line 65) checks only the byte length (`stem.len() < ULID_LEN + 2`). A stem whose byte 26 falls inside a multibyte character passes that guard and panics at the split.

## Reproduction

Reproduced on 2026-07-02 and again during the security triage. The input is 25 ASCII characters, then `é` (2 bytes, occupying bytes 25..27), then `-xx`:

```rust
// 25 ASCII chars + 'é' (2 bytes, occupying bytes 25..27) + "-xx"
ntropy::note::filename::parse("aaaaaaaaaaaaaaaaaaaaaaaaaé-xx.md");
```

It panics with:

```
byte index 26 is not a char boundary; it is inside 'é' (bytes 25..27) of `aaaaaaaaaaaaaaaaaaaaaaaaaé-xx`
```

Other straddling cases also panic: 24 `a` plus `😀` (4 bytes, bytes 24..28), and 25 `a` plus `€` (3 bytes, bytes 25..28). The shortest passing shape is 25 `a` plus `é` plus `-x.md`, a 29-byte stem. macOS NFD normalization does not help: `e` plus a combining U+0301 at position 25 still places bytes 26..28 inside the mark.

Other inputs in the same function are clean (verified). A stem of exactly 26 bytes with fewer characters (13 `é` plus `.md`) gives `TooShort`, because the 28-byte lower bound holds: the ULID and the `-` are 27 ASCII bytes, plus at least one slug byte. A boundary-aligned multibyte prefix (13 `é`, 26 bytes, plus `-x.md`) gives an `Id` error. A valid ULID with a multibyte slug (`…FAV-über.md`) parses. A multibyte character directly before `.md` parses too, since `strip_suffix` and `strip_prefix('-')` work on characters and are boundary-safe.

## Reachability

`Note::parse` (`src/note/mod.rs`, around line 77) feeds every top-level `all-notes/*.md` filename into `filename::parse`. The scanner calls `Note::parse` on every `.md` file it finds, so one file with such a name wedges every scanning command. No file ntropy creates triggers this. It takes a file created outside ntropy, which is exactly the case `FilenameError` exists to reject gracefully. Vault detection only checks for the `.ntropy/` marker (`src/vault/layout.rs`, around lines 16 and 95), so a hostile tree only has to look like a vault.

## End-to-end behaviour

Running a command such as `ntropy search` in a vault that holds the hostile file does not abort cleanly. Instead:

1. The `ignore` parallel walker's worker thread panics in `filename::parse`, reached through `Note::parse` and `load_note` (`src/scan.rs`, around lines 100-137).
2. `ignore`'s `WalkParallel::visit` runs the workers inside `std::thread::scope`. The main thread's `handle.join().unwrap()` panics again (`ignore` 0.4.26, `src/walk.rs`).
3. The main thread's unwind enters the scope's exit, which must join all remaining scoped threads. They spin forever, because the dead worker never signalled that it had finished. A sample of the wedged process showed the main thread parked in `scope` (`Thread::park`) under `scan_notes_dir`, and the workers busy-polling at about 11% CPU. One instance ran more than two minutes before it was killed.

The failure is therefore a livelock that consumes CPU until something external kills the process. It hits every scanning entry point: `search`, `edit` and select (`src/ops/select.rs`, around line 111), `new` (`src/ops/create.rs`, around line 79), `tags` (`src/ops/tags.rs`, around line 34), `info` (`src/ops/info.rs`, around line 50), view administration (`src/ops/view_admin.rs`, around line 59), `reconcile` (`src/reconcile.rs`, around lines 68 and 92), and the LSP cache (`src/bin/ntropy/run/lsp/cache.rs`, around line 77), which wedges the language server inside the editor. One integrity point: `reconcile` scans before it renames anything (`src/reconcile.rs`, around line 68), so the panic strikes before any change. There is no partial-write risk.

## Suggested fix

Use `str::split_at_checked`. It has been stable since Rust 1.80, and the project uses edition 2024 with no MSRV pin. It is better than a separate `is_char_boundary` check, because the boundary test and the split are one operation that cannot drift apart during refactoring. It is the standard library API built for exactly this case.

Do not fix this with a character-count guard. `split_at` is byte-indexed regardless, so a 36-character stem with `é` at character 25 still panics. A character-count guard is not a fix.

Classification: a non-boundary at byte 26 means a non-ASCII byte lies within the first 26 bytes. A ULID is 26 ASCII Crockford characters, so that prefix can never be a ULID. Map the case to the existing `FilenameError::Id`, "does not start with a valid ULID", which is the message the scan warning shows (`e.to_string()`, `src/scan.rs`, around line 137). `thiserror` does not chain sources into `Display`. No new variant is needed. `IdError`'s field is private to its module, so the source comes from parsing the stem, which is guaranteed to fail:

```rust
// src/note/filename.rs, replacing the split at line 69
    // The identity is the leading 26 bytes. `split_at_checked` refuses a split
    // that lands inside a multibyte character; such a stem cannot start with a
    // ULID (26 ASCII characters), so report it as a bad id rather than panic.
    let Some((id_part, rest)) = stem.split_at_checked(ULID_LEN) else {
        return Err(FilenameError::Id {
            name: name.to_string(),
            source: stem
                .parse::<Id>()
                .expect_err("the stem exceeds a ULID's 26 bytes, so it cannot parse as one"),
        });
    };
```

The `expect_err` cannot fail: the `TooShort` guard ensures `stem.len() >= 28`, and `Ulid::from_string` requires exactly 26 bytes. If the team dislikes building a source by parsing, the fallback is a dedicated variant, `#[error("`{0}` does not start with a valid ULID")] BadUlidPrefix(String)`, at the cost of a near-duplicate message. Keep the `TooShort` byte guard as it is. It is correct, and it gives the friendlier message for names that are genuinely short.

## Sibling-instance sweep

`filename.rs` is the only unguarded site. The triage swept all of `src/` for `split_at`, str range slicing, `.get(range)`, byte arithmetic and truncation. Every other fixed or computed `&str` cut is safe:

| Site | Verdict |
|---|---|
| `src/link/mod.rs`, `parse_target` (around lines 171-175) | **Guarded**: `is_char_boundary(ULID_LEN)` before `split_at`, which also rejects short targets that are out of range. |
| `src/bin/ntropy/run/lsp/offset.rs` | Safe: clamps and rounds down to a boundary (around lines 68-71), `clamp_utf8` (around lines 89-98), and UTF-16 walks `char_indices` (around lines 102-114). A round-trip property test covers it (around lines 209-224). |
| `src/id.rs`, `Id::tail` (around lines 63-67) | Safe: slices the canonical 26-ASCII ULID render. |
| `src/text/slug.rs`, `truncate_at_boundary` (around lines 136-146) | Safe: the input is already filtered to ASCII `[a-z0-9-]`. |
| `src/text/tag.rs` (around lines 80-85) | Safe: slices elements of a `Vec<String>`, guarded by a count. |
| `src/note/frontmatter.rs`, `split` (around lines 61-97) | Safe: offsets accumulate whole-line lengths, so every cut is a line boundary. Hence `note/mod.rs` (around lines 89-90), `content[..body_start]`, is a boundary too. |
| `src/template.rs` (around lines 119-141) | Safe: indices come from `find("{{")` and `find("}}")` plus ASCII widths. |
| `src/bin/ntropy/run/lsp/completion/{tag,link}.rs` | Safe: `rfind` and `find` on ASCII delimiters, with guarded byte peeks. |
| `src/bin/ntropy/run/lsp/uri.rs` (around lines 35-45) | Safe: the index comes from `find('/')`, and percent-decoding is byte-based. |
| `src/link/code.rs` | Safe: works on `as_bytes()`. The one str slice cuts at `\n`. |
| `src/bin/ntropy/run/picker/state.rs`, `delete_word` (around lines 190-207) | Safe: uses `rfind` and `len_utf8`, and is documented as multibyte-correct. |
| `src/bin/ntropy/run/picker/layout.rs` (from around line 108) | Safe: display-width truncation over `chars()` and `char_indices`. |
| `src/query/token.rs` | Safe: tokenizes over `Vec<char>`, with no byte slicing. |
| `src/ops/select.rs`, `as_ulid` (around lines 84-91) | Safe: a length check, then a full `parse::<Id>()`. |
| `src/view/leaf.rs` (around lines 70-78) | Safe: uses `Id::tail`. |

## Centralization note

A local guard is enough. Only two sites split at `ULID_LEN` (`filename::parse` and `link::parse_target`), and the second is already guarded. A shared `crate::id` helper would centralize two call sites, which is not worth the API surface. An optional symmetry cleanup is to switch `parse_target` to the same `split_at_checked` pattern.

## Tests to add

Write these first and watch them panic before the fix. In `src/note/filename.rs` tests:

```rust
#[test]
fn parse_rejects_a_multibyte_char_straddling_the_ulid_width() {
    let name = format!("{}é-xx.md", "a".repeat(25));   // byte 26 inside 'é'
    assert!(matches!(parse(&name), Err(FilenameError::Id { .. })));
}
#[test]
fn parse_rejects_a_four_byte_char_straddling_the_ulid_width() {
    let name = format!("{}😀-xx.md", "a".repeat(24));   // bytes 24..28
    assert!(matches!(parse(&name), Err(FilenameError::Id { .. })));
}
#[test]
fn parse_rejects_a_boundary_aligned_multibyte_prefix() {
    let name = format!("{}-x.md", "é".repeat(13));      // 26 bytes, not a ULID
    assert!(matches!(parse(&name), Err(FilenameError::Id { .. })));
}
#[test]
fn parse_accepts_a_multibyte_slug() {
    let parsed = parse(&format!("{ULID}-über.md")).expect("parse");
    assert_eq!(parsed.slug, "über");
}
```

Plus the ADR 0019 contract test in the `src/scan.rs` tests. It pins the real promise, and it must not hang:

```rust
#[test]
fn multibyte_mangled_filename_is_a_warning_not_a_crash() {
    let (_guard, notes) = temp_notes_dir();
    write(&notes, &format!("{}é-xx.md", "a".repeat(25)), "---\ntitle: X\n---\n");
    let scan = scan_notes_dir(&notes).expect("scan");
    assert!(scan.notes.is_empty());
    assert_eq!(scan.warnings.len(), 1);
}
```

## Open questions

- Should `load_note` also be wrapped in `std::panic::catch_unwind`, turning a panic into a `ScanWarning`? The security triage proposed this on 2026-07-02 as an optional follow-up. The panic-to-livelock path lives in `src/scan.rs` and in the `ignore` walker, not in `filename.rs`. Any future panic in the walker closure (`src/scan.rs`, around lines 100-106, for example one raised by frontmatter parsing) would wedge the process the same way. Catching panics there would enforce ADR 0019's "one bad file never breaks a query" against the whole class of panics, not only this instance. The question was raised with the user on 2026-07-02 and has not been filed as its own todo. The fix above stands without it. Decide during the fix whether to include it.

## Done when

- A filename with a multibyte character across byte 26 yields `Err(FilenameError::Id)` instead of a panic. A test written to fail first verifies this.
- The scanner turns such a file into a `ScanWarning` and completes without a hang, which the `scan.rs` contract test verifies.
- Legitimate names (a valid ULID with a multibyte slug) still parse, and the existing `filename.rs` tests stay green.
