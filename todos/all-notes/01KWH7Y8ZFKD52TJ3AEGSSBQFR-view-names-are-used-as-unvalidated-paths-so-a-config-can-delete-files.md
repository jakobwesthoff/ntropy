---
title: "View names are used as unvalidated paths, so a config can delete files outside the vault"
kind: bug
component: view
impact: critical
origin: review
tags: [security]
---
# View names are used as unvalidated paths, so a config can delete files outside the vault

Found in the 2026-07-02 security review of view handling. A view name from
`config.toml` becomes a directory, and the next view sync recursively deletes
every file and symlink in it. A hostile or hand-edited config can therefore
delete files outside the vault. Plain commands reach it: `view add`,
`reconcile`, and any mutating command that refreshes views, including
`ntropy today`. Combined with the walk-up discovery that adopts a config from
a cloned or synced tree, it becomes a drive-by: clone a tree, change into it,
run `ntropy today`, and the named directory is deleted without confirmation
or rollback.

## Problem

Confirmed by code reading during the 2026-07-02 security review. An
independent triage pass reproduced the path mechanics with `rustc` and traced
the delete loop end to end. The empirical integration test in the regression
section is the end-to-end proof.

### Root cause

`Layout::view_dir` in `src/vault/layout.rs` computes a view's output
directory as follows:

```rust
pub fn view_dir(&self, name: &str) -> PathBuf {
    self.root.join(name)
}
```

`root` is absolute and canonical. `Path::join` then misbehaves with an
attacker-controlled `name` in two ways, both reproduced during triage:

- An absolute `name`, such as `/etc` or `/home/user/Documents`, replaces the
  base entirely, so the result is not under the vault at all.
- A relative `name` with `..`, such as `../../Documents`, is kept verbatim
  without lexical normalization, so it resolves outside the vault at syscall
  time.

A name does not need to escape the vault to do damage. `.git` stays inside
the vault and is still a fully relative, single-segment name. `view_dir` is
infallible and performs no validation.

### The destructive primitive

`sync_view` in `src/view/materialize.rs` is not a simple create and delete of
a few symlinks. It works as follows.

1. It resolves `view_dir` from the view name.
2. `collect_state` recursively lists that directory, descending into every
   real subdirectory, and builds the whole map in memory before any deletion.
   `read_dir_entries` in `src/fsutil.rs` does not follow symlinks, so the
   recursion stays in real directories.
3. Every entry that is not one of the view's desired leaves goes through
   `fsutil::remove_file`. For an arbitrary external tree the desired set is
   empty or mismatched, so every regular file and every symlink is removed. The
   test `a_stray_non_leaf_file_inside_a_group_is_removed` pins exactly this
   behaviour: anything that is not a leaf is deleted.
4. Emptied directories are pruned deepest first with `remove_dir_if_empty`.
   The view's own root is kept.

For a name pointing at `~/Documents`, the next sync deletes every regular file
and symlink under it, recursively, and removes every directory that becomes
empty.

### Deletion is non-atomic and has no rollback

`remove_file` propagates its error with `?`, and the entries are a sorted
`BTreeMap`. Files are therefore deleted in sorted path order until the first
`EACCES` or `EPERM`, and then the command aborts. Against a user-owned tree,
such as the user's documents or their own repository's `.git`, every file is
removable and the run completes. Against a system path it deletes the
lexically earlier prefix and then fails, which still destroys data. Files
already removed are not restored.

### Where the names come from

Neither path sanitizes the name.

- The `view add` command checks only exact equality against the reserved names
  `all-notes`, `.ntropy` and `.gitignore` (`is_reserved_name` in
  `src/vault/layout.rs`). It does not reject `/`, `..`, absolute paths, leading
  dots or control characters. `add_view` in `src/ops/view_admin.rs` then calls
  `sync_view` directly, so `ntropy view add "../escape" tags`,
  `ntropy view add ".git" tags`, `ntropy view add "/etc" tags` and
  `ntropy view add "all-notes/x" tags` all run today.
- Loading `config.toml` applies no validation. `PerVaultConfig::load` in
  `src/config/per_vault.rs` is plain serde. Names from the config flow through
  `load_views` in `src/reconcile.rs` to `sync_all` and then `sync_view`. TOML
  strings can carry `..`, absolute paths, leading dots, control characters and
  newlines.

### Three separate vector classes

The fix has to close all three. Confining names to the vault root closes only
the first.

1. Traversal and absolute paths, such as `../../Documents` and `/etc`.
   They escape the vault.
2. In-vault dotfiles. `.git` is a single relative segment with no `..`, and it
   is not reserved. A sync against a mismatched desired set empties the
   repository's git directory. No traversal is needed, and the same applies to
   any leading-dot directory.
3. Reserved-prefix bypass. The check compares the whole string, so `all-notes/x`
   and `.ntropy/x` pass. `view_dir` then writes inside the notes or config
   directory, and the sync deletes real notes or templates there. The reserved
   check must apply to the first path segment.

### What is already safe

- Group values, the subdirectories created under a view, pass through
  `tag::normalize` and `slug::normalize_segment` in `src/text/tag.rs` and
  `src/text/slug.rs`. Those strip everything except `[a-z0-9-]`, with `/`
  between segments. Only the view name is unvalidated, so the whole fix surface
  is that one string.
- `read_dir_entries` does not follow symlinks. Deletion therefore stays inside
  the named tree and removes a symlink entry itself without following it. This
  limits the impact and is worth recording, but it is no reason to weaken the
  fix.

### Related integrity issues in `src/gitignore.rs`

- Newline injection. View names are interpolated raw into `.gitignore` entries
  in `gitignore::sync`. A name with a newline injects arbitrary lines, such as
  a `*` line that hides files from `git status`.
- Panic on an absolute name. The `dir.strip_prefix(root).expect(...)` call
  panics when `join` has replaced the base, because `root` is then not a
  prefix. A `..` name does not panic, since `strip_prefix` works on the text,
  and it writes a nonsense `/../x/` entry instead. `view::sync_all` runs before
  `gitignore::sync` in `src/reconcile.rs`, so the panic never prevents the
  deletion. The files are already gone by then. The `expect` protects nothing,
  and a panic on attacker-influenced input is a denial of service in its own
  right. Remove it whatever the main fix turns out to be.

### Reachability

`sync_view` and `sync_all` run on nearly every mutating command, and each one
loads names from the config first. That covers `view add` and `view remove`,
`reconcile`, `new`, `today` on paths that do not open the editor, the
editor-exit path, and `delete`. A plain `ntropy today` inside a vault with a
hostile config therefore triggers the deletion.

## Impact

Critical. The bug destroys user data outside the vault, which is the top level
in `format.md`. The old review rated it High and wrote that it must not drop
below High. The reach is limited only by which files the invoking user can
write. Even without a hostile vault, a typo in the user's own config can gut
the `.git` directory or the notes directory.

## Suggested fix

Introduce a validating newtype `ViewName` with a fallible constructor, so the
type is the proof that a name is safe. Parse, do not validate.

- `struct ViewName(String)`, constructible only through
  `ViewName::parse(&str) -> Result<ViewName, InvalidViewName>`.
- `Layout::view_dir` takes `&ViewName` and stays infallible. The type carries
  the proof, so no `Result` spreads into the roughly eight call sites, the
  pure path tests or the seed view `by-tag`. `view_dir` stays pure path
  arithmetic, which is correct.
- `ViewDef.name` in `src/view/mod.rs` has type `ViewName`. The compiler then
  rules out building a view path from an unvalidated string.

Two alternatives were rejected. A fallible `view_dir` spreads error handling
into pure path tests and the always-valid seed, forces `expect` calls at
harmless sites, and puts the policy of hard error versus skip at the wrong
layer. Validating at each entry point depends on every future call site
remembering to validate. The newtype keeps the entry point checks, but makes
forgetting them impossible.

### Validation rules

A valid view name is a non-empty sequence of segments separated by `/`, where:

1. The whole string is non-empty.
2. It contains no control character, including NUL, `\n`, `\r` and `\t`. This
   also closes the `.gitignore` newline injection.
3. Splitting on `/` gives at least one segment, and no segment is empty. This
   rejects a leading or trailing `/` and `a//b`.
4. Each segment is not `.` or `..`, does not start with `.`, contains no `\`,
   and equals its own `text::slug::normalize_segment` output. The last rule
   allows only `[a-z0-9-]`, already trimmed and collapsed. It also forbids
   uppercase, spaces, `:`, drive letters, control characters and non-ASCII, and
   it lines view names up with the existing slug rules. It excludes the Windows
   reserved device names and trailing dots and spaces as well. Add a
   `// TODO(windows)` noting that a looser rule set would need explicit device
   name checks if Windows support lands. ADR 0020 is Unix only for v1.
5. The first segment is not in `RESERVED_NAMES`. The check applies to the
   segment, not the whole string, which closes `all-notes/x` and `.ntropy/x`.

Multi-segment names are kept. Nesting is a tested feature at the gitignore
layer (`sync_derives_multi_segment_entry`) and through group nesting in
materialization. Per-segment validation keeps `area/work` working and makes
`..`, absolute and dotfile segments impossible. Dropping nesting would also be
safe, but it removes a tested feature, so only do that as a product decision.

Behaviour change to disclose: names `add` accepts today, such as `By_Tag`,
`my.view` and `area work`, would now be rejected. That is desirable for a
security fix and matches how the rest of ntropy slugifies names. If a more
permissive set is wanted, the fallback is rules 1 to 3 together with no leading
dot, no `.` or `..`, no `\` and no control characters. That still closes every
known vector and allows underscores and mixed case.

### Hostile config policy

Split by where the name comes from.

- `add_view`, where the user typed the name: hard error. Reject with a clear
  message next to the existing `ReservedName` and `Duplicate` variants in
  `ViewAdminError`.
- Names loaded from an existing config, through `load_views`: skip the invalid
  view with a warning and continue with the rest. This is the safe direction
  because the operation is destructive. A skipped view is never materialized,
  so nothing is deleted for it. A hard error on one bad entry would refuse
  every legitimate view and every mutating command for a user with a damaged
  config, which is worse for availability and gives no extra safety. Route the
  warning through the existing `ScanWarning` and `ReconcileReport.warnings`
  channel, so `--strict` turns it into a non-zero exit under ADR 0019. Filter
  invalid names in `load_views` before `view::sync_all` runs in
  `src/reconcile.rs`. The position of the filter matters. Reordering alone is
  not a fix.
- `list_views` and `remove_view`: no validation. Keep them working on invalid
  entries, so a damaged config can be inspected with `list` and purged with
  `remove`. `remove` only edits the config and never materializes anything.
  This is why validation belongs at `ViewDef` construction, not at load.

### Churn

The change touches the field types and constructors of `ViewDef` and
`ViewConfig`, the signature of `gitignore::sync` (currently `&[&str]`), and a
few tests that pass bare `"by-tag"`. It needs one new error variant, which can
reuse `ViewAdminError` or be added to `src/error.rs`. That is bounded work in
exchange for a guarantee the compiler enforces.

## Regression test

Code reading settles the mechanism, but the PoC should still become the
regression test, since it guards against a missed mitigation. Write it as a
Rust integration test under `tempfile`, with no real paths and no binary.

```
base = tempdir()                                  // everything auto-cleaned
base/decoy/important.txt        (junk)            // victim tree outside the vault, inside base
base/decoy/sub/nested.txt       (junk)
base/vault/.ntropy/             (mkdir, makes it a vault)
base/vault/all-notes/           (mkdir, EMPTY, so the desired set is empty and deletion is maximal)
base/vault/.ntropy/config.toml:
    [[view]]
    name  = "../decoy"          // relative traversal, confined to base
    field = "tags"

reconcile::reconcile(Vault::new(base/"vault")).unwrap();   // or refresh_views

assert!(!exists base/decoy/important.txt);        // deleted before the fix
assert!(!exists base/decoy/sub);                  // emptied directory pruned
assert!(exists base/decoy);                       // view root kept
```

Use `name = "../decoy"` rather than an absolute path, so the blast radius
stays provably inside the temporary directory even if the assertions are
wrong. Add a second case with `name = ".git"`, creating `base/vault/.git/HEAD`
and a `refs/` file, to show that the no-traversal dotfile variant empties an
in-vault git directory. After the fix, both configs must produce a skipped view
warning and leave the decoy and `.git` files intact. A `--strict` variant
asserts a non-zero exit. The manual CLI form has the same shape, with
`ntropy reconcile --vault <tmp>/vault`. Always pass `--vault`, so walk-up can
never reach a real vault, and keep everything in a throwaway temporary
directory.

Unit tests to add: `add` rejects `/etc`, `../x`, `.git`, `.ntropy`,
`all-notes`, `all-notes/x`, `a//b`, the empty string and a name containing
`\n`. `add` accepts `by-tag`, `by-status` and `area/work`.

## Further notes

- Do not frame the fix as confining names to the root. That leaves the dotfile
  class and the reserved-prefix class open. Per-segment "no leading dot" and
  "first segment not reserved" close them.
- Keep the `remove_view` invariant that the directory itself is never deleted.
  A directory that a pre-fix run already created is left on disk for the user.
- Confirm that the seed `by-tag` and `area/work` validate, and that a valid
  config survives a round trip through `to_toml` and `load` unchanged.
- The threat write-up should record that deletion is partial and not atomic.
  Files earlier in sorted order are gone even when the run aborts, and nothing
  is rolled back.

## Done when

- No view name containing traversal, an absolute path, a leading dot, a
  reserved first segment or a control character is ever turned into a
  `view_dir`, at any entry point. The newtype enforces this, so a new call
  site cannot regress it.
- A hostile `config.toml` cannot make `sync_view` touch a path outside the
  validated view directory. The out-of-vault decoy test and the in-vault `.git`
  test both keep their files after a sync.
- Invalid names in an existing config are skipped with a warning, and `--strict`
  exits non-zero. `add` hard-errors. `list` and `remove` still work on a damaged
  config.
- `.gitignore` sync cannot be made to inject lines or panic through a hostile
  name.
- The multi-segment decision is recorded in the code or the docs. Legitimate
  views still materialize, and the existing tests stay green.
