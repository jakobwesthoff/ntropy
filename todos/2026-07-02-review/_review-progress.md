# Codebase review progress — 2026-07-02

Full-codebase review, structured into small independent units. Findings are
written as individual todos in this folder immediately upon discovery.
Security findings go to `security_review_queue.md` and are processed after
all review units are done.

Note: a previous session reportedly did review work, but no trace exists in
the repo (no folder, no git history, no stash). This review starts from
scratch.

## Review units

- [x] 01 Core identity & text: `src/id.rs`, `src/text/slug.rs`, `src/text/tag.rs`, `src/note/filename.rs`
- [x] 02 Note parsing: `src/note/mod.rs`, `src/note/frontmatter.rs`, `src/datetime.rs`
- [x] 03 Scanning & fs utilities: `src/scan.rs`, `src/fsutil.rs`
- [x] 04 Vault resolution & config: `src/vault/*`, `src/config/*`
- [x] 05 Query engine: `src/query/*`
- [x] 06 Links: `src/link/*`
- [x] 07 Views & reconcile: `src/view/*`, `src/reconcile.rs`, `src/gitignore.rs`
- [x] 08 Ops layer: `src/ops/*`
- [x] 09 Templates: `src/template.rs`
- [x] 10 CLI runtime: `src/bin/ntropy/{main.rs,cli.rs}`, `run/{mod.rs,output.rs,editor.rs,interact.rs}`
- [x] 11 Picker TUI: `src/bin/ntropy/run/picker/*`
- [x] 12 LSP server: `src/bin/ntropy/run/lsp/*`
- [x] 13 Tests & fixtures: `tests/*`, `src/test_support.rs`, `examples/generate_vault.rs`
- [x] 14 Packaging & CI: `Cargo.toml`, `.github/`, untracked `dist/`, docs/pages
- [ ] 15 Error handling & lib API surface: `src/error.rs`, `src/lib.rs`, cross-cutting
- [x] 16 Process security_review_queue.md

## Findings written so far

- `01kwh5hhwgqwmr1eghhtmed79t-filename-parse-panics-on-multibyte-names.md`
  (unit 01, verified panic repro)
- `01kwh5hhwgqwmr1eghhtmed79v-frontmatter-non-string-title-misleading-error.md`
  (unit 02)
- `security_review_queue.md`: SEC-1 (YAML DoS robustness), SEC-2 (scan panic
  cross-ref)
- `01kwh61rsgshq4d437y920wzq1-query-tokenizer-error-promises-literal-search.md`
  (unit 05)
- `01kwh61rsgshq4d437y920wzq2-query-field-predicate-case-and-keyword-shadowing.md`
  (unit 05)

- `01kwh65bxfx18qv8hd00949hzq-code-masking-limitations-multiline-spans-and-docs.md`
  (unit 06)

- `01kwh6c1ves9yjbescn8n2cmrz-remove-dir-if-empty-enotempty-race.md` (unit 07,
  resolves carried question from unit 03: ENOTEMPTY is NOT tolerated today)
- `01kwh6c1ves9yjbescn8n2cms0-view-leaf-name-collisions-across-groups.md`
  (unit 07)
- `01kwh6c1ves9yjbescn8n2cms1-scan-does-not-detect-duplicate-note-ids.md`
  (unit 07; reconcile rename clobber = silent data loss)
- `security_review_queue.md` SEC-3 updated: CONFIRMED by code reading, incl.
  add-time gap (`../x`, `.git`, `all-notes/sub` all accepted), no load-time
  validation, recursive delete primitive in sync_view, gitignore newline
  injection. Empirical PoC deferred to queue processing (unit 16).

- `01kwh6h9cqb4g2h9950fhk5fbz-md-extension-casing-inconsistent-across-modules.md`
  (units 08/09)

- `01kwh6mqt7t5cje5j1pqysvdqa-editor-env-var-with-arguments-fails-to-launch.md`
  (unit 10)
- `security_review_queue.md` SEC-5 added: terminal escape injection via
  untrusted titles/tags/filenames in output.rs (picker/LSP surfaces to be
  cross-checked in units 11/12).

- `01kwh6qrnamfx7mm7gt6nm4cj0-picker-prompt-line-not-clipped-to-terminal-width.md`
  (unit 11)
- SEC-5 queue entry extended: picker confirmed affected (no control-char
  filtering in draw_row / align_candidates).

- `01kwh6xskhfggksmkce75ncexz-cli-tests-not-hermetic-global-config.md`
  (unit 13)

- `01kwh708xc27x7x1e7mfp2tfeb-crate-packaging-hygiene.md` (unit 14)
- `01kwh708xc27x7x1e7mfp2tfec-release-workflow-lacks-test-gate.md` (unit 14)
- `security_review_queue.md` SEC-6 added: CI supply-chain pinning.

Unit 14 notes: `just check` = clippy --all-targets -D warnings + tests +
fmt --check, a solid CI gate on ubuntu+macos. pages.yml carries a leftover
template comment header ("Copy this file to your project…") — folded into
nothing, too trivial to file. crates.io publish is manual (no publish job),
consistent with ADR 0022's v1 scope.

Unit 13 notes: tests/cli.rs is a comprehensive contract suite with careful
snapshot redaction (same-width ULID/date tokens to preserve alignment).
tests/views.rs and src/test_support.rs clean. examples/generate_vault.rs is
a deliberate, well-reasoned deterministic corpus generator (inlined
SplitMix64 for cross-version reproducibility, real slugify for validity) —
no findings.

Unit 12 notes: no findings filed — the LSP layer is the most rigorously
tested part of the codebase (offset conversion round-trips every char
boundary in both encodings; end-to-end tests over an in-memory connection;
auto-close/snippet-escape edge cases pinned). SEC-4 note: LSP vault
resolution uses ONLY walk-up from the document (no env/global-default
fallback), so a stray .md never binds to the default vault. Minor accepted
quirks, judged not worth filing: malformed request params answered with a
null result instead of an InvalidParams error; a failed scan caches an
empty entry list until the next invalidation; full document text cloned
per completion request.

Unit 11 notes: PickerState and layout are clean, comprehensively unit- and
snapshot-tested (multibyte delete_word, wide-char truncation, dual search/
display corpora). TerminalGuard teardown ordering is correct and tested.
Highlights are sorted+deduped in recompute, so draw_row's binary_search is
sound.

Unit 10 notes: SEC-4 partially resolved — editor comes ONLY from
$VISUAL/$EDITOR env (`run/editor.rs`), never from vault config, so a hostile
vault cannot choose the editor binary. Unit-08 carried question resolved:
`delete` paths always originate from `resolve_selection` (scan results), so
`delete_note` never receives arbitrary paths. Link slugs staying stale
between reconciles after an editor-driven rename is documented in ADR 0028
("Between reconcile runs a renamed target leaves a link's slug stale") — not
filed. output.rs table renderer is correct and thoroughly tested (unicode
width, no trailing whitespace).

Units 08/09 notes: ops layer clean overall. `template::load_named` rejects
empty names and path separators; a bare `..` name is harmless because `.md`
is appended (becomes filename `...md`). `today_note` has a benign TOCTOU
(two concurrent runs → two same-titled daily notes; doc acknowledges
duplicates, newest wins). `delete_note` trusts the caller-resolved path —
verify in unit 10 that selection only ever yields scanned note paths.
`search` scans before compiling the query (wasted scan on bad query, judged
too minor to file). tag::suggest raw-dedup question: not relevant to ops
(list_tags uses normalized note.tags); remains open for unit 12 (LSP).

Unit 07 notes: gitignore.rs sync/prune logic is clean (marker-ownership,
idempotence, user lines preserved). Carried unit-04 question resolved:
per-vault config writes in view_admin.rs DO go through `fsutil::atomic_write`
(lines 55, 78) — no amendment to the config-write todo needed (that todo
concerns the global config path). Group values are safe as path components:
`normalize_segment` restricts to `[a-z0-9-]` per segment, empty segments
dropped.

Unit 06 notes: `parse_target` guards `is_char_boundary(ULID_LEN)` (no
multibyte panic, unlike filename.rs). Badge-style nested links
`[![img](inner)](ULID.md)` are not extracted (regex limitation, judged
acceptable, not filed). `leading_spaces` doc says "capped at four" but
does not cap (harmless, callers only compare >3) — fold into unit 15 doc
nits if more accumulate. No security-queue items from unit 06.

Unit 05 notes: regex crate has linear-time matching and default compile size
limits, so hostile query patterns are not a DoS vector; smart-case AST walk
covers all literal-bearing node kinds; `tag::matches` normalizes both sides.
No security queue additions from unit 05 (queries are user-authored CLI
input; LSP-side query compilation to be checked in unit 12).

## Open questions carried forward

- Unit 07 must check: does `remove_dir_if_empty`'s read-then-remove race
  (concurrent LSP/CLI) surface as spurious sync errors? Decide if ENOTEMPTY
  on `remove_dir` should be tolerated.
- Unit 07/08 must verify SEC-3 (view-name traversal) and whether per-vault
  config names are validated at load/sync time, not just `view add` time.
- Unit 08: check whether per-vault config writes (`view add/remove`) go
  through `atomic_write`; amend the config-write todo if not.

- `tag::suggest` dedups on the raw candidate string, not the normalized form —
  RESOLVED in unit 12: both callers feed it already-normalized tags
  (LSP `unique_tags` over note.tags; note model normalizes at parse), so the
  raw-vs-normalized distinction never materializes. No bug, nothing filed.
- `src/lib.rs` + `src/error.rs` read; clean. Unit 15 stays open for
  cross-cutting checks (panic audit, `.context()` conventions, MPL headers).

## Unit 15 (in progress)

- MPL headers: all tracked `.rs` files carry the header on line 1. ✓
- Panic audit (unwrap/expect/panic!/unreachable!/assert in non-test code):
  all sites traced. Verified sound: frontmatter.rs:73 (checked-above expect,
  `\r\n` handled by `newline_len`), tag.rs:82 + haystack indexing (guarded),
  lsp completion tag.rs:79 (bracket presence implied by find), run/mod.rs
  unreachables (dispatch order verified in run()), scan.rs mutex expects,
  under_tags_key line_start==0 guard, Id::tail clamps n.
  FINDING → SEC-3 addendum in security_review_queue.md: absolute view name
  panics gitignore::sync strip_prefix expect (gitignore.rs:149-151), and the
  destructive view::sync_all runs BEFORE gitignore::sync so the panic does
  not preempt deletion.
- Slice/index audit (non-test): lsp offset.rs fully clamped (starts[0]=0
  makes binary_search index-1 safe); completion link.rs/tag.rs slice only at
  ASCII delimiters or clamped offsets; slug.rs truncation runs after the
  ASCII-only filter (step 4); template.rs render bounds implied by find;
  token.rs loops guarded; fsutil relative_path common ≤ both lens; leaf.rs
  duplicate-id identical-name fallout already captured in cms0 Problem 3 /
  cms1. No new findings beyond the SEC-3 addendum.
- lib.rs/error.rs API surface: FINDING →
  `01kwh7e42pxa5m9hpjgp6mbmd0-fserror-unnameable-in-public-api.md`
  (Error::Fs payload is a Voldemort type; fsutil is pub(crate), no re-export).

## Unit 16 — security queue processed (2026-07-02)

All six SEC candidates triaged (code review + one dedicated Fable security
pass each, run strictly one at a time) and written as extensive, self-contained,
implementation-ready todos. `security_review_queue.md` is now empty. Fable
agents were dispatched in preliminary-severity order (highest first). Notable
triage outcomes that changed the original framing:

- SEC-3 (HIGH) → `01kwh7y8zfkd52tj3aegssbqfr-...`. Fix = `ViewName` validating
  newtype (parse-don't-validate); validate at materialization boundary, skip
  bad config entries with a warning; not just traversal — `.git`/`all-notes/x`
  dotfile & reserved-prefix classes too. PoC = sandboxed decoy-deletion test.
- SEC-5 (Medium) → `01kwh7y8zfkd52tj3aegssbqft-...`. Fix = one
  `sanitize_for_terminal` helper, unconditional (not isatty-gated), escaping
  C0/DEL/C1 + bidi; tags are already normalize-safe; sanitize before width math;
  picker highlight indices depend on it. LSP + reconcile-link-target are extra
  surfaces.
- SEC-4 (Low residual) → `01kwh7y8zfkd52tj3aegssbqfs-...`. Verified NO config
  value selects any executable (only editor, from $VISUAL/$EDITOR). Recommend
  document-don't-gate ADR with a "no execution in config" invariant; uid-check
  as pre-decided escalation. Needs user sign-off before writing the ADR.
- SEC-1 (Low) → `01kwh7y8zfkd52tj3aegssbqfq-...`. Empirically overturned:
  serde_yaml_ng 0.10.0 already bounds billion-laughs (repetition limit) and deep
  nesting (depth 128, no stack overflow). REAL vector = O(n²) CPU in libyaml's
  flow-collection load phase; fix = 16 KiB block byte cap + independent
  `load_note` file-size cap.
- SEC-2 (Medium) → `01kwh5hhwgqwmr1eghhtmed79t-...` (existing todo rewritten).
  Panic → CPU-burning livelock via the `ignore` scoped-thread walker. Fix =
  `split_at_checked` → `FilenameError::Id`. Sweep confirmed it's the ONLY
  unguarded fixed-offset `&str` split on hostile input. Optional follow-up:
  `catch_unwind` in `load_note` (flagged to user, not yet filed).
- SEC-6 (Low-Medium) → `01kwh7y8zfkd52tj3aegssbqfv-...`. Decided policy: SHA-pin
  all actions + `# vX.Y.Z`, SHA-pin generator checkout, `--frozen-lockfile`,
  per-job permission split in pages.yml (highest-value single change),
  `persist-credentials: false`, Dependabot.
