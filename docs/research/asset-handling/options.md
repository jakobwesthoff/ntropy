# Options

Every option either researcher considered, merged. Opus labelled its options A
to F, fable numbered its five 1 to 5. Two ideas that came up in the critiques
are added as G and H. The variants that the user's readability principle
makes relevant, and two that came up in the document review, are in
[Readable folder names](#readable-folder-names) and carry lowercase letters.

| Here | Opus | Fable | Idea |
| :--- | :--- | :--- | :--- |
| A | A | 2 | Today's basename folder, kept current by `reconcile` |
| B | B | 3 | One folder per note named by the bare ULID |
| C | C | 4 | Notes as directories (bundles) |
| D | D | variant of 5 | Content-addressed store |
| E | E | 5 | Assets as first-class ULID objects |
| F | F | 1 | Keep the convention, add checks only |
| G | no | in 5 | A custom link scheme |
| H | critique 1 | response 2 (agreed with the rejection) | Frozen basename |

## Common to every option

- Remote URLs stay as they are. A file elsewhere in the vault keeps working,
  unmanaged, under the export rules of ADR 0073.
- The PDF gets a check before the compile, so a missing or outside image is a
  warning and not a failed render. Todo
  [01M66B2JM5XQDK7M3ZVKA1NZJV](../../../todos/all-notes/01M66B2JM5XQDK7M3ZVKA1NZJV-refuse-note-images-that-leave-the-vault-through-a-symlink-in-pdf-renders.md)
  already proposes resolving each image path with the rules of ADR 0073.
- Windows is unsupported (ADR 0020), so no option is judged on it.

## A. Basename folder, kept current by `reconcile`

- Layout: `all-notes/assets/<ULID>-<slug>/<file>`, linked
  `assets/<ULID>-<slug>/<file>`, folder recognized by its ULID prefix.
- `new`: nothing, or `new --assets`. `asset add <note> <file>` copies in and
  prints the link.
- `reconcile`: on a retitle renames the folder and rewrites every link into
  it, in the note and in any other note. `realign` after `edit` and `write`
  (`src/reconcile.rs:476`) renames only the note and rewrites no link today, so
  it needs a decision of its own, see
  [Readable folder names](#readable-folder-names). Reports orphans and dangling
  asset links.
- `delete`: removes the folder, warns when other notes link into it.
- Views: unchanged, previews through a view break as today.
- LSP: completion, document links, diagnostics, all new code.
- Outputs: unchanged. Site file URLs change on every retitle, which breaks
  inbound links to published files (fable).
- Encryption: the slug cannot appear in an encrypted vault, so the folder
  there must be `<ULID>` alone. Two layouts for one concept, and `vault
  encrypt` and `vault decrypt` must rename folders, with or without rewriting
  links, see [Readable folder names](#readable-folder-names).
- Shared assets: work, at the price of link rewrites in foreign notes on every
  retitle of the owner. The `primary` vault has no shared asset today
  (measured), so that price is zero there.
- Migration: none for well-formed vaults. A folder that drifted, like the two
  of the 2026-11-02 incident, needs a repair by ULID prefix.
- Plain Markdown: renders everywhere. Between a retitle and the reconcile the
  link is still valid because the folder has not moved.
- Scripting and agents: a path copied outside the vault breaks on every
  retitle and every slug-rule change, the Muesli failure of the incident.
- Git and sync: every retitle moves a directory of binaries. Git detects the
  rename, sync tools re-upload or not depending on the tool (opus). Two
  machines that retitle and reconcile independently produce a duplicated
  folder (fable, reasoning, not tested).
- Cost: medium. Modules: a link pass in `src/link/`, a folder rename in
  `src/reconcile.rs`, `src/ops/delete.rs`, the CLI. A retitle that renames the
  folder before the links are rewritten leaves broken images until the next
  `reconcile`, which finishes the job because it identifies the folder by its
  ULID prefix, as it does for note renames. `reconcile` keeps no journal for
  that (`src/reconcile.rs:207-226`).
- Verdict, as the researchers wrote it: rejected. It integrates the convention
  and keeps its flaw. A rename automated is rarer, not impossible: a sync
  conflict on another machine leaves the old state (opus). Opus adds that
  `realign` would rename a folder of binaries under an open editor after a
  plain `edit`. The user's readability principle reopens it, see
  [Readable folder names](#readable-folder-names).

## B. One folder per note named by the bare ULID (recommended)

- Layout: `all-notes/assets/<ULID>/<file>`, linked
  `![Chart](assets/<ULID>/conv.png)`. Plain Markdown relative to `all-notes/`,
  no ntropy syntax. Ownership rule: a directory `all-notes/assets/<ULID>/`
  belongs to the note with that ULID. The owner is read from the path and
  never stored, which keeps ADR 0002 intact.
- Lab (fable, vault `optb`): this layout renders in PDF and HTML with today's
  binary, so a plaintext vault needs no renderer change for it.
- Retitle and slug-rule changes: nothing happens. The folder never renames,
  links never change, paths copied outside the vault stay valid.
- `new`: `new --attach <file>` as an optional shortcut. `asset add <note>
  <file>... [--move] [--as <name>]` copies or moves files into the note's
  folder, creates it, and prints one link per file that renders (see settled
  points). It can work on a locked encrypted vault, since encrypting needs only
  the recipient, provided the note is selected by its ULID or file name without
  a scan. Every selector today scans and fails on a locked vault
  (`src/ops/select.rs:169`, `src/scan.rs:103`), so that is a new code path.
  `asset list <note>` lists the files and marks which the body references,
  `--path` prints the folder for scripts in a plaintext vault. Opus proposed
  `asset add`, `asset list` and `new --attach`. Fable first proposed `new
  --assets` (create the folder) and `asset dir`, then agreed with opus's set
  including `new --attach` and dropped `asset dir` for `asset list --path`.
- `delete`: removes the note and its folder. If another note references a
  file in the folder, it removes the note, keeps the folder, names the
  referencing notes, and a dedicated flag removes the folder anyway.
- `reconcile`: reports dangling asset links (file missing) and orphan folders
  (ULID matches no note), fatal under `--strict`. Never deletes an orphan.
  Reports legacy folders and names the conversion command. Removing empty
  folders is fine by default, git does not track them (fable).
- Search: an optional predicate such as `has:assets`. Not needed first.
- Views: unchanged. Previews through a view may break, as today.
- LSP: completion after `assets/<ULID>/`, document links, and diagnostics for
  missing files. The server has no diagnostics capability today, so
  diagnostics are a new capability and not an extension.
- Outputs: PDF as today for images, with the missing-image rule above. HTML
  and site unchanged, the public file URL becomes
  `files/all-notes/assets/<ULID>/x.png`, stable across retitles and shorter
  than today's. ANSI unchanged.
- Encryption: phase 2, below.
- Shared assets: three ways, none needing a new concept. A second note links
  `assets/<owner ULID>/x.png`, and `delete` protects the owner. Free folders
  such as vault-root `assets/<topic>/` stay unmanaged and unencrypted, as in
  the docs vault. A note that exists to own shared files gives them a
  lifecycle and encryption.
- Migration of `primary`: a one-shot command renames each
  `assets/<ULID>-<slug>/` to `assets/<ULID>/` and replaces the old folder name
  in note bodies. The flat `all-notes/images/<ULID>-<name>.png` files of
  `primary` fit no folder: they move into folders and their links are
  rewritten, or `images/` is left unmanaged. See settled points.
- Plain Markdown: GitHub, VS Code preview and Obsidian resolve relative image
  paths against the file (from knowledge, not lab tested, opus).
- Scripting and agents: the path is computable from the ULID alone,
  `<vault>/all-notes/assets/<ULID>/`. With `new --id` (ADR 0090) a script
  never needs the slug. Today's convention needs the current basename.
- Git and sync: no renames ever, one stable pattern `all-notes/assets/**` for
  `.gitattributes` and LFS. Encrypted assets do not delta.
- Duplicate ULIDs (ADR 0067): the copy gets a new ULID, the folder stays with
  the kept note, the copy's links still point at it, and the cross-note check
  reports that (fable).
- Case-insensitive filesystems: the folder uses the note's uppercase ULID.
- Cost, phase 1: medium. A new `src/asset/` module (layout, recognition,
  folder resolution, orphan detection), a pass for references (a
  `pulldown-cmark` pass with the renderer's options),
  `src/reconcile.rs` (checks, legacy report), `src/ops/delete.rs`, an `asset`
  subcommand in `src/bin/ntropy/cli.rs` and `run/`, LSP navigation and
  completion, the PDF check in the typst engine with `ExportScope`, a
  new ADR, updates to `docs/design/vault-layout-and-views.md`,
  `rendering.md`, `typst-engine.md`, `cli.md`, `language-server.md`, the skill
  (`skills/ntropy/references/writing-notes.md` "Attachments", `vaults.md`)
  and the docs website. Cost, phase 2: large, see below.
- Downside, in both researchers' words: `assets/<ULID>/` says nothing about
  the note to a person browsing the folder.
- Pros: removes the failure and does not automate a repair. Same layout in
  plaintext and encrypted vaults. Ownership from the path, so `delete` knows
  the note's files and the orphan check runs without reading a note body; the
  orphan check also works on a locked vault. Protecting files other notes
  reference needs a body scan, as under E. Plain Markdown, the property ADR
  0028 paid for. No link rewriting after migration.

### Variants of B by location

Every subdirectory of `all-notes/` is reserved for resources by ADR 0019, so
both `all-notes` locations are covered by the scanner. The table is written
for the bare ULID. The location question for the readable names is in
[Location per name](#location-per-name).

| Location | Link | For | Against |
| :--- | :--- | :--- | :--- |
| `all-notes/assets/<ULID>/` | `assets/<ULID>/x` | No `..`. Already the parent in `primary`. One reserved name for the managed area, one pattern for git rules. Migration stays inside one directory. `ls all-notes/` keeps showing notes. Separates managed per-note files from the unmanaged shared files in vault-root `assets/`. | The ULID folder name is unreadable. |
| `all-notes/<ULID>/` | `<ULID>/x` | Sorts directly before its note in a name-sorted listing, because the bare ULID is a prefix of `<ULID>-<slug>.md`. Shortest link. In an encrypted vault `<ULID>/` sits next to `<ULID>.age`. `parse_name` does not block it. | `all-notes/` gains one directory per note with attachments. No single parent for git rules. Differs from where `primary` has its folders, so migration changes the link prefix and not one path component. |
| Vault-root `assets/<ULID>/` | `../assets/<ULID>/x` | Keeps `all-notes/` notes only. | Links need `../`. `all-notes/` is the one special directory and every other top-level directory is a projection of the notes (`docs/design/vault-layout-and-views.md:32-36`). The name is taken by the docs vault's shared files. Encryption would walk a second top-level tree. |

Both researchers prefer the first. Opus asked that the user see the second
before deciding.

### B, phase 2: encryption

The folder name already carries nothing of the title, so only file names and
contents remain to protect.

- Cipher seam: needs a bytes or streaming variant next to
  `read`/`write(String)` (`src/cipher.rs:71-74`). A screenshot must not be
  buffered as a `String`.
- Scheme, if file names are secret (opus, adopted by fable): each file becomes
  `all-notes/assets/<ULID>/<asset id>.age`. It is a standard age file whose
  plaintext is a one-entry tar holding the real name and contents. `age -d |
  tar x` recovers it. Links in the body keep the logical name
  (`assets/<ULID>/shot.png`) in both storage forms, so `vault encrypt` and
  `decrypt` never rewrite note bodies. ntropy maps names to blobs by reading
  each blob's tar header. With a folder named `<ULID>-<slug>`, the same holds
  when links keep the plaintext folder name and the encrypted folder is found by
  ULID prefix, see [Readable folder names](#readable-folder-names).
- Scheme, if file names may leak (fable): each file becomes `<name>.age`, a
  link resolves with a path join, and the resolvers try `<path>.age` when the
  vault is encrypted. The folder stays usable by hand.
- Header reads are cheap. The `age` crate in the dependency tree is 0.12.1,
  it encrypts in `CHUNK_SIZE = 64 * 1024` chunks and its `StreamReader`
  implements `Read` and `Seek`
  (`age-0.12.1/src/primitives/stream.rs:22`, `:483`, `:610`). Reading the
  first 512 bytes of the tar decrypts one chunk. This turned opus's
  speculation into a fact.
- Asset id: a ULID, not random. This argument depends on `asset add` working
  on a locked vault, which needs the selector path without a scan named above.
  On a locked vault `asset add` cannot see an existing blob with the same
  logical name, so duplicates will happen, and
  only an ordered id gives the rule "newest wins, `reconcile` with the key
  removes the older". The attach time leaks, the same kind of fact as the
  note's own ULID.
- Tar headers must be normalized (mtime, uid, user name fixed), so `rekey` and
  the migration verify step can compare bytes.
- What leaks: the owning note's ULID (already in the note's filename), how
  many files a note has, their sizes, and their attach times. Names, types and
  content do not leak under the tar scheme. Under `<name>.age` the names leak
  too. Sizes and counts are already outside the model for notes
  (`docs/design/encryption.md:26-28`).
- Costs of the tar scheme: resolving a link takes one header decrypt per blob
  in the folder (fine for five screenshots, 300 files would pay 300 X25519
  unwraps per render, to be measured, not assumed; inside one command the
  name map is resolved once and reused). Replacing a diagram is a new blob
  plus a later cleanup, so sync sees an add and a delete. The folder is opaque
  to its owner without ntropy.
- Locked vault: `reconcile` can report orphan folders under both schemes. It
  cannot report dangling asset links or unreferenced blobs, since the links
  are inside encrypted notes. Opus and fable agree this is equal for both
  schemes, and the real extra cost of the tar scheme is the header read plus
  the unreadable folder.
- No encrypted asset may sit directly in `all-notes/`: a top-level `.age`
  file is decrypted and warned about on every scan (opus lab `v-enc2`, ADR
  0073). The ADR for the layout should state that hand-encrypted files beside
  the notes are unsupported.
- Rendering: `render --to html` copies through `RenderContext::copy_file`
  (`src/render/mod.rs:75-77`, implemented at `src/bin/ntropy/run/render.rs:512`),
  and `site` copies with `std::fs::copy` in `src/site/build.rs:532-537`. Both
  need a decrypting copy for encrypted sources, and the existence test in
  `mirrored()` (`build.rs:473-490`) must learn the storage form. The staging
  directory already lives in the runtime directory
  (`src/bin/ntropy/run/render.rs:381-395`). Typst needs real files under its
  root, and plaintext must not be staged inside the vault. So the compile root
  moves to a staging directory holding the decrypted assets, the document and
  the theme. This is the "narrower root" option of todo
  [01M66B32ZP0A6WRDY0QV26AHNX](../../../todos/all-notes/01M66B32ZP0A6WRDY0QV26AHNX-decide-how-much-of-the-vault-a-typst-theme-may-read.md),
  and it also closes the symlink escape of todo
  [01M66B2JM5XQDK7M3ZVKA1NZJV](../../../todos/all-notes/01M66B2JM5XQDK7M3ZVKA1NZJV-refuse-note-images-that-leave-the-vault-through-a-symlink-in-pdf-renders.md).
  Nobody prototyped it. If themes read other vault files by a vault-absolute
  path, those files would have to be staged too (speculation, both).
- Editing: stage the decrypted note as `<tmp>/<name>.md` and its folder under
  `<tmp>/assets/<ULID>/`, so the relative link resolves in the preview, and
  re-encrypt new or changed files on exit (`src/bin/ntropy/run/edit.rs`,
  `securetemp.rs`). Large folders make this slow and put more plaintext in the
  runtime directory, so it may be opt-in.
- `vault encrypt`, `decrypt` and `rekey` walk asset folders, with the same
  additive, verified migration and crash-safety marker as notes (ADR 0041,
  0080). `reconcile` adopts plaintext files dropped into an asset folder of an
  encrypted vault, as it adopts plaintext notes (`src/reconcile.rs:305`).
- LSP: `decrypted.rs` materializes notes only, so assets would need the same.
- Cost: large, dominated by the Typst staging root and the migration paths,
  plus the locked-vault selector path for `asset add`. Modules: `src/cipher.rs`, `src/crypto/age_io.rs`, `src/migrate/`,
  `src/render/typst/engine.rs`, `src/site/build.rs`,
  `src/bin/ntropy/run/edit.rs`, `securetemp.rs`, LSP `decrypted.rs`.

## C. Notes as directories (bundles)

- Layout: `all-notes/<ULID>-<slug>/index.md` plus its files, like Hugo leaf
  bundles and TextBundle. Link `![](shot.png)`.
- For: the shortest, most portable link. The only layout where `mv` and
  `rm -r` treat a note and its files as one unit. A view that links the
  bundle directory keeps relative links working (fable, by symlink semantics,
  not lab tested).
- Against: it breaks the flat store and the filename identity (ADR 0003,
  0004), the note-link form (ADR 0028, 0086) and every existing link in every
  vault, the scanner's depth-1 rule (`src/scan.rs:119-145`, ADR 0019),
  filename parsing (`src/note/filename.rs`), duplicate handling (ADR 0067),
  the encrypted filename `<ULID>.age` (ADR 0041), the export rule that a note
  is a file directly in `all-notes/` (ADR 0073), the selectors of ADR 0090 and
  0091, and the language server's file-per-note model. `[x](<ulid>-<slug>.md)`
  becomes `[x](../<ulid>-<slug>/index.md)` in every note. Either every note
  becomes a directory or a note is promoted when it gains an asset, which
  means two shapes in every code path.
- Retitles rename the directory, which brings back A's problem, unless the
  directory is named by ULID only, and then every directory is unreadable.
  Bundles named `<ULID>-<slug>/` hit the `parse_name` bug as soon as one holds
  a second `.md` file (opus).
- Encryption: `<ULID>/note.age` plus `<ULID>/<name>.age`, or one archive per
  bundle.
- Shared assets: `../<other bundle>/x.png`, which breaks when the other note
  is retitled.
- Migration: every vault and every link.
- Cost: very large, nearly every module.
- Verdict: rejected. The right model for a tool designed around it, and the
  wrong one to retrofit. B gets most of the benefit, a defined unit per note
  that is deleted with the note and staged as a unit, without touching note
  identity. A future `move` or `export` command could give the unit at the
  tool level (opus).

## D. Content-addressed store

- Layout: `assets/<sha256>.<ext>` or sharded, links
  `![](../assets/9f86d081....png)`. Dedup is free and files are immutable.
  Fable adds integrity checks against silent corruption in sync and immutable
  URLs for a published site.
- Against: unreadable names. Editing an image changes its address, so every
  link needs rewriting. Deletion needs garbage collection by scanning all
  bodies. Under encryption a plaintext hash in the name lets a storage
  provider confirm that a known file is in the vault. The ciphertext hash is
  useless for dedup because age encryption is randomized. A keyed hash needs
  the identity for every write, against ADR 0041. A random name loses the
  dedup.
- git-annex and LFS give content addressing below any layout. If dedup ever
  matters, `reconcile` could report identical files across folders without
  changing the layout (fable).
- Cost: large.
- Verdict: rejected. It solves dedup, which nobody asked for, and costs
  readability and lifecycle.

## E. Assets as first-class ULID objects

- Layout: `all-notes/assets/<asset ULID>-<name slug>.<ext>`, one flat
  directory, linked `![](assets/01M7QX3H5K0000000000000000-org-chart.png)`.
  Resolved by its ULID prefix like a note link, its name part refreshed by
  `reconcile`. Encrypted: `assets/<asset ULID>.age`, a one-to-one mirror of
  notes, with the readable name only in the link inside the encrypted note.
- For: sharing is natural, any note links any asset. The cleanest encryption
  story: no name leak and no per-note count. Opus calls it the most
  consistent with ntropy's note model and the best choice if shared assets
  were the common case. Fable's counter: asset ULIDs minted at attach time
  cluster around the note's creation time and sync timestamps tie a new blob
  to the note changed in the same commit, so E blurs the association and
  does not remove it.
- Against: no ownership. `delete` cannot know which files belong to the
  deleted note without a reference scan, and in an encrypted vault that scan
  needs the key, so orphan handling does not work on a locked vault. Every
  attachment needs a minted name, so a file can only be added through ntropy,
  and an agent that writes ten screenshots calls the command ten times. One
  directory holds every asset of the vault, awkward in a file browser and in
  path completion. "Which files belong to this note" has no filesystem answer,
  only a parse, so staging for an encrypted edit means parsing links first.
  Files added during the edit have no owner yet.
- Migration: every file renamed.
- Cost: medium to large, a second identity kind through scanner, links,
  reconcile, LSP and migration.
- Verdict: runner-up. For meeting minutes with their own screenshots and
  transcripts, ownership matters more. The count of cross-note links in
  `primary` found no shared asset (80 of 80 links and 16 of 16 image links
  point at the linking note's own files, see
  [current-state.md](current-state.md#measured-in-the-primary-vault)), which
  removes the case for E in that vault.

## F. Keep the convention, add checks only

- Todo option 3. `reconcile --strict` or a lint reports asset links to missing
  files and folders whose ULID prefix matches a note with another basename or
  no note. Needs a generic local-link extractor beside `link::extract`.
- `new`, `delete`, views, LSP, renderers, encryption unchanged. Migration
  none, cost small.
- Detects the incident and fixes nothing: folder names still go stale on every
  retitle, external paths still break, `delete` still leaves orphans, and
  encryption has no defined unit to work on. The title leak stays.
- Verdict: not chosen alone. It is a strict subset of B's phase 1 and both
  researchers make it the first commit of B.

## G. A custom link scheme

- `asset:<ULID>/chart.png` or Joplin's `:/id`. Resolution becomes trivial and
  every plain Markdown preview breaks.
- Verdict: rejected by fable with the reasoning of ADR 0028, which rejected
  wikilinks and ULID-only targets for the same reason. Opus agreed in the
  critique.

## H. Frozen basename

- `<ULID>-<slug at creation>`, never renamed, resolved by ULID prefix.
  Readable, stable, external paths never break. Raised by opus in critique 1
  as the strongest counter-design to B's readability problem. Fable agreed
  with the rejection in response 2.
- Against: after a retitle the name is misleading, the state fable calls
  "wrong forever" and the incident found for two folders. It still leaks the title under encryption, so
  encrypted vaults need bare ULIDs anyway. It is the shape the `parse_name`
  bug hits if the folder sits beside the notes. Opus rates a name that lies
  worse than no name. Fable adds that it is what `primary` has today for
  every note retitled since its folder was made.
- Verdict: rejected.

## Readable folder names

The user's principle (see [work-order.md](work-order.md)) is that ntropy's
structure should be human readable and navigable without ntropy. Option B
conflicts with it, plainly: `assets/<ULID>/` carries no title, so a person
browsing the folder cannot tell which note it belongs to without looking the
ULID up. Both researchers knew this as B's "downside" and weighed it as taste,
answering it with `asset list`, `ls *<ULID>*` and an optional derived view.
Under the stated principle those answers need ntropy or a shell and do not
count as the correlation the principle asks for. The variants below are
therefore the real alternatives and not dismissed ones.

| Variant | Idea | Origin |
| :--- | :--- | :--- |
| a | Folder named exactly like the note file, `assets/<ULID>-<slug>/`, bare `<ULID>` in an encrypted vault | Option A, with the encrypted-vault naming both researchers already wrote into A. Lead's reading: this matches how note files are named |
| b | `<ULID>-<slug at creation>`, never renamed | Option H |
| c | Bare `<ULID>` plus a derived readable view | Fable's derived directory with B |
| d | Folder `all-notes/<ULID>-<slug>/` beside the note | Location variant of B with the name changed |
| e | Real folder `<ULID>-<slug>/` plus a tracked symlink `assets/<ULID>` as the stable link address | From the document review, lab-checked for HTML and PDF only |
| f | A file-name prefix instead of a folder: `<ULID>-<name>.png` in one directory | Convention found in `primary`; from the document review, not argued by the researchers |

Facts that bear on every variant:

- `reconcile` already renames a note file when its title or the slug rules
  change and rewrites note links in plaintext vaults (`src/reconcile.rs:221`,
  `rewrite_links` at `:396`). `realign` after `edit` and `write` renames only
  the note and rewrites no link (`src/reconcile.rs:476-520`). `link::extract`
  skips images (`src/link/mod.rs:73-75`), so `rewrite_links` does not reach
  asset links today. The 2026-11-02 incident was a `reconcile` that renamed 20
  notes while the asset folders of two kept their old names (todo
  [01M6E0CWM5KS1FRX3DTB3VREB7](../../../todos/all-notes/01M6E0CWM5KS1FRX3DTB3VREB7-decide-how-ntropy-stores-names-and-reconciles-note-assets.md)).
- A link into a renamed folder does not resolve by ULID today, as a note link
  does. A rename of the folder leaves its links broken until they are rewritten.
- In an encrypted vault the note file is `<ULID>.age`, the slug is dropped on
  purpose (ADR 0041), and `reconcile` rewrites no links
  (`src/reconcile.rs:221`).
- Note links keep the `<ULID>-<slug>.md` form in both storage modes, so
  `vault encrypt` and `vault decrypt` rewrite no bodies
  (`src/note/mod.rs:163-176`). The migration verifies every produced file by
  reading it back and comparing it with its source's plaintext
  (`src/migrate/mod.rs:14-22`).
- `primary` has 25 folders named `<note basename>` and a flat `images/`
  directory of `<note ULID>-<name>.png` files
  ([current-state.md](current-state.md#measured-in-the-primary-vault)). The
  migration line is the same for every folder variant: the folders themselves
  are unchanged under a and b, renamed and rewritten under B and c, moved under
  d, and gain a symlink under e; the 16 `images/` files are moved into folders
  and their links rewritten, or `images/` is left unmanaged. The 4 folders no
  note links are reported once and what to do with them is not decided.
- Checks and rewrites that must find asset links use a `pulldown-cmark` pass
  with the renderer's options, since a regex misses reference-style,
  angle-bracket and raw `<img>` forms (settled points below).

### (a) Folder named exactly like the note file

Layout `assets/<ULID>-<slug>/`. `reconcile` renames the folder together with
the note and rewrites the note's own asset links in the same pass. Option A
above has the full dimension list.

What is reused and what is new. Reused: `reconcile` already renames note files
and identifies notes by ULID prefix, and `rewrite_links` is a tested rewrite
over bodies. New: the folder rename step, a finder for asset links (extract
skips images), the rewrite in foreign notes, and the handling in `realign`.

- Incident: removed by automation, since the folder follows the note on every
  `reconcile`. A retitle that renames the folder before the links are rewritten
  leaves broken images until the next `reconcile`, which finishes the job
  because it identifies the folder by ULID prefix, as it does for note
  renames. What remains is a sync conflict when two machines retitle
  independently (opus, fable, reasoning, not tested).
- Outside paths: break on every retitle and every slug-rule change, which is
  the Muesli failure of the incident, now at every rename.
- Links from other notes into the folder need rewriting too. None exist in
  `primary` today (measured).
- Link finding: the rewrite uses the Markdown pass or a literal replacement of
  the old folder name. A plain regex pass misses some link forms.
- Git: git detects the rename.
- Readability: the strongest of the variants. Folder and note have the same
  name, which is what the principle asks for.

Sub-decision, how `realign` behaves, since it runs after every `edit` and
`write`:

1. `realign` renames the folder and rewrites the note's own asset links. The
   folder moves under an open editor. Rewriting other notes from `realign` is
   a write pattern ntropy has nowhere today.
2. `realign` leaves the folder. Links stay valid because the folder has not
   moved, and the next `reconcile` renames it.
3. Rename on demand: `reconcile` only reports drift between folder and note
   name and an explicit command renames. This is the middle between a and the
   settled "reconcile only reports" rule for conversion, with the same
   reason: agents run `reconcile` routinely. It keeps readability and removes
   the surprise, and drift stays until the user runs the command, which is the
   state the incident found.

Encrypted-vault sub-variants (the lead's reading, not evaluated by the
researchers beyond "two layouts for one concept"):

- enc-1: the folder is bare `<ULID>` and `vault encrypt` rewrites the asset links
  in every note body to match. That breaks the invariant that the migration's
  verify step compares a produced file with its source's plaintext.
- enc-2: the link text stays `assets/<ULID>-<slug>/x` in both modes, as note links
  keep `<ULID>-<slug>.md`, and the folder is found by ULID prefix when the
  vault is encrypted. The conversion renames folders only. The link text of an
  encrypted note then does not match the folder name, as note link targets do
  not match `<ULID>.age` today. Decrypt must derive a slug from the title for
  the folder name, and outside paths break at each conversion.

In both, the title leak of today's convention is removed, plaintext vaults stay
readable, the encrypted vault is as unreadable as its note files, and the
per-note count and size leak and the file-name question stay as in B, phase 2.
Not prototyped.

### (b) `<ULID>-<slug at creation>`, never renamed

Option H. Resolved by ULID prefix.

- Incident: no drift, because nothing renames. The name is correct at creation
  and wrong after a retitle, the state fable calls "wrong forever" and the
  incident found for two folders. `primary` already has this for every note
  retitled since its folder was made.
- Outside paths: stable.
- Encryption: leaks the title, so encrypted vaults need bare `<ULID>` anyway.
- Migration of `primary`: the folders are unchanged, the existing names are
  recognized by ULID prefix. Recognition is the one cost.
- Readability: correlation by ULID prefix survives a retitle, the human
  readable title does not. Opus rates a misleading name worse than none.

### (c) Bare `<ULID>` plus a derived readable view

Fable's derived directory with option B: `<name>/<date>-<slug>` symlinks to
`assets/<ULID>/`, rebuilt by `reconcile`, gitignored like a view (ADR 0008,
0032). Not prototyped.

- Incident: none.
- Outside paths: stable.
- Encryption: views do not exist in encrypted vaults (ADR 0041), so there is
  no readability there, where the names must stay hidden anyway.
- Migration of `primary`: rename each folder to `assets/<ULID>/` and rewrite
  link prefixes by literal replacement.
- Readability: through the view, not through the folder. The view is
  gitignored, so absent in a clone, and stale between reconciles. A person
  navigating `assets/` without ntropy still sees ULIDs. Of the readable
  variants it is the one the principle fits least.

### (d) `all-notes/<ULID>-<slug>/` beside the note

The location variant of B with the name changed.

- Needs the `parse_name` fix first: a link to `<ULID>-<slug>/transcript.md` is
  read as a note link and rewritten by `reconcile`, and the renderers misread
  it before any retitle (todo
  [01M7MNSS8YXMWC461EWYT9HFFD](../../../todos/all-notes/01M7MNSS8YXMWC461EWYT9HFFD-a-link-into-a-folder-named-like-a-note-is-read-as-a-link-to-that-note.md)).
- Incident, outside paths and encryption: as (a). Encrypted vaults would use
  `all-notes/<ULID>/` beside `<ULID>.age`.
- Migration of `primary`: the folders move out of `assets/`, and the link
  prefix changes from `assets/<basename>/` to `<basename>/`.
- Readability: the folder sits next to its note in any listing with the same
  name, and `all-notes/` gains one directory per note with attachments.

### (e) Readable folder plus a tracked symlink as the stable address

From the document review. The real folder is `assets/<ULID>-<slug>/`, renamed by
`reconcile` as in (a), and a symlink `assets/<ULID>` points at it. Links use
the stable address `assets/<ULID>/x`.

- Lab (review): HTML copies and PDF embeds through the directory symlink, and
  `reconcile` leaves the symlink alone.
- Incident: links never name the changing folder, so a rename breaks none.
  Outside paths through `assets/<ULID>/` survive a retitle. Paths through the
  readable folder name do not.
- Costs: two entries per note in `assets/`. HTML and site publish the file under
  the alias path, and under both if both spellings are linked. A directory
  link that mirrors a folder does not follow symlinked directories
  (`src/site/scope.rs:560-572`). The readable folder is stale between a
  retitle and the next `reconcile`.
- Views are symlink trees too, but they are derived and gitignored, while this
  alias is tracked. ntropy builds on Unix only (ADR 0020).
- Untested hypotheses: how the GitHub web view treats directory symlinks, how
  sync tools carry symlinks, how editor previews resolve them.
- Encryption: the readable folder leaks the slug, so encrypted vaults use bare
  `<ULID>` without an alias.
- Readability: the folder is readable, the link is not.

### (f) A file-name prefix instead of a folder

Found in `primary`: a flat `all-notes/images/<note ULID>-<name>.png`. Ownership
is read from the file name prefix, no folders.

- Not argued by either researcher and not evaluated here beyond the facts: no
  title in the name, so it does not answer the readability principle beyond
  B. Hand-dropping a file needs the prefix. `delete` finds the files by
  prefix. Encryption leaks the file names and the per-note count unless names
  are hidden as in B, phase 2.
- Migration of `primary`: `images/` needs none. The 25 folders would turn into
  prefixed files with rewritten links, or both conventions coexist.

### Location per name

The location choice depends on the name. For the bare ULID see the table under
B. For `<ULID>-<slug>`:

| Location | Link | Consequence |
| :--- | :--- | :--- |
| `all-notes/assets/<ULID>-<slug>/` | `assets/<ULID>-<slug>/x` | As in `primary` today. The `assets/` prefix keeps the link from starting with a ULID, so `parse_name` does not see it. |
| `all-notes/<ULID>-<slug>/` | `<ULID>-<slug>/x` | Needs the `parse_name` fix first. Sorts next to its note with the same name. |
| Vault-root `assets/<ULID>-<slug>/` | `../assets/<ULID>-<slug>/x` | Same drawbacks as for the bare ULID. |

### Comparison of the naming variants

| | a | b | c | d | e | f | B (bare ULID) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Human readable without ntropy | yes | yes, stale after a retitle | folder no, view yes | yes | folder yes, link no | no title | no |
| Rename drift | removed by automation, rename on demand keeps it until the command runs | no renames | no renames | as a | links stable, readable folder stale until `reconcile` | no renames | no renames |
| Outside paths survive a retitle | no | yes | yes | no | alias path yes | yes | yes |
| Encryption | bare `<ULID>` there | needs bare `<ULID>` there | no view there | bare `<ULID>` there | bare `<ULID>`, no alias | names leak | same layout |
| Migration of `primary` folders | none | none | rename and rewrite | move and rewrite | add symlinks | prefix and rewrite | rename and rewrite |
| Needs `parse_name` fix | no | no | no | yes | no | no | no |

## Comparison

| | A basename | B ULID folder | C bundles | D content hash | E asset ULIDs | F checks |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Survives a retitle without a write | no | yes | links yes, directory no | yes | name part only | no, stale name |
| Path computable from the ULID alone | no | yes | no | no | no | no |
| Readable in a file browser | yes | no | yes | no | names yes, owner no | yes |
| `delete` knows the note's files without parsing | yes | yes | yes | no | no | no |
| `delete` protects files other notes use | scan of all bodies | scan of all bodies | scan of all bodies | scan of all bodies | scan of all bodies | scan of all bodies |
| Same layout plaintext and encrypted | no | yes | yes | yes | yes | no |
| Encrypted name leak | slug, needs a second layout | ULID, count, sizes | ULID | hash or nothing | ULID, sizes | full |
| Hand-dropping a file works | yes | yes | yes | no, as designed | no | yes |
| Existing note links untouched | yes | yes | no | yes | yes | yes |
| Shared assets | link rewrites | link into owner or free folder | awkward | natural | natural | as today |
| Migration of `primary` | none | folder rename and link prefix | everything | everything | every file | none |
| Cost | M | M, then L for encryption | XL | L | M to L | S |

## The recommendation both converged on

This recommendation conflicts with the user's stated principle that the
structure be readable and navigable without ntropy, see
[Readable folder names](#readable-folder-names). It is the researchers'
conclusion before that principle was stated, and it stays here as the option
that scores best on drift, outside paths and encryption.

Option B at `all-notes/assets/<ULID>/`, plain relative links, ownership read
from the path, built in phases:

1. Fix what is wrong regardless of the design: the `parse_name` bug, the
   percent-encoded path bug, the dead link for a missing file in HTML, and a
   PDF check so a missing or outside image warns instead of failing, with the
   symlink rule of todo 01M66B2JM5XQDK7M3ZVKA1NZJV.
2. Phase 1: layout and ownership rule recorded in an ADR, recognition through
   a `pulldown-cmark` pass, `reconcile` checks (F's checks are the first commit),
   the legacy conversion command, `delete`, `asset add` and `asset list`, LSP
   support, docs and skill. Fable's order: ADR, local-link extractor and
   checks, conversion, `delete` and commands, PDF handling, LSP, encryption.
3. Phase 2: encryption, decided together with todo
   01M66B32ZP0A6WRDY0QV26AHNX (the Typst root), staged edit previews and
   migration commands.

This recommendation was formed before the readable variants were treated. The
settled points below are split by whether they hold for every naming.

Why, in the researchers' words: the incident came from a name that had to
follow the title. B removes the name instead of automating the follow-up. It
keeps the property every plain-Markdown tool relies on, a relative path to a
real file, and adds the one property only ntropy can add, an owner, because
the ULID is already in every note's filename. ntropy made this call once
before: encrypted notes are `<ULID>.age` because the ULID is the identity and
the slug is decoration. Lab evidence that this matters: the `enc` vault shows
the title in the asset folder name beside `<ULID>.age`.

### Arguments against it, and how they were answered

- **Readability.** Hundreds of 26-character folders in Finder. Fable: that is
  a CLI answer (`asset list`, `ls *<ULID>*`) to a file-browser problem.
  Answers offered: those commands, the location variant `all-notes/<ULID>/`,
  and an optional derived directory of symlinks named `<date>-<slug>`
  pointing at the asset folders, rebuilt by `reconcile`, gitignored like a
  view and absent in encrypted vaults. That derived directory was not
  prototyped. It is a user decision, see
  [open-questions.md](open-questions.md).
- **Encryption leaks per-note counts and sizes.** Opus: "note 01M7... has 14
  files and 300 MB" says "recorded meeting". Fable: B ties every blob to a
  note in the clear. Answer: inherent to a per-note folder that stays
  readable on a locked vault. E blurs but does not remove it, and an archive
  per note would hide the count but rewrite the whole blob on every change.
  Both recommend stating the leak in the threat model, as note count and
  sizes already are. User decision.
- **Location.** See the variants table. Both stayed with
  `all-notes/assets/<ULID>/`. See also [Location per name](#location-per-name).
- **Migration risk.** Rewriting link prefixes by regex could miss forms and a
  crash between folder rename and body rewrite is a risk. Answer: a literal
  replacement of the unique old folder name, legacy prefix kept recognized so
  a rerun finishes the job (see settled points).
- **Shared assets are second class.** A logo used by twenty notes lives in one
  note's folder or outside management. Answer: the three ways under B.
  In `primary` no asset is shared between notes (measured).
- **Bundles give a filesystem unit.** `delete` gives the unit at the tool
  level, and rewriting every note link in every vault is not worth the
  filesystem-level unit.
- **Link rewriting.** After migration there is none, ever. Opus calls it the
  strongest point for the design, and no critique dented it.

## Technically settled points

Agreed by both researchers after the critiques. The first group holds
whatever the folder is named. The second group was settled for the bare
ULID folder only.

### Regardless of the naming

- **Checks use a Markdown parser, not a regex.** Fable's lab `haz` shows
  reference-style and angle-bracket links render today and a `LINK_RE`-style
  pattern sees neither, so a regex lint would call used files unreferenced.
  Checks, `delete`'s reference scan and LSP diagnostics use `pulldown-cmark`
  with the renderer's `parser_options()` (`src/render/markdown/mod.rs:304`), as
  `src/link/code.rs:28-38` already does for code masking, so they see exactly
  the links the renderers see. The render walk itself exposes no spans to its
  outputs. The asset ADR corrects ADR 0028's sentence "ntropy still ships no
  Markdown parser" (`:56`), which link masking already contradicts. Opus adds a
  question for later: whether note links should move to the same pass, since
  reference-style note links are invisible to `reconcile` for the same reason.
- **`delete`.** Delete the note. Remove its folder when no other note
  references a file in it. When others do, keep the folder, warn, name them,
  and let a dedicated flag remove it anyway. `--force` keeps its one meaning,
  skipping the confirmation. The kept folder shows up as an orphan in
  `reconcile` with its referrers. Fable's earlier "refuse without `--force`"
  was dropped because a non-interactive delete already requires `--force`.
  Whether `delete` removes the folder by default at all is a user decision.
- **PDF missing or outside image.** A warning and a placeholder, failing under
  `--strict` as HTML already does. Today it fails the whole render. Checked in
  the typst engine with `ExportScope`, as the HTML engine does
  (`src/render/html/engine.rs:93`, `src/site/scope.rs:504`), which brings the
  ADR 0073 rules of todo
  [01M66B2JM5XQDK7M3ZVKA1NZJV](../../../todos/all-notes/01M66B2JM5XQDK7M3ZVKA1NZJV-refuse-note-images-that-leave-the-vault-through-a-symlink-in-pdf-renders.md)
  with it. The engine's note that it touches no filesystem
  (`src/render/typst/engine.rs:112-113`) is amended. `src/render/prepare.rs` is
  the alternative, against its lossless-only contract (`prepare.rs:7-12`). The
  todo is updated to match. The emitter then receives the verdict.
- **PDF non-image links elsewhere in the vault.** Lead's technical
  recommendation: print as plain text with a `note:` line that `--strict`
  ignores, as fable argued, since such links are legitimate and a warning on
  every render cannot be switched off under `--strict`. The level of ADR 0073
  for routine skips is `note:`. Opus's critique said plain text plus a warning
  and did not answer the correction.
- **Site URLs** keep vault paths. The public path stays stable across retitles
  for a folder that does not rename. A mapping under the note's page can come
  later without touching vaults. Opus withdrew its mapping suggestion.
- **Commands.** `asset add [--move] [--as]` printing a link that renders,
  `asset list [--path]` marking unreferenced files. Names are the user's to
  veto.
- **A link that renders.** `asset add` either sanitizes stored names (spaces
  to `-`) or prints the angle-bracket form, and the exporters should
  percent-decode. This depends on the percent-encoded path bug.
- **Skill text.** `skills/ntropy/references/writing-notes.md:262-268` stops
  recommending files beside the notes once the layout lands. Opus's first
  question asked whether to do it earlier, before encryption. Both researchers
  then agreed on "when the layout lands".
- **ADR text on previews.** The ADR states that relative file links resolve
  from `all-notes/` and that a note opened through a view may not preview them.
- **Conversion style.** Any conversion is a one-shot command with a dry run,
  and `reconcile` only reports what it finds and names the command. The
  reasons: agents run `reconcile` routinely (the skill tells them to after an
  edit), so a conversion inside it would let any agent restructure the vault as
  a side effect. A conversion that renames folders and breaks stored absolute
  paths should happen when the user picks the moment. It would keep a legacy
  path in `reconcile` forever. In an encrypted vault it would need the key, and
  `reconcile` is meant to run locked. This reversed the position of fable's
  proposal and opus's open choice.

### Settled for the bare ULID folder

- **How the conversion rewrites.** Literal replacement of
  `assets/<old folder name>/` by `assets/<ULID>/` in every note body, plus its
  percent-encoded spelling, whatever syntax surrounds it. The old name
  contains the ULID and is unique. This also covers raw `<img>`, reference
  definitions and code blocks that quote the path. Opus's regex over link
  prefixes with code regions masked was dropped for it.
- **Conversion guards.** Refuse when both `<ULID>/` and `<ULID>-<slug>/` exist
  and when two legacy folders share a ULID. A folder whose ULID matches no
  note is reported, not touched. Print the old and new absolute paths so
  references outside the vault can be fixed. Paths copied outside the vault
  (the Muesli case) break once and never again.
- **Legacy prefix.** After conversion `assets/<ULID>-<anything>/` stays
  recognized as legacy and reported, because a second machine can sync in an
  old-style link. It costs one prefix match.

### Technical points the readable variants open

- How `realign` behaves for a renamed folder (the three choices under a).
- Whether rename is automatic in `reconcile` or on demand.
- How the encrypted-vault conversion treats link text (enc-1 or enc-2 under a).
- How other notes' links into a renamed folder are found and rewritten, and the
  write pattern that rewriting from `realign` would add.
- The alias symlink's behaviour in the site export and untested viewers (e).
- Whether the `parse_name` fix is a precondition (d).

## Ideas withdrawn

- **An `assets` symlink in every view directory** (fable's proposal), so
  `assets/<ULID>/x.png` resolves from a view entry. Lab (fable `optb`): it
  works with `by-tag/probe/assets -> ../../all-notes/assets`. Withdrawn
  because: a tag `probe/assets` already makes `by-tag/probe/assets/` a group
  directory (lab, both researchers), the fix would be a new rule on which tag
  values a user may write because ADR 0064 validates view names and not field
  values, leaf directories sit at several depths so every directory needs its
  own link with its own `../` count, recursive tools such as `find -L` would
  walk the asset tree once per view directory, it helps only managed folders
  while `../assets/x.png` in the docs vault stays broken, and the problem
  itself is untested, since neither researcher checked which editors resolve
  relative paths against the symlink and not its target. Fable withdrew it and
  recommends no. Opus first wanted views left alone, then judged it
  technically feasible with a rule that renames a colliding group directory
  (for example `assets-1`, like leaf collisions, ADR 0068) and left it to the
  user without a recommendation.
- **Hashed asset file names** (fable). An unkeyed hash of a guessable name
  can be confirmed by dictionary. A keyed hash needs the identity to write,
  which breaks `asset add` on a locked vault, against ADR 0041. A hashed name
  also loses the file's name for anyone recovering with stock `age` without
  the note's links.
- **A conversion inside `reconcile`** (fable's proposal), for the reasons above.
- **"Any per-note folder directly in `all-notes/` needs the `parse_name` fix
  first"** (opus). True for `<ULID>-<slug>/` only.
- **A regex pass in `src/link/` for asset links** (both proposals), for a
  `pulldown-cmark` pass.
- **Asset id "ULID or random"** (opus), for ULID.
- **The PDF check in the emitter** (opus), for the typst engine with
  `ExportScope`.
- **`asset dir`** (fable), for `asset list --path`.
- **Site URL mapping under the note's page** (opus).
