# Codebase analysis for the web export

What ntropy has today that a static-site export builds on, must respect,
or must add. Validated against the working tree at commit `7274544`
(v1.12.0) on 2026-09-10. Every section names its sources; line numbers
refer to that commit.

The reading is organized by the questions the feature raises: where the
data comes from, how a note becomes an artifact, how navigation is
modelled, what constrains a whole-vault operation, and what is absent.

## 1. Data: the note set and how it is read

**Scanning is stateless.** `scan::scan_notes_dir(all_notes_dir, cipher)`
(`src/scan.rs`) walks the top level of `all-notes/` once, parses every
note, and returns `Scan { notes, warnings }` with notes sorted newest
first by ULID and one `ScanWarning { path, message }` per skipped file.
There is no index and no cache in the library; every operation rescans
(ADR 0002). Decryption happens inside the scan through the `NoteCipher`,
so callers never see ciphertext and the encrypted/plaintext split is
invisible above the scan (`src/scan.rs:175-179`).

**The `Note` type** (`src/note/mod.rs:41-70`) carries everything an
export needs per note: `id` (ULID, from the filename), `slug`, `title`,
`tags` (normalized `a/b` segments), `frontmatter` (the full
`serde_yaml_ng::Mapping`, arbitrary fields preserved), `body` (Markdown
after the frontmatter, held in memory), `path`, `modified` (filesystem
mtime, soft), and `storage` (plaintext or encrypted). Creation time is
derived from the ULID (`created_ms()`, `created_date()` rendered in the
system-local timezone, ADR 0010).

**Resources.** Anything in `all-notes/` that is not a top-level note
file is ignored silently: other files and all subdirectories
(`docs/design/vault-layout-and-views.md:42-46`). There is no dedicated
assets directory in the layout (`src/vault/layout.rs`); ADR 0045 lets a
theme reach files anywhere in the vault with a root-absolute path such
as `/assets/logo.svg`, and a note body reaches `../assets/logo.svg`.
Nothing enumerates which assets a note references except the typst
emitter's image handling, which resolves paths textually and never
touches the filesystem (`src/render/typst/emitter.rs:1007-1029`).

**`VaultSession`** (`src/session.rs`) is "a vault plus the cipher to
read it". It holds no notes. The only in-memory scan cache in the repo
is the language server's, and it lives in the binary
(`src/bin/ntropy/run/lsp/cache.rs`), holds a projection (id, title,
tags, path, link target) rather than `Note`, and refreshes by full
rescan on watched-file changes (`docs/design/language-server.md:31-46`).
No library-level "vault snapshot" abstraction exists.

## 2. Navigation axes as they exist today

The vault offers four ways to reach a note, all of which R2 asks the
site to mirror:

1. **The chronological list.** `ops::search(session, None)` returns all
   notes newest-first; `search`/`list` prints them.
2. **Tags.** `tags: [a/b, c]` in frontmatter; `/` denotes hierarchy by
   convention (ADR 0006). `ops::list_tags` returns distinct tags with
   counts. The `tag:` query predicate is a contiguous-segment sub-path
   match, so `tag:programming` also matches `area/programming/cli`
   (`docs/design/query-and-search.md:43-49`).
3. **Views.** A view is `ViewDef { name, field }` (`src/view/mod.rs:22-35`):
   a directory name plus a frontmatter field to group by. Configured as
   `[[view]]` tables in `<vault>/.ntropy/config.toml`
   (`src/config/per_vault.rs:22-39`). Grouping semantics
   (`src/view/materialize.rs:167-186`): list-valued fields place the note
   under each value, `/` in a value nests directories, values are
   normalized like tags, notes without the field are omitted. Leaves are
   named `<date>-<slug>.md` with a ULID-tail disambiguator on collision
   (`src/view/leaf.rs`). There is no per-view filter; that is an open
   todo (`todos/01kvxys57hdztv654z1xx5dge3-per-view-filtering.md`).
   **The grouping computation is not separable from materialization
   through the public API**: `sync_view` both computes and writes
   symlinks; the pure `desired_links(view_dir, view, notes)` that
   computes the tree is private (`src/view/materialize.rs:92`). The
   export needs either that function exposed or its own grouping.
4. **Full-text search and the query DSL.** `query::parse` and
   `query::compile` (`src/query/mod.rs:26-33`) yield an AST
   (`And/Or/Not/Tag/Field/Text`) and a `Prepared` matcher whose
   `matches(&Note) -> bool` is a pure function over an in-memory note
   with no I/O (`src/query/eval.rs:56-65`). `text:` is a `regex` crate
   regex with multi-line and smart-case (`src/query/text_search.rs`).
   The module depends on `regex`, `regex-syntax`, `serde_yaml_ng`,
   `thiserror` and the crate's own `note` and `text` modules. Nothing in
   it is platform-specific; whether it builds for `wasm32` has not been
   tried.

Views are disabled in encrypted vaults because a symlink tree spells out
the tag taxonomy in plaintext inside the synced directory
(`docs/design/encryption.md:236-246`, `todos/01ky7r9ean3b4bxs6fehpcg4ny-encrypted-vault-views.md`).

## 3. Rendering: what exists and how much transfers

### The engine-agnostic layer (reusable as-is)

`src/render/mod.rs` defines the seam that ADR 0038 built for exactly
"another engine":

- `PreparedDocument` (`mod.rs:120-142`): `id`, `path`, `vault_root`,
  `title`, `tags`, `created`, `frontmatter` (full mapping), `body`
  (verbatim), `links: Vec<ResolvedLink>`. `ResolvedLink` carries the
  byte `range` in the body, the `display` text, the target `id`, and
  `Option<LinkTarget { title, slug }>` (`None` = dangling).
  `render::prepare(note, index, vault_root)` builds it
  (`src/render/prepare.rs:35-64`). The documented guardrail: a field
  must be a fact resolvable without knowing the output format
  (`docs/design/rendering.md:107-114`).
- `Renderer::render(&self, doc, ctx)` and `RenderContext` with
  `stage_file`, `run`, `write_output`, `warn`, `output_path`
  (`mod.rs:52-83`). **The context assumes exactly one output path per
  render call**; a site produces a tree of files, so either the trait
  grows or the site export is a different operation from `render`.
- `Registry::new(options, theme)` registers formats by string key with
  a default engine each; today `pdf` and `typst`, both served by the
  `typst` engine (`src/render/registry.rs:61-78`). Adding a format is
  one `register` call plus a `Renderer` impl. The registry's own tests
  already use a hypothetical `html` format name to prove name isolation
  (`registry.rs:184`); that is test scaffolding, not a feature.
- `RenderOptions { paper, theme }` is the `[render]` config section
  (`src/render/options.rs:61-77`). `Paper` names are Typst's paper
  identifiers. `theme` names `<vault>/.ntropy/themes/<name>.typ`
  (`src/render/theme.rs`); `theme::select(flag, configured)` and
  `theme::validate_name` (single path component, no dot prefix) are
  engine-neutral, `theme::load` reads a `.typ` file.
- `src/link/`: `extract(body)` finds `[display](<ulid>[-slug].md)` links
  outside code, `index(notes)` builds a `HashMap<Id, &Note>`,
  `resolve`, `at_offset`, `rewrite_body`. Pure text work, no Typst
  coupling.

### The typst engine (pattern transfers, code does not)

`src/render/typst/` (`emitter.rs` 2063 lines, `document.rs`,
`engine.rs`, `writer.rs`, `value.rs`, `prelude.typ`) is the only
Markdown converter in the repo. Its structure:

- pulldown-cmark 0.13 with `default-features = false` (`Cargo.toml:44`),
  options `ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_TASKLISTS |
  ENABLE_FOOTNOTES | ENABLE_GFM`, math off (`emitter.rs:113-117`), driven
  through `into_offset_iter()` so every event carries its byte span.
- An `Emitter` struct with a `Vec<Frame>` stack, one frame per open
  container, each frame owning a `TypstWriter` body
  (`emitter.rs:137-186, 240-269`). `start`/`end` push and pop frames
  and, on `End`, format the finished frame as Typst syntax inline
  (`emitter.rs:685-689` for headings, `1043-1051` for links).
- Structure-level mechanisms that are independent of Typst: note-link
  classification by matching a link event's span against the resolved
  link table (`emitter.rs:649-662`), bare-URL detection with `linkify`
  per text event (`emitter.rs:1100-1123`), footnote two-pass buffering
  with nonce placeholders (`emitter.rs:317-357`), image alt flattening,
  loose-list detection, callout kind extraction (`emitter.rs:852-877`).
- **No output abstraction.** Every closing handler string-formats Typst;
  `TypstWriter` is a concrete struct with the three escaping channels
  (`markup_text`, `string_literal`, `raw`) plus `syntax`
  (`writer.rs:59-61`). An HTML emitter cannot plug in without either a
  parallel emitter copying the skeleton or a refactor introducing an
  output trait, which does not exist.
- **Headings get no ids or anchors** anywhere (`emitter.rs:685-689`,
  `prelude.typ`). In-page and cross-page anchors are new work.
- **Note links target `<slug>.pdf` unconditionally**
  (`NOTE_LINK_EXTENSION`, `emitter.rs:80`; ADR 0044), the name a default
  `render` gives the target. The convention "artifacts in one directory
  reference each other by default name" is what a site must re-derive
  for its own URL scheme.
- **Raw HTML is dropped with a warning**; remote images degrade to a
  link plus a warning; math renders literally (`docs/design/typst-engine.md:152-178`).
  Each of these has a natural HTML rendering the site would want to
  decide on separately.
- Asset paths are rewritten to vault-root-absolute (`/all-notes/x.png`)
  because the typst compile root is the vault (ADR 0045).

The theme mechanism is Typst through and through: an embedded prelude
(`prelude.typ` via `include_str!`, `prelude.rs:23`), the theme's source
spliced verbatim after it, then `#show: note.with(title:, frontmatter:,
paper:)` as the single content/presentation seam
(`document.rs:39-73`). Four overridable functions form the theme API:
`note`, `callout`, `notelink`, `task` (`docs/design/typst-engine.md:329-335`).
The transferable idea is the shape: embedded default look, a vault file
that overrides selectively, one typed seam carrying title and frontmatter.

### What is entirely absent

- Any HTML production: no `pulldown_cmark::html` use, no HTML emitter,
  no templates, no CSS, no JavaScript anywhere in `src/` (grep for
  `html` in `src/` hits only tests of the drop-HTML behavior and the
  registry name tests).
- Backlinks: `grep -rn backlink src` returns nothing. ADR 0028 states
  the intent (compute on demand by scanning bodies), two todos plan an
  LSP and a CLI surface for it
  (`todos/01kvzkk1bvqnhfx3v6w7w80ytd-lsp-backlinks-references.md`,
  `todos/01kw4bdqkqfn8ep41cet8b1avz-cli-link-helpers.md`).
- Structured (JSON) output of notes: planned, not built
  (`todos/01kw9z4fjqnvjstfem8a6q1maa-json-output-mode.md`).
- Multi-note rendering: `render` is one note per invocation by decision
  (ADR 0037; `docs/design/rendering.md:265-273` lists it as deferred).
- Any web-related dependency in `Cargo.toml`.

## 4. Binary-side plumbing a new command would reuse

`src/bin/ntropy/cli.rs` is a clap derive tree with global flags
`--vault`, `-n`, `--strict`, `-i`, `--passphrase-file`. Interactivity is
decided once per invocation from `-n` and the presence of `/dev/tty`
(`src/bin/ntropy/run/interact.rs:20-22`, ADR 0036) and threaded as a
bool. Scan warnings fold into the exit code under `--strict` through
`exit_for_warnings` (`src/bin/ntropy/run/mod.rs:786-792`).

`cmd_render` (`src/bin/ntropy/run/render.rs:42-220`) is the closest
template: load `PerVaultConfig`, select and load the theme before any
scan, build the registry, resolve the selector (a full ULID directly,
anything else as a DSL query; ambiguity opens the picker or errors under
`-n`), default the output path, `prepare`, absolutize the output path,
warn when the output lands inside an encrypted vault, run the engine
through `ProcessContext` (temp staging under the runtime dir, `PATH`
resolution anchored to the parent cwd, subprocess with piped stdin),
report.

ADR 0013 fixes the boundary: domain logic in the library with
`thiserror` errors and no ambient I/O; the binary contributes effects
through injected capabilities. A site generator's "compute the file
tree" belongs in the library; writing it to disk is the binary's job or
a context's.

## 5. Constraints that apply to a whole-vault operation

- **Encrypted vaults.** Every note is decrypted in memory per command
  (`docs/design/encryption.md:182-188`). An exported site is plaintext by
  nature, like a rendered PDF; `render` warns when the artifact lands
  inside an encrypted vault (`encryption.md:220-233`). An exported site's
  directory structure and file names expose the same taxonomy the
  disabled views would. Assets are not encrypted at all today
  (`todos/01kz9zn4gkf9xct143e1w5ysej-encrypted-vault-assets.md`).
- **Reserved names.** `all-notes`, `.ntropy`, `.gitignore`, `README.md`
  and every configured view name are taken inside a vault
  (`src/vault/layout.rs:51`); an output directory defaulting to inside
  the vault would have to avoid them and would be picked up by git
  unless `.gitignore` is managed (ADR 0032 manages entries for views).
- **Distribution via crates.io** (ADR 0022): the crate is built from the
  packaged tarball with `cargo`. Seed content is embedded with
  `include_str!` from real files under `src/vault/seed/` (ADR 0039),
  which is the established pattern for shipping non-Rust files in the
  binary. Nothing in the build invokes a JavaScript toolchain; the
  project's own landing page under `docs/pages/` is built by an external
  Bun-based generator and is unrelated to vault content
  (`README.md:927-955`).
- **Unix-only v1** (ADR 0020); symlink views are the main Windows
  blocker. A static site has no such dependency.
- **Testing** (ADR 0021): three insta layers. `tests/views.rs` snapshots
  a generated symlink tree from a temp vault and is the precedent for
  testing a generated file tree; `tests/cli.rs` contract tests run the
  binary with `-n`/`--print` and redact vault paths, ULIDs, and dates;
  no test executes an external tool except the `#[ignore]`d
  `just verify-render`. The kitchen-sink fixture
  (`tests/fixtures/kitchen-sink.md`) exercises every supported Markdown
  construct and pins the full typst document as one snapshot.

## 6. Dependencies relevant to reuse in a desktop app or browser

From `Cargo.toml` (v1.12.0): `pulldown-cmark` (no default features),
`linkify`, `regex`, `regex-syntax`, `serde`, `serde_json`,
`serde_yaml_ng`, `ulid`, `jiff`, `thiserror`, `anyhow`, `clap`,
`crossterm`, `nucleo`, `ignore`, `directories`, `tempfile`, `toml`,
`unicode-width`, `lsp-server`, `lsp-types`; `age` and the keyring stores
behind the default `encryption` feature; `libc` under `cfg(unix)`. No
explicit `[lib]`/`[[bin]]` sections: library `src/lib.rs`, binary
`src/bin/ntropy/main.rs`.

The pure conversion path (`emitter::emit` plus `document::assemble`)
is string in, string plus warnings out, with no I/O; only the `pdf`
format's delivery spawns a process, and only through `RenderContext`.
`tempfile` is used in the library by `scan.rs`, `fsutil.rs`,
`template.rs`, `cipher.rs`, `gitignore.rs`, `session.rs` (filesystem
work) and in the binary's render context. A desktop application
embedding the library natively is unaffected by any of this; a browser
(WebAssembly) build of the query or conversion modules has not been
attempted and the crate has no feature gating aimed at it.
