# 70. Trust model for discovered vaults

Date: 2026-10-30

## Status

Accepted

## Context

ntropy resolves its vault from `--vault`, then `$NTROPY_VAULT`, then a walk
up from the working directory, then the global default
([ADR 0016](0016-configuration-format-location-and-vault-resolution.md)). The
walk-up runs to the filesystem root without asking. It adopts the nearest
`.ntropy/` directory, or follows the nearest `.ntropy-vault` pointer to any
existing vault ([ADR 0026](0026-project-local-vault-pointer-file.md)). The
language server walks up from the directory of each document it serves
instead, so opening a file in an editor that runs `ntropy lsp` adopts that
tree's vault the same way, whatever `$NTROPY_VAULT` says
([ADR 0029](0029-language-server-over-lsp-server-with-a-session-scan-cache.md)).

A command run inside a tree the person did not write, such as a cloned
repository, a synced folder or an extracted archive, therefore uses that
tree's vault: its `config.toml`, its templates and themes, and its notes.

git guards the same situation with `safe.directory`, added after
CVE-2022-24765: a `.git` directory planted above the working directory could
carry configuration such as `core.fsmonitor` that makes git run arbitrary
commands.

## Decision

Walk-up discovery stays as it is: no confirmation, no ownership check, no
trust store and no opt-in. A vault found by walk-up is trusted as far as the
contents of the working directory are, and ntropy treats everything in it as
data.

### Nothing in a vault is ever executed

The invariant means exactly two rules, and they bind every feature:

- No value in a vault, in its `config.toml` or in the global config selects
  an executable, a shell command, a hook, a pager, or an argument of a
  process that ntropy starts. Where the vault is and which note is worked on
  still shape the paths ntropy passes: the note path the editor opens, the
  vault root and output path that `typst` gets, and, for a note of an
  encrypted vault, `$NTROPY_VAULT_HINT` with the vault root in the editor's
  environment.
- Vault content that is code runs only inside an interpreter that cannot
  start a process.

Scripts that an `html` or site export carries are not run by ntropy. They run
in the browser that opens the export, as the accepted residuals state.

ntropy starts two programs, and no vault chooses either of them unless
`$PATH` holds an empty or relative entry (see the accepted residuals):

- The editor: `$VISUAL`, then `$EDITOR`, read from the environment only, with
  no config key and no built-in default. It runs through `/bin/sh` with the
  note path as a positional parameter
  ([ADR 0066](0066-editor-value-run-through-the-shell.md)).
- `typst`, for the `pdf` format
  ([ADR 0040](0040-custom-typst-engine-with-own-markdown-emitter.md)). The
  program name is fixed in the engine and looked up on `$PATH`. The arguments
  are `compile`, `--root` with the vault root, `-`, and the output path. The
  document arrives on stdin.

Vault content that is code reaches two interpreters:

- A Typst theme ([ADR 0045](0045-vault-render-themes.md)) becomes part of
  the document `typst` compiles. Typst's language has no way to start a
  process: its data-loading functions read files, and `plugin()` runs
  WebAssembly isolated from the system. `--root` keeps the paths a document
  names inside the vault, and a path that climbs out fails. typst follows a
  symlink inside the vault, and package imports read typst's own package
  directories.
- A site theme's templates
  ([ADR 0058](0058-theme-templates-overriding-the-built-in-ones-by-name.md))
  are rendered by the embedded minijinja engine. ntropy registers its
  templates and an HTML-escaping formatter with it, and adds no loader,
  function or filter of its own.

Note templates substitute four placeholders in one pass and evaluate nothing
([ADR 0017](0017-note-templates-with-placeholder-substitution.md)).

A note's local link and image paths resolve inside the vault, and `render`
and `site` write only inside their output. View names are validated before
they become paths, and a view directory behind a symlink is skipped
([ADR 0064](0064-view-names-are-validated-slug-segments.md)).

A feature that breaks the invariant supersedes this ADR and first adds a
trust boundary. The boundary is decided: a check that the discovered
`.ntropy/` belongs to the user running ntropy.

### Accepted residuals

From the vault:

- A committed `.ntropy-vault` sends ntropy to any existing vault. Commands
  then use that vault's own configuration. `ntropy info` names the pointer
  file it followed. `delete` needs a selector, and without `--force` it asks
  for confirmation when interactive and refuses otherwise.
- A vault planted in a world-writable ancestor of the working directory is
  adopted, since the walk-up runs to the filesystem root. Notes ntropy
  writes there land in a directory its planter controls.
- Run interactively and without `--print`, `new`, `today` and `search` open
  a note in the editor, a single match without the picker. That note or its
  template may come from the vault's author. Execution features of the
  editor, such as vim modelines, are outside ntropy.
- A note's frontmatter has no size limit, so a vault can make ntropy spend
  unbounded time and memory parsing it. This is a local denial of service,
  and nothing executes.
- typst and the `html` and site exports' file copies follow a symlink inside
  the vault, so a vault can make a render or an export read a file outside
  it.
- A Typst theme's `@preview` import makes `typst` download the package over
  the network.
- Raw HTML in a note passes into `html` and site output, and a site theme's
  templates and files may add scripts. Those scripts run in the browser of
  whoever opens the exported pages.

From the person's own environment, not from the vault:

- An editor value that ends in `;`, `&`, `|` or a newline makes the shell run
  the note path as a command (ADR 0066).
- An empty or relative `$PATH` entry is resolved against the working
  directory, so a file named like the editor's program or `typst` in an
  untrusted working tree can run for the commands that open the editor and
  for `render --to pdf`.

### Rejected alternatives

- **An ownership check** on the discovered vault: it catches a `.ntropy/`
  owned by another user in a world-writable ancestor, and archives extracted
  as root, but not a repository the user cloned or a folder they synced. It
  stays the escalation named above.
- **Restricting pointer targets**: it takes away the external vault ADR 0026
  exists for.
- **A trust store**, and **walk-up only after an opt-in**: each protects only
  against low-severity misdirection and removes the walk-up that works
  without flags (ADR 0016).

### Revisit when

- A configuration key comes close to execution, such as an `editor` key, a
  pager or a hook.
- A feature lets vault content start a process, or name a file path outside
  the vault that ntropy or typst reads or writes, other than through a
  symlink inside the vault or a Typst package import.
- Windows support arrives
  ([ADR 0020](0020-unix-only-v1-with-soft-performance-target.md) is
  Unix-only).
