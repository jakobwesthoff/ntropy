---
title: "Decide whether walk-up vault discovery needs a trust boundary"
kind: investigation
component: vault
status: needs-discussion
impact: low
origin: review
tags: [security]
---
# Decide whether walk-up vault discovery needs a trust boundary

Found in the 2026-07-02 security review of vault discovery. ntropy finds its vault by walking up from the working directory. A command run inside an untrusted tree (a cloned repository, a synced folder, an extracted archive) silently uses that tree's vault: its `config.toml` view definitions, its templates and its notes. Nothing asks for confirmation, and there is no allowlist or opt-in, unlike git's `safe.directory`, which was added after CVE-2022-24765. The open question is whether ntropy should gate this or document the trust model and keep the default.

## Recommendation

The security triage ran after the code review on 2026-07-02. It verified the execution surface and recommends against building a trust boundary. Instead, the primitives get hardened (the view-name, terminal-escape and frontmatter-parser fixes, each in its own todo, see Relations), and the threat model goes into a new ADR with a binding "no execution in config" invariant. This is a proposal for the user to accept. The ADR must not be written as a decided record until the user confirms the "document, don't gate" direction.

## Context

### Discovery order

`resolve_with_source` in `src/vault/resolve.rs` (around lines 66-92) checks, in order: `--vault`, `$NTROPY_VAULT`, a walk-up from the working directory (`walk_up` and `walk_up_with`, around lines 104-131), and the global `default_vault`. At each directory of the walk-up, a `.ntropy-vault` pointer wins over a `.ntropy/` directory in the same directory, and the nearest ancestor wins. The walk-up asks for no confirmation and runs up to `/` (around line 115).

### The pointer file

`resolve_pointer` (around lines 137-161) reads the first line of `.ntropy-vault` as a path. The path may be absolute, relative to the pointer's directory, or `~`-expanded (`expand_tilde`, around lines 164-175). The only constraint is that the target must already be a vault. A `.ntropy-vault` committed into a cloned repository can therefore redirect ntropy to any existing vault. Once redirected, everything (config, templates and notes) comes from the target vault, and the hostile tree contributes only the redirect. A redirect to the user's own vault yields only ordinary operations on that vault, and after the view-name fix those cannot escape it. This is no worse than plain `.ntropy/` adoption. The pointer restriction option below covers the alternative.

### Execution-influence surface: verified empty

The triage swept `src/` for process spawns (`Command`, `spawn`, `exec`, `.status(`, `.output(`) and environment reads (`env::var`, `var_os`, `env!`, `option_env!`).

- There is exactly one subprocess spawn in the binary: `Command::new(&editor)` in `src/bin/ntropy/run/editor.rs` (around line 20). The editor comes only from `$VISUAL`, then `$EDITOR` (around lines 35-46). There is no config fallback and no built-in default, and a missing variable is a hard error (around lines 43-45). The other hits are crossterm's `execute!` terminal macro (`src/bin/ntropy/run/picker/mod.rs`, around lines 72 and 79) and `thread::spawn` in LSP tests only.
- The complete environment-read inventory is `VISUAL` and `EDITOR` (`editor.rs`, around line 37), `NTROPY_VAULT` (`src/bin/ntropy/run/mod.rs`, around line 97), and `current_dir` (`run/mod.rs`, around lines 98 and 122). Nothing else.
- The complete per-vault config schema (`src/config/per_vault.rs`, around lines 19-31) is `views: Vec<ViewConfig { name, field }>`. `name` becomes a path component (see the view-name todo). `field` is only a frontmatter-key lookup (`src/view/materialize.rs`, around lines 167-186). The global config (`src/config/global.rs`, around lines 27-32) holds only `default_vault: Option<PathBuf>`, validated through `require_vault`. It lives in the user's OS config directory, which an attacker cannot write to. There is no pager, no hook, no shell and no "open with" anywhere.
- Templates: `render` (`src/template.rs`, around lines 115-157) substitutes `{{key}}` in one pass against a closed set of four keys (`title`, `id`, `date`, `slug`). There is no evaluation, inclusion or command mechanism. `load_named` rejects empty names and path separators (around lines 93-98).
- LSP: the server has a read-only request surface (completion, definition, documentLink, workspaceSymbol; `src/bin/ntropy/run/lsp/mod.rs`, around lines 224-232). `for_document` sets only `start_dir` and has no environment or global fallback (`lsp/vault.rs`, around lines 36-40). No `fs::write` or `create_dir_all` in the LSP tree runs outside tests.

A hostile vault cannot choose the program ntropy launches, and it cannot influence any process or code execution. All three "not influenced" claims hold.

### Adjacent soft spot

ntropy opens attacker-authored content in the user's editor. A single-match `search` opens the note at once (`src/bin/ntropy/run/mod.rs`, around lines 172-175). `new` and `today` open a note rendered from a template that may be hostile. The program is chosen by the user, but the content is chosen by the attacker. Editor-side execution features, such as vim modelines and `exrc`, are outside ntropy's control. The ADR sketch treats this as a non-goal, not a mitigation.

## Residual risk

The finding was first rated medium as an amplifier. It drops to low once the view-name, terminal-escape and frontmatter-parser fixes land. It cannot escalate to code execution, and its blast radius is data integrity, denial of service and display, which those three fixes cover. What remains:

- Write misdirection: commands act on a vault the user did not consciously choose. This stays within legitimate vault operations (note creation, `reconcile` renames, view sync, `.gitignore` sync). `delete` still needs a selector and confirmation, or `--force` (`run/mod.rs`, around lines 296-304).
- Mild self-disclosure: notes created in a planted vault land in an attacker-readable location, for example under a `/tmp` ancestor, since walk-up runs to `/`. There is no network, so there is no exfiltration channel.
- Symlinked notes: reads follow symlinks (`src/scan.rs`, around line 135), so a hostile vault could link `all-notes/x.md` to a user file. The target must parse as a note (frontmatter with a title) to appear, which excludes almost all real files. Every write path replaces the symlink rather than following it. `atomic_write` renames over the link (`src/fsutil.rs`, around lines 72-91), and `remove_file` removes the link (around lines 99-101).
- Template text: inert, attacker-chosen prose in a newly created note.
- The real residual is future config growth, covered by the binding invariant in the ADR sketch.

Standalone, this reduces to a documentation and invariant item, not a code change.

## Options

Ranked by the triage.

1. Do nothing at the trust layer. Harden the primitives and write the ADR. This is the recommendation. git's `safe.directory` exists because repository-local config selects executables (hooks, `core.pager`, `core.fsmonitor`), and CVE-2022-24765 was code execution. ntropy's config selects no executable (verified above), so the premise of the analogy does not hold. Once the primitives are fixed, every gating option protects only against low-severity misdirection, and each one costs either the default walk-up behaviour (ADR 0016) or machinery out of proportion for a single-user, local notes CLI. This option must come with the binding invariant below.
2. uid ownership check. Reserved for escalation, not v1. It catches a foreign-uid `.ntropy/` in a world-writable ancestor (`/tmp` on a shared machine) and archives extracted as root. It misses the dominant case, a repository you cloned or a folder you synced, where every file is owned by you. Since it misses the main attack and the primitives are fixed anyway, its edge cases (sudo, containers, NFS uid mapping) are not worth the cost now. Implement it if an escalation trigger fires.
3. Pointer restriction. Rejected. Disallowing absolute or `~` targets, or confining targets below the pointer's directory, removes the reason for ADR 0026 ("a vault that lives elsewhere ... external", `docs/adr/0026`, around lines 12-15) in order to close a low-severity misdirection. `ntropy info` already shows the source (`ResolveSource::Pointer`, `resolve.rs`, around lines 39-40; `run/mod.rs`, around lines 339-347). A per-command stderr notice was rejected too: it would fire on every command for users who keep external vaults on purpose.
4. Trust store, a `safe.directory` analogue. Rejected for v1. It is git-level machinery for consequences that are not git-level, and it taxes every legitimate multi-vault user.
5. Walk-up opt-in. Rejected. It removes the default mode of operation to defend a surface that hardening has already closed. It has the worst cost-benefit of the five.

### Interaction with the view-name fix

With view names validated (the `ViewName` newtype from the view-name todo), the destructive residual is effectively gone. Plain adoption confines view sync to validated directories under the adopted root, which is the attacker's own tree. A pointer redirect syncs the target vault with the target's own config, which is the user's trusted config. Hostile config never mixes with user data, so nothing left justifies a gate.

## ADR sketch

Write this only after the user accepts "document, don't gate". The ADR is titled "Trust model for discovered vaults":

1. Precedence, restated with references to ADRs 0016 and 0026 rather than a copy of them.
2. The trust assumption, stated plainly: a vault found by walk-up is trusted to the same degree as the working directory's contents. ntropy treats everything inside a vault as untrusted data, never as instructions. Display is sanitized (the terminal-escape todo), view names are validated before they become paths (the view-name todo), frontmatter parsing is bounded (the frontmatter-parser todo), and templates are inert placeholder substitution.
3. The binding invariant: no configuration value, per-vault or global, may name an executable, a shell command, a hook, a pager, or an argument passed to a spawned process. The only subprocess ntropy launches is the editor, resolved only from `$VISUAL` and `$EDITOR`. Any feature that breaks this invariant must supersede the ADR and first introduce a trust boundary (a uid check or a trust store).
4. Accepted residuals, named: committed `.ntropy-vault` misdirection (operations use the target vault's own config, and `delete` stays behind a selector and confirmation); a planted vault in a world-writable ancestor can be adopted, since walk-up runs to `/`; auto-opened content may be attacker-authored, and editor-side execution is out of scope.
5. Non-goals: no trust store, no ownership check and no opt-in walk-up in v1.
6. Revisit triggers: config gains any execution-adjacent field. The most likely accidental path is "fixing" the `$EDITOR`-with-arguments limitation by adding an `editor` config key, which would invalidate this decision (see the editor todo in Relations). Vault content gains an interpreted role. Windows support arrives (ADR 0020 is Unix-only).

## Open questions

- Does the user accept the "document, don't gate" direction, or choose a gate from the options above? The ADR waits on this answer.

## Other verified points

- `--vault` and `$NTROPY_VAULT` are sound. Both pass through `require_vault`, which checks for a vault and canonicalizes the path (around lines 95-100). direnv-style tools can set `NTROPY_VAULT` from a repository's `.envrc`, but those tools gate that behind their own allow step, so it is not ntropy's boundary.
- `require_vault` and canonicalization are well designed. A found vault that fails to canonicalize is a hard error, not a silent fall-through to the global default (around lines 121-127). A broken pointer is also a hard error (around lines 137-161). Both prevent a silent switch to a different vault.
- A time-of-check to time-of-use window in `is_vault` followed by use is real, but exploiting it needs local write access to the checked directories, and an attacker with that access already owns the tree. Out of scope. `is_vault` uses `is_dir()`, which follows symlinks, so a `.ntropy` symlink counts as a vault. That is harmless.
- The global default fallback is not a security issue. The "notes went to my default vault" surprise is a usability matter, and the broken-pointer error blocks the dangerous variant.
- Pointer parsing reads only the first line, does not expand `~user` (it falls through to a literal path), and does not chain pointers. This is fine.

## Done when

- The user confirms the "document, don't gate" direction, or chooses a gate from the options.
- If accepted, the ADR is written per the sketch, with the no-execution-in-config invariant as its centerpiece and the uid ownership check named as the pre-decided escalation. It cross-references the view-name, terminal-escape and frontmatter-parser todos.
- The `$EDITOR`-with-arguments todo is updated to point at this decision, so a future editor fix cannot silently break the invariant.

## Relations

- Relates to: [Unbounded frontmatter size lets one note slow every scan](01KWH7Y8ZFKD52TJ3AEGSSBQFQ-unbounded-frontmatter-size-lets-one-note-slow-every-scan.md), a primitive fix this recommendation relies on
- Relates to: [Filename parse panics on a multibyte character straddling byte 26](01KWH5HHWGQWMR1EGHHTMED79T-filename-parse-panics-on-a-multibyte-character-straddling-byte-26.md), the same untrusted-file trigger reached through walk-up discovery
- Relates to: [View names are used as unvalidated paths, so a config can delete files outside the vault](01KWH7Y8ZFKD52TJ3AEGSSBQFR-view-names-are-used-as-unvalidated-paths-so-a-config-can-delete-files.md), the view-name fix that removes the destructive residual
- Relates to: [EDITOR values with arguments fail to launch](01KWH6MQT7T5CJE5J1PQYSVDQA-editor-values-with-arguments-fail-to-launch.md), the likeliest route to breaking the no-execution invariant
