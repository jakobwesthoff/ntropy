Part of the ntropy skill. Read SKILL.md first, which holds the rules for every task; `ntropy skill` prints it.

# Vaults

A vault is an ordinary directory. There is no database and no index: the
Markdown files inside it are the single source of truth, and every command
re-reads them fresh.

## Anatomy

`ntropy init <path>` scaffolds this structure (idempotent — it creates what is
missing and never touches what exists). Name the target with the positional
path; passing both a path and `--vault` to `init` is rejected as a conflict:

```
<vault>/
├── all-notes/            # canonical notes, <ulid>-<slug>.md — the source of truth
├── by-tag/               # seeded materialized view (symlinks, derived, disposable)
├── README.md             # seeded: identifies the vault, links the ntropy docs
├── .gitignore            # auto-managed: ignores the view directories
└── .ntropy/              # the only reserved directory
    ├── config.toml       # per-vault view definitions
    └── templates/        # default.md and today.md, plus your own
```

Only top-level `*.md` files in `all-notes/` are notes, extension in lowercase.
Subdirectories and all other files there, `.MD` files included, are ignored,
so images and attachments can live next to the notes.

## How the active vault is resolved

Every command operates on exactly one vault, resolved in this order:

1. `--vault <path>` (global flag on every command)
2. `$NTROPY_VAULT`
3. Walk up from the current directory to the nearest ancestor holding a
   `.ntropy-vault` pointer file or a `.ntropy/` directory. Nearest wins; a
   pointer beats a `.ntropy/` in the same directory.
4. The global default vault (recorded by `ntropy init --set-default`).

Check what resolved, and why, with `ntropy info`. The first line names the
active vault and the rule that selected it:

```
Active vault:  /Users/you/project/notes (via pointer file /Users/you/project/.ntropy-vault)
Default vault: /Users/you/notes
```

Run `ntropy info` whenever it is unclear which vault a command will hit;
creating notes into the wrong vault is the failure it prevents. When no vault
resolves, commands fail with `no vault found; pass --vault, set $NTROPY_VAULT,
run inside a vault, or set a default`. That is a question for the user, not a
reason to run `ntropy init` or `init --set-default` yourself (the core's rule on
choosing a vault).

When a script needs the path rather than the report, `ntropy info --print`
prints the active vault's absolute path alone and exits non-zero if none
resolves:

```bash
vault=$(ntropy info --print) || exit 1
```

## Creating vaults

### Global default vault

```bash
ntropy init ~/notes --set-default
```

`--set-default` records the vault as the global fallback in the OS config file
(`~/.config/ntropy/config.toml` on Linux,
`~/Library/Application Support/ntropy/config.toml` on macOS), a single
`default_vault = "..."` line. Without the flag, `init` never touches global
config. `$NTROPY_CONFIG_DIR`, when set and not empty, replaces that directory:
the file is then `$NTROPY_CONFIG_DIR/config.toml`, for reading and writing.

### Project-local vault

Keep the vault in a subdirectory and drop a pointer file at the project root so
every command run anywhere inside the project resolves to it:

```bash
ntropy init myproject/notes
echo "notes" > myproject/.ntropy-vault
```

The pointer file is a single line: a path relative to the pointer file's own
directory, an absolute path, or a `~` path. A broken pointer is a hard error,
never a silent fall-through to the default.

Initializing the project root itself as the vault also works (`ntropy init .`),
but then `all-notes/` and the view directories sit next to the project's own
files. Prefer the subdirectory-plus-pointer layout.

### Custom / external vault

For a vault that no pointer or default reaches, pass it explicitly:

```bash
ntropy --vault /path/to/vault search -n tag:work    # one command
export NTROPY_VAULT=/path/to/vault                  # a whole session
```

A pointer file can also target an external vault (`echo "~/team-vault" >
.ntropy-vault`), sharing one vault across several projects.

## Encrypted vaults

`ntropy init <path> --encrypted` stores the vault's notes encrypted at rest.
The vault gains two files and loses its views:

```
.ntropy/identity.pub    the public recipient; its presence marks the vault encrypted
.ntropy/identity.age    the identity, wrapped under a passphrase
all-notes/<ulid>.age    notes, with no slug in the filename
```

There is no `by-tag/` and no `.gitignore`, because materialized views are
disabled: a symlink tree would spell out the tag taxonomy in plaintext names
inside the synced directory. Both key files belong in version control — they
*are* the vault, and the identity is useless without the passphrase.

What differs for an agent:

- **A `-p` path is ciphertext.** Never read it. `ntropy search -n -P <ulid>`
  prints a note's text and reads the same in either kind of vault.
- **Reading needs the key**: `search`, `show`, `render`, `site` and `today`
  fail on a locked vault with a message naming `ntropy unlock`, which puts the
  key in the OS credential store; `ntropy lock` removes it.
- **Writing does not**: `ntropy new` and `ntropy write <id>` work on a locked
  vault. `write` is also the only way to author a note there, since the `-p`
  path is an age container and text written into it is not a note.
- **Headless**, `--passphrase-file <file>` is enough: the first line of the
  file is the vault's passphrase, and it unwraps `.ntropy/identity.age`.
  `--identity <file>` (or `$NTROPY_IDENTITY`) is the alternative and takes an
  unwrapped age secret key (an `AGE-SECRET-KEY-` line, as `age-keygen`
  writes), never `.ntropy/identity.age`; the passphrase is then not used.
  With `-n` ntropy never prompts.
- **Views do not exist** there, so `ntropy view add` is refused.
- **Rendered and exported output is plaintext.** Write it outside the vault
  ([rendering.md](rendering.md), or `ntropy skill rendering`).

Converting an existing vault is `ntropy vault encrypt` (and `vault decrypt`
back). Both rewrite every note, ask first unless given `-y`, and are safe to
interrupt: `--resume` finishes one that was. They refuse to start while two
note files share a ULID, a file sits where a converted note would go or a
file is in `all-notes/_assets/` (asset encryption is not supported yet); run
`ntropy reconcile` first, or move the named files away.

`ntropy vault passphrase` changes only the passphrase that wraps the
identity: the current one comes from `--passphrase-file`, the new one from
`--new-passphrase-file <file>`. `ntropy vault rekey` re-encrypts every note
to a fresh key; the vault keeps its passphrase unless `--new-passphrase-file`
gives a new one, and `--resume` finishes an interrupted run. With `-n` and
no new-passphrase file a command that needs a new passphrase fails instead
of prompting.

## Configuration files

- **Global** `config.toml`: only `default_vault`. Written by
  `init --set-default`; rarely edited by hand.
- **Per-vault** `<vault>/.ntropy/config.toml`: the view definitions
  (`[[view]]` tables with `name` and `field`), so views travel with the vault.
  Manage it through `ntropy view add|remove|list`, not by hand — the CLI also
  keeps the vault's `.gitignore` and the materialized trees in sync, which a
  hand edit does not (a hand edit needs a follow-up `ntropy reconcile`).

Templates are plain files under `<vault>/.ntropy/templates/`, not config. See
[writing-notes.md](writing-notes.md), or `ntropy skill writing-notes`.

## Reserved names

Inside a vault, `all-notes`, `.ntropy`, `.gitignore`, and `README.md` are
reserved, as is the directory of every configured view, and a view name's first
segment may not be one of them
([views.md](views.md), or `ntropy skill views`). `.ntropy-vault` is reserved as
the pointer-file name. ntropy owns these names: create no file or directory
under them yourself.

## Vaults under git

A vault is plain files, so version it directly: `git init` inside it and commit
notes like any text. ntropy maintains the vault's root `.gitignore` so the
derived view directories stay untracked; it marks its own entries with a
comment and never touches lines you add yourself.

## Strictness and malformed notes

Malformed notes (no `title`, bad filename) are skipped with a stderr warning by
default. Pass `--strict` to turn those warnings into hard errors — useful as a
vault health check. `reconcile --strict` also fails on a configured view
skipped for an invalid name ([views.md](views.md), or `ntropy skill views`):

```bash
ntropy search -n --strict > /dev/null   # exits non-zero if any note is malformed
```

Under `--strict` the matching notes still print and only the exit code and the
`warning:` lines on stderr change. An empty vault also exits non-zero, since a
no-match listing does: read stderr, not only the exit code
([querying.md](querying.md), or `ntropy skill querying`).

End of references/vaults.md.
