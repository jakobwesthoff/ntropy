# `filename::parse` panics (→ livelock) on multibyte characters straddling byte 26

> **STATUS: TRIAGED (code review + Fable security triage, 2026-07-02).**
> Queue item SEC-2. Originally filed as a verified panic during review unit 01;
> Fable's security triage confirmed the panic, discovered that its **end-to-end
> behavior is a CPU-burning livelock, not a clean crash**, and swept the codebase
> confirming this is the **only** unguarded fixed-offset `&str` split on hostile
> input. Fix and tests below are ready to implement.

## Severity

**Medium** (≈ CVSS 5.5, `AV:L/AC:L/PR:N/UI:R/S:U/C:N/I:N/A:H`). Local, DoS-only,
no memory unsafety or data corruption. Aggravating: a single crafted file bricks
*every* scanning command; the failure is a CPU-burning hang, not a clean abort;
and it violates ADR 0019's explicit "one malformed note never breaks a query"
contract. Combined with SEC-4 (walk-up adoption of a discovered vault), the
trigger is "run any ntropy command anywhere beneath a tree containing `.ntropy/`
+ `all-notes/<hostile>.md`". Fix promptly; not emergency-grade.

## Problem

`src/note/filename.rs:parse` (lines 58-83) splits the filename stem at a fixed
byte position:

```rust
let (id_part, rest) = stem.split_at(ULID_LEN);   // line 69
```

`str::split_at` panics when the index is not a UTF-8 char boundary. The preceding
guard (line 65) only checks the *byte* length (`stem.len() < ULID_LEN + 2`), so a
stem whose byte 26 falls inside a multibyte character passes the guard and then
panics at line 69.

## Reproduction (verified 2026-07-02, re-confirmed in triage)

```rust
// 25 ASCII chars + 'é' (2 bytes, occupying bytes 25..27) + "-xx"
ntropy::note::filename::parse("aaaaaaaaaaaaaaaaaaaaaaaaaé-xx.md");
```

Panics with:

```
byte index 26 is not a char boundary; it is inside 'é' (bytes 25..27) of `aaaaaaaaaaaaaaaaaaaaaaaaaé-xx`
```

Additional straddle cases confirmed to panic: 24×`a` + `😀` (4-byte, bytes
24..28); 25×`a` + `€` (3-byte, bytes 25..28); shortest passing shape 25×`a` +
`é` + `-x.md` (29-byte stem). macOS NFD normalization does **not** save you:
`e` + combining U+0301 at position 25 still lands bytes 26..28 inside the mark.

**Other inputs in the same function are clean** (verified): a stem exactly 26
bytes but fewer chars (13×`é` + `.md`) → `TooShort` (28-byte lower bound holds
because ULID + `-` are 27 ASCII bytes plus ≥1 slug byte); a boundary-aligned
multibyte prefix (13×`é` = 26 bytes + `-x.md`) → `Id` error; a valid ULID +
multibyte slug (`…FAV-über.md`) → parses; a multibyte char immediately before
`.md` → parses (`strip_suffix`/`strip_prefix('-')` are char-based, boundary-safe).

## End-to-end behavior is worse than a crash: a permanent livelock

Running e.g. `ntropy search` in a vault containing the hostile file does **not**
cleanly abort:

1. The `ignore` parallel-walker worker thread panics in `filename::parse` (via
   `Note::parse` ← `load_note`, `src/scan.rs:100-137`).
2. `ignore`'s `WalkParallel::visit` runs workers inside `std::thread::scope`; the
   main thread's `handle.join().unwrap()` re-panics (`ignore-0.4.26/src/walk.rs`).
3. Main's unwind enters the scope's exit, which must join all remaining scoped
   threads, but they spin forever (the dead worker never signaled quiescence).
   The wedged process was sampled: main parked in `scope` → `Thread::park` under
   `scan_notes_dir`; workers busy-polling at ~11% CPU indefinitely (one instance
   ran 2+ minutes until killed).

So the failure mode is a **livelock consuming CPU until externally killed**, in
every scanning entry point: `search`/`edit`/select (`src/ops/select.rs:111`),
`new` (`src/ops/create.rs:79`), `tags` (`src/ops/tags.rs:34`), `info`
(`src/ops/info.rs:50`), view admin (`src/ops/view_admin.rs:59`), `reconcile`
(`src/reconcile.rs:68,92`), and the LSP cache (`src/bin/ntropy/run/lsp/cache.rs:77`,
which wedges the language server inside the editor). One integrity mitigation:
`reconcile` scans before any rename (`src/reconcile.rs:68`), so the panic strikes
before mutation — no partial-write risk.

## Reachability

`Note::parse` (`src/note/mod.rs:77`) feeds every top-level `all-notes/*.md`
filename into `filename::parse`. The scanner calls `Note::parse` on every `.md`
file it finds, so one file with such a name wedges every scanning command. No
canonical ntropy-created file triggers this; it requires an out-of-band created
file — exactly the case `FilenameError` exists to reject gracefully. Vault
detection is just the `.ntropy/` marker (`src/vault/layout.rs:16,95`), so the
hostile tree need only look like a vault.

## Recommended fix: `str::split_at_checked`

Use `str::split_at_checked` (stable since Rust 1.80; the project is edition 2024,
no MSRV pin). It is strictly better than a separate `is_char_boundary` check —
the boundary test and the split are one atomic operation that cannot drift apart
under refactoring, and it is the stdlib API built for exactly this.

**Do NOT** "fix" via a character-count guard (candidate option c): `split_at` is
byte-indexed regardless, so a 36-char stem with `é` at char 25 still panics. A
char-count guard is not a fix.

**Classification:** a non-boundary at byte 26 implies a non-ASCII byte within the
first 26 bytes, and a ULID is 26 ASCII Crockford chars, so the prefix can never
be a ULID → map to the existing `FilenameError::Id` ("does not start with a valid
ULID"), which is the message the scan warning renders (`e.to_string()`,
`src/scan.rs:137`; thiserror does not chain sources into `Display`). No new
variant needed. `IdError`'s field is module-private, so obtain the source by
parsing the stem (guaranteed to fail):

```rust
// Replace src/note/filename.rs:69:
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

The `expect_err` is airtight: the `TooShort` guard ensures `stem.len() >= 28`,
and `Ulid::from_string` requires exactly 26 bytes. Fallback if the team dislikes
the parse-to-build-a-source construction: a dedicated variant
`#[error("`{0}` does not start with a valid ULID")] BadUlidPrefix(String)` (at
the cost of a near-duplicate message). Keep the `TooShort` byte guard as is — it
is correct and gives the friendlier message for genuinely short names.

## Sibling-instance sweep — `filename.rs:69` is the ONLY unguarded site

Fable swept all of `src/` for `split_at`, str range slicing, `.get(range)`, byte
arithmetic, and truncation. Every other fixed/computed `&str` cut is safe:

| Site | Verdict |
|---|---|
| `src/link/mod.rs:171-175` `parse_target` | **Guarded**: `is_char_boundary(ULID_LEN)` before `split_at` (also rejects out-of-range short targets). |
| `src/bin/ntropy/run/lsp/offset.rs` | Safe: clamps + rounds down to boundary (68-71), `clamp_utf8` (89-98), UTF-16 walks `char_indices` (102-114); round-trip property test (209-224). |
| `src/id.rs:63-67` `Id::tail` | Safe: slices the canonical 26-ASCII ULID render. |
| `src/text/slug.rs:136-146` `truncate_at_boundary` | Safe: input is post-filter `[a-z0-9-]` ASCII. |
| `src/text/tag.rs:80-85` | Safe: `Vec<String>` element slices, count-guarded. |
| `src/note/frontmatter.rs:61-97` `split` | Safe: offsets accumulate whole-line lengths; every cut is a line boundary (so `note/mod.rs:89-90` `content[..body_start]` is a boundary too). |
| `src/template.rs:119-141` | Safe: indices from `find("{{")`/`find("}}")` + ASCII widths. |
| `src/bin/ntropy/run/lsp/completion/{tag,link}.rs` | Safe: `rfind`/`find` of ASCII delimiters, guarded byte peeks. |
| `src/bin/ntropy/run/lsp/uri.rs:35-45` | Safe: index from `find('/')`; percent-decode is byte-based. |
| `src/link/code.rs` | Safe: operates on `as_bytes()`; the one str slice cuts at `\n`. |
| `src/bin/ntropy/run/picker/state.rs:190-207` `delete_word` | Safe: `rfind` + `len_utf8`, documented multibyte-correct. |
| `src/bin/ntropy/run/picker/layout.rs:108+` | Safe: display-width truncation over `chars()`/`char_indices`. |
| `src/query/token.rs` | Safe: tokenizes over `Vec<char>`; no byte slicing. |
| `src/ops/select.rs:84-91` `as_ulid` | Safe: length check then full `parse::<Id>()`. |
| `src/view/leaf.rs:70-78` | Safe: uses `Id::tail`. |

## Tests to add (bug-fix-first — write the first before the fix and watch it panic)

In `src/note/filename.rs` tests:

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

Plus the ADR 0019 contract test in `src/scan.rs` tests (pins the real promise —
this must NOT hang):

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

## Related hardening (optional, SEPARATE line item — inform user before filing)

The panic→livelock is a property of `scan.rs` + `ignore`'s scoped-thread walker,
**not** of `filename.rs`. Any *future* panic inside the walker closure
(`src/scan.rs:100-106`, e.g. from frontmatter parsing) would wedge the process
the same way. Wrapping `load_note` in `std::panic::catch_unwind` and converting a
panic into a `ScanWarning` would enforce ADR 0019's "one bad file never breaks a
query" against the entire class, not just this instance. Advisory; the primary
fix above stands on its own. This is a candidate for its own todo — raise with
the user before filing.

## Centralization note

Local guard is sufficient: only two sites split at `ULID_LEN` (`filename::parse`,
`link::parse_target`), and the second is already guarded. A shared `crate::id`
helper would centralize two call sites — not worth the API surface. Optional
symmetry cleanup: switch `parse_target` (`src/link/mod.rs:171-175`) to the same
`split_at_checked` pattern.

## Acceptance

- A filename with a multibyte char across byte 26 yields `Err(FilenameError::Id)`,
  not a panic; verified by test (written failing first).
- The scanner turns such a file into a `ScanWarning` and completes (no hang);
  verified by the `scan.rs` contract test.
- Legitimate names (valid ULID + multibyte slug) still parse; existing
  `filename.rs` tests stay green.
