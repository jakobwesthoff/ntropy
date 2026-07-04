# SEC-3: View name is an unvalidated path component → destructive out-of-vault file deletion

> **STATUS: TRIAGED (code review + Fable security triage, 2026-07-02).**
> Confirmed by code reading (review units 07 & 15) and independently verified by
> a Fable security-engineer pass (path mechanics reproduced via `rustc`; delete
> loop traced end-to-end). Empirical end-to-end PoC (crafted config → observed
> deletion) is specified below and should be written as the regression test as
> part of the fix. **This is the highest-severity item in the review queue: a
> destructive, data-losing primitive, not a DoS.**

## Severity

**High** (≈ CVSS 7.0–7.8, `AV:L/AC:L/PR:N/UI:R/S:U-or-C/C:N/I:H/A:H`). A hostile
or hand-edited per-vault config turns the next view sync into a recursive delete
of an attacker-chosen directory tree, with impact bounded only by which files
the invoking user can write. It combines with SEC-4 (walk-up auto-adopts a
discovered `.ntropy/config.toml`) into a drive-by: clone/sync/extract a tree,
`cd` in, run `ntropy today`, and the attacker-named path is enumerated and
deleted with no confirmation and no rollback. Do **not** rate below High. Do
**not** defer pending the SEC-4 decision: fixing this removes the destructive
primitive regardless of whether walk-up adoption is changed, and it is the
higher-leverage of the two. Even ignoring SEC-4, the `.git` / `all-notes/x`
self-clobber cases are reachable through a user's own config typo.

## Root cause

`Layout::view_dir` (`src/vault/layout.rs:85-87`) computes a view's output
directory as:

```rust
pub fn view_dir(&self, name: &str) -> PathBuf {
    self.root.join(name)
}
```

`root` is canonical/absolute (set in `require_vault`/`walk_up`,
`src/vault/resolve.rs:99,126`). `Path::join` then has two dangerous behaviors
with an attacker-controlled `name`, both reproduced with `rustc` during triage:

- an **absolute** `name` (`/etc`, `/home/user/Documents`) *replaces* the base
  entirely → result is not under the vault at all;
- a **relative** `name` with `..` (`../../Documents`) is kept verbatim (no
  lexical normalization) → resolves outside the vault at syscall time.

And crucially, a name need not escape the vault at all to be catastrophic (see
`.git` below). `view_dir` is infallible and does zero validation.

## The destructive primitive (`sync_view`)

`sync_view` (`src/view/materialize.rs:45-86`) is **not** "create/delete a few
symlinks". Traced end-to-end:

1. Resolves `view_dir = vault.layout().view_dir(&view.name)` — the escaped or
   dotfile path (`materialize.rs:46`).
2. `actual_state` → `collect_state` (`materialize.rs:139-158`) **recursively**
   enumerates that directory (`read_dir_entries`, descending into every real
   subdirectory), building the **entire** in-memory map before any deletion
   (`materialize.rs:142`). `read_dir_entries` (`src/fsutil.rs:138-159`) does
   **not** follow symlinks, so recursion descends only real dirs.
3. For every entry that is not one of the view's *desired* leaves, calls
   `fsutil::remove_file` (`materialize.rs:58-64`). For an arbitrary external
   tree the desired set is empty/mismatched, so this removes **every regular
   file and every symlink** in the tree. The test
   `a_stray_non_leaf_file_inside_a_group_is_removed` (`materialize.rs:524-545`)
   pins exactly this "delete anything that isn't my leaf" behavior.
4. Prunes emptied directories deepest-first via `remove_dir_if_empty`
   (`materialize.rs:81-83`, `fsutil.rs:118-132`). The view's own root is kept
   (`materialize.rs:80`).

Net effect for `name` pointing at `~/Documents`: the next sync deletes every
regular file and symlink under `~/Documents`, recursively, and removes every
directory that this empties.

### Deletion is non-atomic, ordered, and has no rollback

`remove_file` propagates its error with `?` (`fsutil.rs:99-101`,
`materialize.rs:62`), and `actual` is a `BTreeMap`, so entries are deleted in
sorted-path order until the first `EACCES`/`EPERM`, then the whole command
aborts. Against a **user-owned** tree (their own `~/Documents`, their own repo
`.git`) every file is removable, so it completes fully. Against a system path it
deletes the lexically-earlier prefix and then errors — still destroying data,
just partially. There is no rollback for the files already gone. "Guts `.git` /
wipes `~/Documents`" is realistic *precisely because* those are user-writable.

## The names come from two paths, neither sanitizes

### 1. `view add` CLI — validation is only exact reserved-name equality

`add_view` (`src/ops/view_admin.rs:41-44`) checks only
`layout::is_reserved_name(name)` (`src/vault/layout.rs:101-103`), which is exact
string equality against `RESERVED_NAMES = ["all-notes", ".ntropy", ".gitignore"]`
(`src/vault/layout.rs:34`). It does **not** reject `/`, `..`, absolute paths,
leading-dot names, or control chars. `add_view` then calls `sync_view` directly
(`view_admin.rs:60`). So `ntropy view add "../escape" tags`,
`ntropy view add ".git" tags`, `ntropy view add "/etc" tags`, and
`ntropy view add "all-notes/x" tags` all pass today. (CLI dispatch:
`run/mod.rs:321,329`.)

### 2. `config.toml` load — no validation at all

`PerVaultConfig::load` (`src/config/per_vault.rs:38-50`) is plain serde
(`toml::from_str`). `ViewConfig`/`ViewDef` construction (`per_vault.rs:27-31`,
`src/view/mod.rs:28-35`) is a plain `Into<String>`. Names loaded from an
existing config are never validated. `reconcile`/`refresh_views` map them
straight through: `load_views` (`src/reconcile.rs:190-197`) →
`sync_views_and_gitignore` (`src/reconcile.rs:80-88`) → `view::sync_all`
(`src/view/mod.rs:42-47`) → `sync_view`. TOML strings can carry `..`, absolute
paths, leading dots, control chars, and newlines.

## Three distinct vector classes (the fix must close all three)

The most important triage correction: **this is not only a traversal /
confinement bug.** Three independent classes:

1. **Traversal / absolute** — `../../Documents`, `/etc`. Escapes the vault.
2. **In-vault dotfile** — `.git` is a single-segment, no-`..`, fully "relative"
   name that is **not** in `RESERVED_NAMES`. `view_dir(".git")` = `<root>/.git`,
   and a sync against a note-less/mismatched set deletes every file in the
   repo's git directory. **No traversal required.** A framing of "confine to
   root" would close class 1 and leave this wide open. Same class: any
   leading-dot dir (`.ssh` if it happened to be in-tree, `.ntropy`-adjacent
   dotfiles).
3. **Reserved-prefix bypass** — the whole-string `is_reserved_name` is defeated
   by prefixing: `all-notes/x` and `.ntropy/x` pass it, yet `view_dir` writes
   *inside* the canonical notes/config dirs and the sync deletes real notes /
   templates there. The reserved check must apply to the **first path segment**,
   not the whole string.

## What is safe (checked — narrows the fix surface to one string)

- The grouping **values** (the subdirectories created *under* a view) are safe:
  they flow through `tag::normalize` / `slug::normalize_segment`
  (`materialize.rs:180`, `src/text/tag.rs:32`, `src/text/slug.rs:38-75`), which
  strip everything except `[a-z0-9-]` (and `/` between segments). Only the view
  **name** is unvalidated. **The entire fix surface is this one string.**
- `read_dir_entries` not following symlinks (`fsutil.rs:144-159`) bounds the
  blast radius to the named tree: the deletion will not chase a symlink *inside*
  the victim tree to a third location; it removes the symlink entry itself. A
  real limit on impact, worth recording — not a reason to weaken the fix.
- No mitigations exist otherwise: no canonicalization/`starts_with(root)`
  confinement anywhere (`Vault::new`/`Layout::new` take the root verbatim,
  `src/vault/mod.rs:29-33`, `layout.rs:45-47`); `create_dir_all(&view_dir)`
  (`materialize.rs:52`) is a no-op on an existing external dir.

## Related integrity issues, same root (`src/gitignore.rs`)

- **Newline injection.** View names are interpolated raw into `.gitignore`
  entries (`format!("/{}/", rel.display())`, `gitignore.rs:145-154`). A TOML name
  containing a newline injects arbitrary `.gitignore` lines (e.g. an `*` line
  that hides files from `git status`).
- **Panic on absolute name.** `dir.strip_prefix(root).expect(...)`
  (`gitignore.rs:149-151`) panics for an **absolute** name (`join` replaced the
  base, so `root` is not a prefix). A `..`-relative name does **not** panic
  (`strip_prefix` is textual: `/vault/../x` keeps the `/vault` prefix) and
  yields a nonsense `/../x/` entry instead — so the `..` variant is *quieter*,
  not louder. Either way, `view::sync_all` runs **before** `gitignore::sync`
  (`src/reconcile.rs:85-87`), so the panic never prevents the deletion — the
  files are already gone. **Remove this `.expect()` regardless of the main fix**;
  a library that panics on attacker-influenced input is a DoS surface on its own,
  and here it protects nothing.

## Reachability / which commands trigger it

`sync_view`/`sync_all` run on essentially every mutating command, each loading
names from config first:
- `view add` / `view remove` → `add_view`/`remove_view` (`run/mod.rs:321,325`;
  `add_view` also syncs directly).
- `reconcile` → `reconcile::reconcile` (`run/mod.rs:228`).
- `new`, `today` (no-edit paths), the editor-exit path, and `delete` →
  `refresh_views` (`run/mod.rs:206,220,368`, `src/ops/delete.rs:23-26`).

So merely `ntropy today` inside a vault whose `config.toml` carries a hostile
view name triggers the deletion.

## Fix — recommended architecture

**Introduce a validating newtype `ViewName` with a fallible constructor
("parse, don't validate").** This is preferred over both a fallible `view_dir`
and scattered per-site checks.

- `struct ViewName(String)` constructed only via
  `ViewName::parse(&str) -> Result<ViewName, InvalidViewName>`.
- `Layout::view_dir` takes `&ViewName` and **stays infallible** — the type is
  the proof, so no `Result` plumbing leaks into the ~8 call sites, the pure-path
  tests, or the seed view (`by-tag`, `src/…/init.rs:71`). `view_dir` remains
  pure path arithmetic, which is correct.
- `ViewDef.name: ViewName` (`src/view/mod.rs`) carries the proof. It becomes
  *impossible to construct a view path from an unvalidated string*, enforced by
  the compiler rather than by remembering to call a checker.

Why not the alternatives:
- **Fallible `view_dir -> Result`** spreads error handling into pure-path tests
  and the always-valid seed, forces `.expect()` at innocuous sites, and applies
  *policy* (hard error vs. skip) at the wrong layer/depth — no clean place for
  "skip one bad view with a warning".
- **Validate-at-each-entry-point** is a "remember to call it everywhere"
  contract the next new call site can silently break. Its *insight* is right and
  the newtype keeps it (validation still logically happens at the CLI-`add` and
  config-load boundaries); the newtype just makes "did we validate?"
  un-forgettable.

**Do NOT validate inside `PerVaultConfig::load`.** That path also backs
`list_views` and `remove_view` (`view_admin.rs:35-37,72-80`). If load rejects a
poisoned config, the user can no longer *inspect or remove* the bad entry —
trapping them. Keep `PerVaultConfig`/`ViewConfig.name` a faithful, permissive
`String` that round-trips; validate at the **materialization boundary**
(`ViewDef` construction, reached by `load_views` in `reconcile.rs:190-197` and
by `add_view`), i.e. exactly where a name becomes a filesystem path. `ViewDef`
is "a view we are about to materialize", so it is the natural home.

**Concrete churn to disclose:** touches `ViewDef`/`ViewConfig` field types and
`::new` signatures, the `gitignore::sync` signature (`&[&str]` today,
`gitignore.rs:142`), and a handful of tests passing bare `"by-tag"`. Needs one
new error variant (reuse `ViewAdminError` or add to `src/error.rs`). Bounded,
in exchange for a compiler-enforced guarantee.

## Validation ruleset

A valid view name is a non-empty `/`-separated sequence of segments where:

1. The whole string is non-empty.
2. It contains no NUL and no other control character (C0/C1, incl. `\n`, `\r`,
   `\t`). Closes the `.gitignore` newline injection (`gitignore.rs:152`).
3. Split on `/`: at least one segment, **no empty segment** (rejects leading/
   trailing `/` and `a//b`).
4. Each segment:
   - is not `.` or `..` (closes traversal);
   - does not start with `.` (closes `.git`, `.ntropy`, `.gitignore`, hidden
     dirs generally — a view group dir has no reason to be hidden);
   - contains no `\` (Windows separator; reject even on Unix as a hostile
     filename char);
   - **equals its own `text::slug::normalize_segment` output** (`[a-z0-9-]`,
     already collapsed/trimmed). This single rule also forbids uppercase,
     spaces, `:`, drive letters, leading `/`, control chars, and non-ASCII, and
     *aligns view names with the existing group-value/slug rules*
     (`src/text/slug.rs:38-75`).
5. The **first** segment is not in `RESERVED_NAMES` (`layout.rs:34`) — applied
   to the segment, not the whole string (closes `all-notes/x`, `.ntropy/x`).

**Recommendation: adopt rule 4's slug-equality check.** It reuses
`normalize_segment` (no new crate), makes the name-space a strict subset of
already-safe strings, and is Windows-safe as a side effect (excludes `\`, `:`,
trailing dots/spaces, and the chars in `CON/PRN/AUX/NUL/COM#/LPT#`). Add a
`// TODO(windows)` that a looser ruleset would need explicit device-name checks
if Windows support lands (ADR 0020 is Unix-only for v1).

**Tightening caveat (deliberate behavior change):** names `add` currently
accepts like `By_Tag`, `my.view`, `area work` would now be **rejected**. That is
desirable for a security fix and keeps names consistent with everything else
ntropy slugifies, but call it out. If a more permissive space is wanted, the
fallback is rules 1–3 + "no leading dot, no `.`/`..`, no `\`, no control chars"
(permits underscores/mixed case) — still closes every known vector.

**Multi-segment decision: keep nesting, validate per-segment.** Multi-segment
names (`area/work`) are a deliberate, tested feature at the gitignore layer
(`gitignore.rs:339-347,390-395`, `sync_derives_multi_segment_entry`) and via
group nesting in materialize. Per-segment validation preserves `area/work` while
making `..`/absolute/dotfile segments impossible, so keeping nesting costs
nothing in safety. (Restricting to one segment is also safe but regresses that
tested feature; only do it if nesting is being dropped as a product decision.)

## Hostile-config policy (split by provenance)

- **`add_view` (user just typed it):** **hard-error.** Reject immediately with a
  clear message alongside the existing `ReservedName`/`Duplicate` variants
  (`ViewAdminError`, `view_admin.rs:24-32`).
- **Names loaded from an existing config (`load_views`, `reconcile.rs:190`):**
  **skip the invalid view with a warning, continue with the rest.** This is the
  fail-safe direction *because the operation is destructive*: a skipped view is
  simply not materialized → **nothing is deleted** for it. Hard-erroring the
  whole command on one bad entry buys no extra safety (you refuse to act on it
  either way) and instead breaks every *legitimate* view and every mutating
  command for a user with a poisoned/corrupted config — a worse availability
  outcome. Route the warning through the existing `ScanWarning` /
  `ReconcileReport.warnings` channel so `--strict` promotes it to a non-zero
  exit (`run/mod.rs` `exit_for_warnings`), matching ADR 0019's scan-robustness
  contract. **Filter invalid names in `load_views` *before* `view::sync_all`
  runs** (`reconcile.rs:85`) — the filtering point matters; reordering alone is
  not a fix.
- **`list_views` / `remove_view`:** **do not validate** — keep them working on
  invalid entries so a poisoned config is inspectable (`list`) and purgeable
  (`remove`, which only edits config and never materializes). This is *why*
  validation must live at `ViewDef`/materialization, not at load.

## Empirical PoC / regression test (sandboxed, safe)

Code reading is conclusive on the mechanism; still write the PoC as the
regression test (guards against a missed mitigation and pins the fix). A Rust
integration test, fully under `tempfile`, no real paths, no binary:

```
base = tempdir()                                  // everything auto-cleaned
base/decoy/important.txt        (junk)            // victim tree OUTSIDE vault, INSIDE base
base/decoy/sub/nested.txt       (junk)
base/vault/.ntropy/             (mkdir → is a vault)
base/vault/all-notes/           (mkdir, EMPTY → desired set empty → maximal delete)
base/vault/.ntropy/config.toml:
    [[view]]
    name  = "../decoy"          // relative traversal, confined to base
    field = "tags"

reconcile::reconcile(Vault::new(base/"vault")).unwrap();   // or refresh_views

assert!(!exists base/decoy/important.txt);        // proven deleted (pre-fix)
assert!(!exists base/decoy/sub);                  // emptied dir pruned
assert!(exists base/decoy);                       // view root itself kept
```

Use `name = "../decoy"` (not an absolute path) so the blast radius is provably
confined to the auto-cleaned `TempDir` even if the assertion logic is wrong. Add
a second case `name = ".git"` (create `base/vault/.git/HEAD` + a `refs/` file)
to prove the no-traversal dotfile variant guts an in-vault git dir. **After the
fix:** both configs produce a skipped-view **warning** and leave the
decoy/`.git` files **intact** — that is the regression assertion. A `--strict`
variant asserts a non-zero exit. Manual CLI form is the same shape with
`ntropy reconcile --vault <tmp>/vault` (use `--vault` to eliminate any chance of
walk-up hitting a real vault); keep it strictly inside a throwaway temp dir.

Unit tests to add: `add` rejects `/etc`, `../x`, `.git`, `.ntropy`, `all-notes`,
`all-notes/x`, `a//b`, `""`, and a `\n`-bearing name; `add` accepts `by-tag`,
`by-status`, `area/work`.

## Anything a thorough fix must not forget

- Do not frame the fix as "confine to root" — that leaves class 2 (`.git`,
  dotfiles) and class 3 (`all-notes/x`) open. Per-segment "no leading dot" +
  first-segment-reserved are what close them.
- Remove the `gitignore.rs:149-151` `.expect()` regardless of the primary fix.
- Keep `remove_view`'s "never delete the directory" invariant
  (`view_admin.rs:69-80`) — a formerly-materialized hostile dir (if one ever got
  created pre-patch) is left on disk for the user, consistent with that
  invariant.
- Confirm the seed `by-tag` and `area/work` still validate and that a valid
  config round-trips through `to_toml`/`load` unchanged (`per_vault.rs:98-106`).
- Record the non-atomic partial-deletion property in the threat write-up
  (lexically-earlier files are gone even on an aborted run; no rollback).

## Acceptance

- No view name containing traversal / absolute / leading-dot / reserved-first-
  segment / control-char content is ever turned into a `view_dir`, at any entry
  point — enforced structurally (newtype) so a new call site cannot regress it.
- A crafted hostile `config.toml` cannot make `sync_view` touch a path outside
  the validated view directory — proven by the out-of-vault decoy test surviving
  a sync; the `.git` in-vault case survives too.
- Invalid names in an existing config are skipped with a warning (`--strict`
  → non-zero exit); `add` hard-errors; `list`/`remove` still work on a poisoned
  config.
- `.gitignore` sync cannot be made to inject lines or panic via a hostile name.
- The multi-segment decision is recorded (kept + per-segment validation, or
  dropped) and legitimate views still materialize; existing tests stay green.
