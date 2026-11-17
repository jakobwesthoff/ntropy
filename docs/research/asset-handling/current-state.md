# Current state

What ntropy does with assets at commit `ed914a3`. ntropy has no concept of an
asset. The scanner ignores everything in `all-notes/` except top-level note
files. The renderers and exports handle linked files better than the user
assumed. What is missing is everything around the file: where it lives, what
it belongs to, and what happens on retitle, delete and encryption.

## The user's assumptions, checked

| Assumption | Verdict | Evidence |
| :--- | :--- | :--- |
| Nothing reconciles asset directories | Correct | Lab: retitle a note, run `reconcile --strict`. Output `renamed ...-asset-test-note.md -> ...-renamed-asset-note.md`, `0 warnings`, exit 0, folder name unchanged. This is the 2026-11-02 incident. Code: `reconcile` rewrites note links only (`src/reconcile.rs:396`, `rewrite_links`), and has no path that touches a directory in `all-notes/`. |
| Unsure whether single or multi document HTML carries assets | Wrong, they are carried | There is one HTML format, a page plus a `<stem>_files/` directory. ADR 0057 lists "one self-contained file" among the rejected alternatives. `render --to html` and `site` copy every referenced Markdown-linked file inside the vault, images and non-images alike, except what ADR 0073 refuses: listed names, and a `.md` or `.age` directly in `all-notes/` that is not an exported note, such as a transcript beside the note (ADR 0057, 0072, 0073). |
| Images render in PDFs | Partly | True for images inside the vault, wherever they sit. One missing image or one image outside the vault fails the whole render. Linked non-image files are not embedded, the PDF gets a relative `/URI` annotation. |
| Assets are not encrypted | Correct, and worse | Notes become `<ULID>.age`, assets stay plaintext. The folder name of today's convention carries the slug that ADR 0041 removes from note filenames on purpose. |

## Per output

The test note linked a PNG, a PDF and a ZIP in each placement of the table
further down.

### PDF (`render --to pdf`)

How it works. The emitter writes a local image as `#image("<path>")` with the
path resolved against the note's root-absolute directory
(`src/render/typst/emitter.rs:229`, `resolve_asset` at `:575`, which calls
`markdown::resolve_root_relative`, `src/render/markdown/mod.rs:1420`). A path
that climbs out of the vault passes through as written and the compiler
rejects it by name (the doc comment above `resolve_asset`, `docs/design/typst-engine.md`, "Asset
paths"). The compile runs with `typst compile --root <vault>`
(`src/render/typst/engine.rs:116-123`, ADR 0045, ADR 0074; the table check at `:276` passes the same root). A non-image link
becomes `#link("<dest>")` with the destination as written (`emitter.rs:584`,
`wrap_link`).

Lab results, from the opus and fable labs, which agree:

- All in-vault images embedded: `pdfimages -list` (fable) and a byte-level
  inspection (opus) show one embedded image per image link in placements a, b
  and c.
- The `/URI` actions of the non-image links are the targets as written, for
  example `beside.zip`, `assets/<basename>/convention.pdf`,
  `../shared/else.pdf`. A viewer resolves them against the PDF's own
  location, so they work only while the PDF sits in `all-notes/`. The PDF has
  no `/EmbeddedFile` entry.
- An image outside the vault: exit 1, `path "../../outside/outside.png" would
  escape the project root`.
- An absolute filesystem path is read as a vault-root path (ADR 0072) and
  fails with `file not found (searched at <vault>/Users/...)`.
- A missing image fails the render with typst's `file not found`.
- A file symlink inside the vault that points outside is followed and
  embedded. This is a documented residual (ADR 0070, todo
  [01M66B2JM5XQDK7M3ZVKA1NZJV](../../../todos/all-notes/01M66B2JM5XQDK7M3ZVKA1NZJV-refuse-note-images-that-leave-the-vault-through-a-symlink-in-pdf-renders.md))
  and was not re-tested in this research.
- Typst 0.15.1, the installed version, compiles `#pdf.attach("beside.zip")`
  and the PDF then carries the file (opus: `/EmbeddedFile` present, lab
  `embedtest/t.typ`; fable: `pdfdetach -list` shows it). ntropy does not use
  `pdf.attach`.

The emitter is a pure function of the body and a base string and touches no
filesystem (`src/render/typst/emitter.rs:86-106`). The typst engine's
documentation says it touches neither the filesystem nor the working
directory (`src/render/typst/engine.rs:112-113`), and `src/render/prepare.rs`
is the lossless-only half of rendering (`prepare.rs:7-12`). The HTML engine
already judges paths with `ExportScope` (`src/render/html/engine.rs:93`,
`src/site/scope.rs:504`). A check for missing or outside PDF images is
settled on that precedent in [options.md](options.md#technically-settled-points).

### `render --to html`

Referenced paths are resolved against the note's directory and copied through
the site's mirror (`src/render/html/engine.rs:64` `asset_href`,
`src/site/build.rs:455-489` `mirrored`, ADR 0057, 0072, 0073).

Lab results:

- Placements a, b, c: files copied to
  `<stem>_files/files/<vault path>`, `src` and `href` rewritten, for images,
  PDFs and ZIPs alike. A link to a directory mirrors the directory.
- Placement d (outside the vault): a warning
  `points outside the vault and is not copied`, the target kept as written.
- A missing file warns `referenced file not found in the vault`. Without
  `--strict` the exit is 0, with `--strict` it is 1 (fable's lab, confirmed by
  opus's lab `m2.html` and `m3.html`).
- `render --help` still calls `html` "a self-contained web page"
  (`src/bin/ntropy/cli.rs:201`), stale since ADR 0057.

A missing file in the vault gets a rewritten, dead link, although
`docs/design/html-engine.md:75-76` says a missing file "is a warning and stays
unresolved". The code explains it: `ExportScope::resolve` returns `Ok(None)`
for a path that does not exist (`src/site/scope.rs:446-449`), so `href_path`
(`src/site/scope.rs:504`) accepts it and the link is rewritten, and only the
copy step warns. Filed as todo
[01M7MNV5C1Y9AMQXP0TJ0T7E7V](../../../todos/all-notes/01M7MNV5C1Y9AMQXP0TJ0T7E7V-html-and-site-exports-point-a-missing-file-at-a-dead-path-in-the-files.md).
The opus lab saw it for the absolute filesystem path only, the fable lab for
every missing in-vault path, and the todo reproduces all three variants.

### Site export (`site`)

Same copy logic (`docs/design/site-export.md`, ADR 0054, ADR 0073). Lab:
`site -o site` writes `files/all-notes/...` and `files/shared/...`, the note
page links `../files/...`, and the warnings are the same three as for HTML.
Asset URLs mirror vault paths (`FILES_DIR`, `src/site/build.rs:55`), so
today's convention publishes
`files/all-notes/assets/<ULID>-<slug>/<file>`, ULID and slug included. ADR
0054 rejected ULIDs in note page URLs (`docs/adr/0054-...md:118`), so file
URLs and page URLs follow different rules today.

### ANSI (`show`, `render --to ansi`)

An image is `[image: alt]` with the source dimmed after it, a link shows its
target (`docs/design/ansi-engine.md`, rows "Images" and "Links"). Lab: every
path printed as written, including missing and outside ones, no warning, no
OSC 8 hyperlinks (opus grepped for `\e]8` and found none).

## Per subsystem

### Scanner

Reads `all-notes/` at depth 1 and keeps regular files with the vault's note
extension (`src/scan.rs:119-145`, ADR 0019, `docs/design/vault-layout-and-views.md:43-50`).
Everything else, including every subdirectory, is a resource and ignored
silently. Nothing in the library lists or classifies assets. Search sees an
asset path only as text in a note body (lab: `search conv.png` finds the
note).

### `link::extract`

Finds inline links with `LINK_RE = \[([^\]]*)\]\(([^)\s]+)\)`
(`src/link/mod.rs:60`). It skips images (a preceding `!`, `:73-75`) and
accepts only targets of the form `<26-char ULID>[-<slug>].md` (`parse_name`,
`:234-243`). An asset link is therefore invisible to everything built on it:
reconcile's relinking, backlinks, language server navigation. The regex sees
no whitespace in a target, no reference-style definitions and no
angle-bracket targets. Code regions are masked with `pulldown-cmark` and the
renderer's options (`src/link/code.rs:28-38`, `parser_options()` at
`src/render/markdown/mod.rs:304`), so ADR 0028's sentence that ntropy "still
ships no Markdown parser" (`docs/adr/0028-...md:56`) is already out of date:
the regex finds candidates, the parser masks code. Since ADR 0049 the
renderers share a Markdown walk (`src/render/markdown/mod.rs`), which exposes
no source spans to its outputs.

### `reconcile`

Renames the note, rewrites note links, never touches a directory in
`all-notes/`. In an encrypted vault it rewrites no links at all
(`src/reconcile.rs:221`). `realign` runs after `edit` and `write`
(`src/reconcile.rs:476`) and is inert in an encrypted vault. Duplicate ULID
repair (ADR 0067) gives a hand-copied note a new ULID; the asset folder stays
with the note that keeps the ULID.

### `delete`

`ops::delete_note` is one `remove_file` plus a view refresh
(`src/ops/delete.rs:22-25`). Lab: after `delete -f`, the asset folder stays
with its three files, and a following `reconcile --strict` reports 0
warnings. Orphans are silent.

### Views

Views are symlinks into `all-notes/`. Lab: the entry is
`by-tag/probe/<date>-<slug>.md -> ../../all-notes/...`, and `by-tag/probe/`
holds no `assets` entry. A tool that resolves a relative link against the path
it opened, not against the symlink target, would show broken images for a
note opened through a view. Neither researcher tested which editors do that.
Lab (fable): `reconcile` deletes a foreign symlink named `assets` placed in a
view directory. Views are disabled in encrypted vaults (ADR 0041) and
gitignored (ADR 0032).

### Language server

Capabilities: completion, definition, document links, workspace symbols
(`src/bin/ntropy/run/lsp/mod.rs:145-158`). There is no diagnostics
capability: `rg diagnostic src/bin/ntropy/run/lsp` finds nothing. Document
links and definition cover note links only
(`src/bin/ntropy/run/lsp/navigation.rs:62-80`, through `link::extract`). Link
completion excludes the image bracket on purpose
(`src/bin/ntropy/run/lsp/completion/link.rs:112-114`). Asset paths get no
completion, no click-through and no warning. Encrypted notes are
materialized read-only in the runtime directory
(`src/bin/ntropy/run/lsp/decrypted.rs:5-15`), without any assets.

### Encryption

Encryption covers note files only. The `Cipher` seam reads and writes
`String` (`src/cipher.rs:71-74`), so binary files have no path through it.
Writing needs only the public recipient (`src/cipher.rs:61-67`), which is
what lets `new` work on a locked vault.

Lab (opus, `v-enc`; fable, `enc` after `vault encrypt -y`): notes become
`<ULID>.age`, the asset files stay plaintext with their names, and
`file` reports them as PNG and PDF. The folder keeps its slug:
`assets/<ULID>-<slug>/`. `docs/design/encryption.md:26-28` lists "Metadata"
as outside the model (ULIDs, note count, file sizes), not asset content.
PDF, HTML and site render from an encrypted vault with all assets only
because the assets are plaintext (both labs).

An encrypted asset placed directly in `all-notes/` collides with note
detection. Lab (opus, `v-enc2`): `beside.png.age` gives
`warning: skipped beside.png.age: decrypted content is not valid UTF-8` on
every command and one warning under `reconcile --strict`. The same file in a
subdirectory is ignored. ADR 0073 also treats any `.md` or `.age` directly in
`all-notes/` as a note for exports (`docs/adr/0073-...md:102`, `:140`).

Editing an encrypted note writes plaintext to a temp file in the runtime
directory, outside the vault (`src/bin/ntropy/run/securetemp.rs:5-16`). A
relative asset link has nothing to resolve against there, so an editor
preview would show no images. This follows from the paths and was not run in
an editor.

## Per placement

| Placement | Image in PDF | Non-image in PDF | HTML and site | Encrypted vault |
| :--- | :--- | :--- | :--- | :--- |
| a. Beside the note, `all-notes/x.png` | Embedded | Relative `/URI`, works only beside `all-notes/` | Copied to `files/all-notes/x.png`, except a Markdown file (see below) | Plaintext |
| b. `all-notes/assets/<note basename>/x.png` | Embedded | Same | Copied, path carries ULID and slug | Plaintext, folder name leaks the slug |
| c. Elsewhere in the vault, `../shared/x.png` or `/shared/x.png` | Embedded | Same, path as written (`/shared/...` points at the filesystem root) | Copied to `files/shared/x.png` | Plaintext |
| d. Outside the vault, relative or absolute filesystem path | Whole render fails | Annotation with the path as written | Warning, not copied, link kept as written (an absolute filesystem path is read as a vault path and gets a dead rewritten link, see above) | Not part of the vault |

A Markdown attachment beside the notes (placement a) is warned about by every
scan (`too short to be a <ulid>-<slug>.md name`) and never exported, since ADR
0073 treats a `.md` directly in `all-notes/` that is not an exported note as
unexportable (review lab, not part of the researchers' labs).

Placement a is what the skill advises, b is the convention in the `primary`
vault, c is what the docs vault does for shared files.

## Conventions in use

Four conventions exist in the user's own material:

1. The `primary` vault: `all-notes/assets/<note basename>/`, linked
   `assets/<basename>/<file>`. It comes from the meeting-minutes workflow
   (todo [01M6E0CWM5KS1FRX3DTB3VREB7](../../../todos/all-notes/01M6E0CWM5KS1FRX3DTB3VREB7-give-each-note-an-asset-folder-that-ntropy-manages.md)).
   The researchers did not open that vault; the lead measured it afterwards, see
   below.
2. The agent skill: attachments "right next to the notes", referenced with
   relative Markdown (`skills/ntropy/references/writing-notes.md:262-268`).
3. The docs vault of the project: vault-root `assets/<topic>/`, shared and
   unmanaged, linked `../assets/site-theme/01-quick-start.png`
   (`docs/website/all-notes/01M6CBSVXZXEW713FS476VB76F-writing-a-site-theme.md:69`).
   `docs/design/typst-engine.md:217-218` uses `/assets/logo.svg` as its
   theme asset example.
4. The `primary` vault also has a flat `all-notes/images/` directory with files
   named `<note ULID>-<name>.png` (measured, see below). The one-shot
   conversion has to cover it too.

### Measured in the `primary` vault

Measured by the lead on 2026-11-17 in the `primary` vault (283 notes, with the
user's permission, read-only, only counts recorded):

- `all-notes/assets/` holds 25 folders named `<note basename>` with 81 files,
  one of them `.DS_Store`. 80 links point into these folders. All 80 point
  into the linking note's own folder (folder ULID prefix equals the note's
  ULID), none into another note's folder, and no folder is linked by more
  than one note. 4 folders have no linking note.
- A flat `all-notes/images/` directory holds 16 files named
  `<note ULID>-<name>.png`, with 16 links. Every file's ULID prefix equals
  the linking note's ULID, and no image is linked by more than one note.
- No asset is shared between notes in `primary`.

## Limits found

- **Percent-encoded paths.** `![x](my%20pic.png)` is looked up literally. The
  HTML export warns and writes a dead link, the PDF render fails (fable lab
  `haz`, `out/h2.html`). A file with a space renders only in angle brackets,
  `![x](<my pic.png>)`. Todo
  [01M7MP0N1Y3Z331VHFKSVRJMXQ](../../../todos/all-notes/01M7MP0N1Y3Z331VHFKSVRJMXQ-a-percent-encoded-local-file-path-is-not-decoded-in-html-and-pdf-output.md).
- **Link forms the regex cannot see.** Fable lab `haz`:

  | Form | HTML | PDF | Seen by a `LINK_RE`-style regex |
  | :--- | :--- | :--- | :--- |
  | `![a](assets/<ULID>/pic.png)` | copied | embedded | yes |
  | `![ref][r]` with `[r]: <ULID>/pic.png` | copied | embedded | no |
  | `![a](<assets/<ULID>/my pic.png>)` | copied | embedded (review lab, file beside the note) | no |
  | `![a](assets/<ULID>/my%20pic.png)` | warning, dead link | render fails | yes, as dangling |
  | `<img src="assets/<ULID>/pic.png">` | not copied, `src` kept as written | not applicable | no |

  Raw `<img>` is passed through un-exported by design:
  `docs/design/html-engine.md` records only local Markdown images and links
  as files to copy.
- **The `parse_name` hazard.** `parse_name` accepts a target that starts with
  a ULID, continues with `-` and ends in `.md`, slashes included
  (`src/link/mod.rs:239`). A link to `<ULID>-<slug>/transcript.md` is read as
  a link to the note. Lab (opus `v-hazard`): after a retitle, `reconcile`
  prints `relinked <ULID>-pdf-inside-only/transcript.md -> <ULID>-pdf-inside-only-v2.md`
  and the link to the transcript now points at the note. The renderers are
  hit before any retitle: the fable lab emitted
  `#link("hazard-note.pdf")[#notelink[Hazard Note]]` for such a link. A folder
  named by the bare ULID is not affected, since the rest after the ULID then
  starts with `/` and fails the check (opus `v-adj` and fable `haz` both:
  `relinked 0 links`, link intact). The `assets/` prefix of the `primary`
  convention avoids the bug by accident. Todo
  [01M7MNSS8YXMWC461EWYT9HFFD](../../../todos/all-notes/01M7MNSS8YXMWC461EWYT9HFFD-a-link-into-a-folder-named-like-a-note-is-read-as-a-link-to-that-note.md).
- **A `#` in a file name** cannot be linked
  (`docs/design/html-engine.md:40`, `skills/ntropy/references/writing-notes.md`).
- **Windows** is unsupported (ADR 0020).

## Disagreements between the researchers on facts

Settled against the code:

- Whether the `parse_name` hazard blocks a per-note folder directly in
  `all-notes/`. Opus first said so for any such folder. The code
  (`src/link/mod.rs:239`) and both labs show it holds for `<ULID>-<slug>/`
  only, not for `<ULID>/`. Opus conceded.
- Which missing HTML files get a dead link. Settled by the code path above:
  every missing in-vault path.
- Where a PDF existence check belongs. Opus first named the emitter, fable
  pointed out it is pure. The emitter signature rules it out. The engine and
  `prepare.rs` both have no-filesystem contracts, so the settled place is the
  typst engine with `ExportScope`, as the HTML engine does (see
  [options.md](options.md#technically-settled-points)).
- Whether the site's ULID exposure is new. Today's convention already
  publishes the ULID in file URLs (both labs).
- Whether "views unchanged" is shared by every option. Fable corrected it for
  bundles: a view that links the bundle directory keeps relative links
  working. Not tested in either lab, so this is stated as a property of
  symlinks, not as a result.
- Dendron's link form: see [external.md](external.md).
