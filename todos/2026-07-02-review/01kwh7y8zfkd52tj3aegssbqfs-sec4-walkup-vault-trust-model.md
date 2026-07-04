# SEC-4: Walk-up vault discovery silently adopts untrusted config/templates/notes

> **STATUS: TRIAGED (code review + Fable security triage, 2026-07-02).**
> Trust-model / design finding, not a single bug. Fable independently verified
> the execution-influence surface (swept the whole tree for spawns/env/config)
> and produced a ranked recommendation. **Recommended outcome: do NOT build a
> trust boundary; harden the primitives (SEC-3/SEC-5/SEC-1, tracked separately)
> and record the threat model in a new ADR with a binding "no execution in
> config" invariant.** This recommendation is a proposal for the user to accept;
> the ADR must not be written as a decided record until the user confirms the
> "document, don't gate" direction.

## Summary

`vault::resolve` (`src/vault/resolve.rs`) discovers the active vault by walking
up from the cwd, honoring a `.ntropy-vault` pointer file and a `.ntropy/`
directory at each ancestor. Running *any* ntropy command inside an untrusted
tree (cloned repo, synced folder, extracted archive) silently adopts that tree's
vault: its `config.toml` view definitions, its templates, and its notes. No
trust prompt, allowlist, or opt-in comparable to git's `safe.directory` (added
after CVE-2022-24765). The finding's weight is as an *amplifier* of SEC-3
(destructive view sync), SEC-5 (terminal injection), and SEC-1 (YAML DoS).

## Severity: LOW residual (once SEC-3/SEC-5/SEC-1 land)

Downgraded from "Medium as amplifier". **SEC-4 cannot escalate to code
execution** (see the verified surface below); its blast radius is
data-integrity / DoS / display, which the three primitive fixes already cover.
What remains after those:
- **Write misdirection**: commands act on a vault the user didn't consciously
  choose — confined to legitimate vault ops (note creation, `reconcile`
  renames, view sync, `.gitignore` sync). `delete` still requires a selector +
  confirmation or `--force` (`run/mod.rs:296-304`).
- **Mild self-disclosure**: notes created in a planted vault land in an
  attacker-readable location (e.g. a `/tmp` ancestor; walk-up runs to `/`,
  `resolve.rs:115`). No network → no exfiltration channel.
- **Symlinked notes**: reads follow symlinks (`scan.rs:135`), so a hostile vault
  could symlink `all-notes/x.md` at a user file — but the target must *parse as
  a note* (frontmatter with a title) to surface, excluding almost all real
  files; and every write path *replaces* the symlink rather than following it
  (`atomic_write` renames over the link, `fsutil.rs:72-91`; `remove_file` removes
  the link, `fsutil.rs:99-101`).
- **Template text**: inert attacker-chosen prose in a newly created note.
- **The real residual is future config growth** — see the binding invariant.

Standalone, this reduces to a **documentation-plus-invariant item**, not a code
change.

## Where / how discovery works

`resolve_with_source` (`src/vault/resolve.rs:66-92`) order: `--vault` >
`$NTROPY_VAULT` > cwd **walk-up** (`walk_up`/`walk_up_with`,
`resolve.rs:104-131`) > global `default_vault`. The walk-up iterates
`start.ancestors()`; at each dir a `.ntropy-vault` pointer wins over a same-dir
`.ntropy/`, nearest ancestor wins. No confirmation; adoption is fully implicit.

### The pointer file
`resolve_pointer` (`resolve.rs:137-161`) reads the first line of `.ntropy-vault`
and treats it as a path that may be absolute, relative to the pointer's dir, or
`~`-expanded (`expand_tilde`, `resolve.rs:164-175`); the only constraint is the
target must already be a vault. So a `.ntropy-vault` committed into a cloned repo
can redirect ntropy to any pre-existing vault. **But**: once redirected,
*everything* (config, templates, notes) comes from the **target** vault — the
hostile tree contributes only the redirect. A redirect to the user's own vault
therefore yields only ordinary, config-legitimate operations on that vault, and
post-SEC-3 those cannot escape it. Not a meaningfully worse vector than plain
`.ntropy/` adoption; see the recommendation against a narrower pointer
mitigation below.

## Execution-influence surface — VERIFIED EMPTY (the crux)

Fable swept `src/` for process spawns (`Command`/`spawn`/`exec`/`.status(`/
`.output(`) and env reads (`env::var`/`var_os`/`env!`/`option_env!`):

- **Exactly one subprocess spawn in the whole binary**: `Command::new(&editor)`
  (`src/bin/ntropy/run/editor.rs:20`). The editor comes only from `$VISUAL` then
  `$EDITOR` (`editor.rs:35-46`); no config fallback, no built-in default (missing
  env is a hard error, `editor.rs:43-45`). Other grep hits are crossterm's
  `execute!` terminal macro (`picker/mod.rs:72,79`) and `thread::spawn` in LSP
  *tests* only.
- **Complete env-read inventory**: `VISUAL`/`EDITOR` (`editor.rs:37`),
  `NTROPY_VAULT` (`run/mod.rs:97`), `current_dir` (`run/mod.rs:98,122`). Nothing
  else.
- **Complete config schema**: per-vault (`src/config/per_vault.rs:19-31`) is
  `views: Vec<ViewConfig { name, field }>` — that is the *entire* schema. `name`
  becomes a path component (SEC-3); `field` is only a frontmatter-key lookup
  (`materialize.rs:167-186`). Global (`src/config/global.rs:27-32`) is
  `default_vault: Option<PathBuf>` only, validated through `require_vault`, and
  lives in the user-owned OS config dir (not attacker-writable). **No pager, no
  hooks, no shell, no "open with" anywhere.**
- **Templates**: `render` (`src/template.rs:115-157`) is a single-pass `{{key}}`
  substitution against a closed four-key set (`title`/`id`/`date`/`slug`); no
  eval, inclusion, or command mechanism. `load_named` rejects empty names and
  path separators (`template.rs:93-98`).
- **LSP**: read-only request surface (completion/definition/documentLink/
  workspaceSymbol, `lsp/mod.rs:224-232`); `for_document` sets only `start_dir`
  (no env/global fallback, `lsp/vault.rs:36-40`); the only non-test `fs::write`/
  `create_dir_all` in the LSP tree are `#[cfg(test)]`.

**Conclusion:** a hostile vault cannot choose the program ntropy launches, nor
influence any process/code execution. All three "NOT influenced" claims
confirmed.

**Adjacent soft spot (ADR non-goal, not a mitigation):** ntropy auto-opens
attacker-authored content in the user's editor — single-match `search` opens
immediately (`run/mod.rs:172-175`); `new`/`today` open a note rendered from a
(possibly hostile) template. The program is user-chosen but the content is
attacker-chosen; editor-side execution features (vim modelines, `exrc`) are
outside ntropy's control.

## Trust-boundary options — ranked recommendation

1. **(c) Do nothing at the trust layer; harden primitives + ADR — RECOMMENDED.**
   git's `safe.directory` was justified because repo-local config *selects
   executables* (hooks, `core.pager`, `core.fsmonitor`) — CVE-2022-24765 was
   code execution. ntropy's config selects no executable (verified), so the
   analogy's premise does not hold. With SEC-3/5/1 fixed, every gating option
   protects only against Low-severity misdirection at the cost of the tool's
   default UX (walk-up, ADR 0016) or disproportionate machinery for a v1 local
   single-user notes CLI. **Must be accompanied by the binding invariant below.**
2. **(b) uid ownership check — designated escalation, not v1.** Catches a
   foreign-uid `.ntropy/` in a world-writable ancestor (`/tmp` on a shared
   machine) and archives extracted as root. **Misses the dominant scenario** —
   a repo you cloned / a folder you synced, where every file is owned by you.
   Since it misses the headline attack and the primitives are hardened anyway,
   not worth its edge cases (sudo, containers, NFS uid mapping) now. Implement
   if an escalation trigger fires.
3. **(d) pointer restriction — rejected.** Disallowing absolute/`~` targets or
   confining below the pointer dir deletes ADR 0026's raison d'être ("a vault
   that lives elsewhere... external", `docs/adr/0026:12-15`) to close an
   already-Low misdirection vector. Visibility already exists via `ntropy info`
   (`ResolveSource::Pointer`, `resolve.rs:39-40`, `run/mod.rs:339-347`). A
   per-command stderr notice was considered and rejected (would fire constantly
   for the intended external-vault users).
4. **(a) trust store / `safe.directory` analogue — rejected for v1.** Git-level
   machinery for non-git-level consequences; taxes every legitimate multi-vault
   user.
5. **(e) walk-up opt-in — rejected.** Removes the default mode of operation to
   defend a surface hardening already closed. Worst cost/benefit.

### Interaction with the SEC-3 fix (strengthens the recommendation)
Once view names are validated (SEC-3's `ViewName` newtype), SEC-4's destructive
residual is effectively eliminated: plain adoption confines view sync to
validated dirs under the adopted (attacker's own) root; a pointer redirect syncs
the *target* vault using the *target's own* config (the user's, trusted), never
mixing hostile config with user data. Nothing left justifies a gate.

## Recommended ADR (decision sketch — write only after user accepts "document, don't gate")

A new ADR, "Trust model for discovered vaults":
1. **Precedence restated** (referencing ADRs 0016/0026, not duplicating).
2. **Trust assumption, stated plainly**: a vault discoverable by walk-up is
   trusted to the same degree as the cwd's contents. ntropy treats everything
   inside a vault as untrusted *data*, never instructions: display is sanitized
   (SEC-5), view names validated before becoming paths (SEC-3), frontmatter
   parsing bounded (SEC-1), templates are inert placeholder substitution.
3. **THE BINDING INVARIANT (load-bearing)**: no configuration value, per-vault or
   global, may name an executable, shell command, hook, pager, or argument
   passed to a spawned process. The only subprocess ntropy launches is the
   editor, resolved exclusively from `$VISUAL`/`$EDITOR`. Any feature that would
   break this invariant must supersede this ADR and introduce a trust boundary
   (uid check or trust store) *first*.
4. **Accepted residuals, named**: committed `.ntropy-vault` misdirection
   (operations use the target vault's own config; `delete` stays behind selector
   + confirmation); a planted vault in a world-writable ancestor is adoptable
   (walk-up ranges to `/`); auto-opened content may be attacker-authored and
   editor-side execution is out of scope.
5. **Non-goals**: no trust store, no ownership check, no opt-in walk-up in v1.
6. **Revisit triggers**: config gains any execution-adjacent field — the most
   likely accidental path is "fixing" the `$EDITOR`-with-arguments limitation
   (todo `01kwh6mqt7t5cje5j1pqysvdqa`) by adding an `editor =` config key, which
   would **invalidate this entire decision**; vault content gains any interpreted
   role; Windows support (ADR 0020 is Unix-only).

## Other verified points (no action needed)

- `--vault`/`$NTROPY_VAULT` sound (both through `require_vault`: is-vault +
  canonicalize, `resolve.rs:95-100`). ADR note: direnv-style tools can set
  `NTROPY_VAULT` from a repo's `.envrc`, but those tools gate that behind their
  own allow step; not ntropy's boundary.
- `require_vault`/canonicalize flow well designed: a found vault that fails to
  canonicalize hard-errors rather than silently falling through to the global
  default (`resolve.rs:121-127`); a broken pointer hard-errors
  (`resolve.rs:137-161`). Both prevent the "silently used a different vault"
  failure.
- TOCTOU in `is_vault`-then-use: real windows exist but exploiting them needs
  local write access to the checked dirs (attacker already owns the tree). Out
  of scope. Minor: `is_vault` uses `.is_dir()` (follows symlinks), so a
  `.ntropy` symlink counts as a vault — harmless.
- Global-default fallback: no security issue; the "notes went to my default
  vault" surprise is usability, and the broken-pointer hard error blocks the
  dangerous variant.
- Pointer parsing: only the first line is read, `~user` expansion unsupported
  (falls through to a literal path), pointers are not chained. Fine.

## Acceptance

- User confirms the "document, don't gate" direction (or chooses a gate option
  from the ranked list).
- If accepted: a new ADR is written per the sketch above, with the
  no-execution-in-config invariant as its centerpiece and the uid ownership
  check named as the pre-decided escalation. Cross-references SEC-3/SEC-5/SEC-1.
- The `editor =` config-key revisit trigger is explicitly linked from the
  `$EDITOR`-with-arguments todo (`01kwh6mqt7t5cje5j1pqysvdqa`) so a future editor
  fix cannot silently break the invariant.
