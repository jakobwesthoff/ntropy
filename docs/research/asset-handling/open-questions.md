# Open questions

What only the user can decide, merged from the two researchers' final
tables. Everything technical that they settled is in
[options.md](options.md#technically-settled-points), split into points that
hold for any naming and points settled for the bare ULID only. The questions are
ordered so that earlier answers narrow later ones. Question 1 leads because
the user's principle about readable structure bears on every layout.

Each question lists the options, what each changes, and the researchers'
recommendation where they gave one.

## 1. Folder naming

The user's principle: ntropy's structure is meant to be human readable and
navigable without ntropy, and a folder named only by a ULID breaks that,
because no correlation with the note can be established. The researchers'
converged recommendation (bare ULID) conflicts with it. They had treated
readability as a taste question. It is a constraint, so the readable
variants are the first-class options here. Details, costs and the 2026-11-02
incident for each are in
[options.md](options.md#readable-folder-names).

| Variant | Folder name | Rename drift (the incident) | Outside paths | Encrypted vault | Principle |
| :--- | :--- | :--- | :--- | :--- | :--- |
| a | `<ULID>-<slug>`, renamed by `reconcile` with the note, bare `<ULID>` in an encrypted vault (option A with the encrypted naming both researchers wrote into it) | Removed by automation, or kept until a rename command runs (rename on demand). A sync conflict between machines remains | Break on every retitle, or only when the rename command runs (rename on demand), which lists old and new paths | Bare `<ULID>`, links rewritten (enc-1) or link text unchanged with the folder found by ULID prefix (enc-2) | Met |
| b | `<ULID>-<slug at creation>`, never renamed | None, but the name is stale after a retitle | Stable | Needs bare `<ULID>` there | Met at creation, misleading after a retitle |
| c | Bare `<ULID>` plus a derived readable view of symlinks | None | Stable | No view there | Met by the view, not by the folder |
| d | `all-notes/<ULID>-<slug>/` beside the note, with the `parse_name` fix | As (a) | Break on every retitle | As (a) | Met |
| e | Readable folder plus a tracked symlink `assets/<ULID>` as the stable link address (lab-checked for HTML and PDF only) | Links never name the changing folder; the folder is stale until `reconcile` | Stable through the alias | Bare `<ULID>`, no alias | Folder met, link not |
| f | File-name prefix `<ULID>-<name>` instead of a folder, as in `primary`'s `images/` | None | Stable | Names leak | Not met beyond B |
| B | Bare `<ULID>` (the researchers' recommendation) | None | Stable | Same layout | Not met |

The researchers' recommendation, bare `<ULID>`, remains in options.md as the
option that scores best on drift, outside paths and encryption. It scores
worst on the stated principle, so no recommendation is given here that
ignores the principle.

Questions inside this one:

- Does the principle rule out bare `<ULID>` folders in plaintext vaults
  entirely, or only when nothing else (a derived view, `asset list`) gives the
  correlation?
- Does the user accept the drawbacks of renaming (outside paths break on
  every retitle, links from other notes need rewriting, none in `primary`
  today) in exchange for readability, as note files already do?
- If the folder renames: does `realign` (after `edit` and `write`) rename it
  and rewrite the note's own asset links at once, leave it to the next
  `reconcile`, or does `reconcile` only report drift and an explicit command
  renames (rename on demand)? Rewriting other notes' links from `realign` is a
  write pattern ntropy has nowhere today.
- In an encrypted vault, is a bare `<ULID>` folder acceptable, since the note
  there is `<ULID>.age`, and do the links keep the plaintext folder name (enc-2)
  or get rewritten (enc-1)?

## 2. Folder location

Applies to whichever naming is chosen. Every subdirectory of `all-notes/` is
reserved for resources by ADR 0019, so both `all-notes` locations are covered
by the scanner.

| Location | Bare `<ULID>` | `<ULID>-<slug>` |
| :--- | :--- | :--- |
| `all-notes/assets/<name>/` | Links `assets/<ULID>/x`. Both researchers recommend it. Migration of `primary` renames one path component inside one directory. `ls all-notes/` keeps showing notes. One pattern for git rules | As in `primary` today. The `assets/` prefix keeps the link from starting with a ULID, so `parse_name` does not see it |
| `all-notes/<name>/` | Links `<ULID>/x`. Sorts right above its note and links are shorter, but `all-notes/` gains one directory per note and the migration changes the link prefix. Opus asked that the user weigh it. `parse_name` does not block it | Needs the `parse_name` fix first. Sorts next to its note with the same name |
| Vault-root `assets/<name>/` | Keeps `all-notes/` notes only, links need `../`. No advantage found beyond taste | Same |

## 3. Encrypt assets at all in phase 2?

This gates questions 4 and 5. Options: encrypt them, or document that assets
stay plaintext. Where the folder name carries no slug in an encrypted vault,
the title leak of today's convention is gone, so "content of notes only,
assets documented as plaintext" is a possible stated limit (fable's question).
The todo [01KZ9ZN4GKF9XCT143E1W5YSEJ](../../../todos/all-notes/01KZ9ZN4GKF9XCT143E1W5YSEJ-protect-non-note-assets-in-encrypted-vaults.md)
lists the same fork. No researcher recommends the plaintext option, and opus
included encryption in its phased plan. This decision changes whether the
Typst staging root work (see the measurements below) is needed at all.

## 4. Are asset file names secret in an encrypted vault?

Relevant only if question 3 is yes.

- Secret: each asset becomes an opaque blob, tar inside age, with the name
  inside the ciphertext. The folder is unreadable without ntropy and age, name
  lookup needs the key, and a replaced diagram becomes an add plus a delete for
  sync. Against the principle: an encrypted vault's notes are already
  unreadable without ntropy or age, and this makes the asset folder equally so.
  Opus recommends it, fable agrees that it is the right scheme if names must be
  secret.
- Visible: `<name>.age`. Simpler, a link resolves with a path join, the folder
  is usable by hand, one more line in the threat model. Closer to the
  principle: a person can still read what is there. A name such as
  `2026-q3-layoffs-org-chart.png` leaks as much as a note title would (todo
  [01KZ9ZN4GKF9XCT143E1W5YSEJ](../../../todos/all-notes/01KZ9ZN4GKF9XCT143E1W5YSEJ-protect-non-note-assets-in-encrypted-vaults.md)).

The researchers recommend the secret scheme if names must be secret, and
state no preference for the user's answer. The recommendation predates the
principle and does not weigh it.

## 5. Accept per-note counts and sizes as a stated leak?

Relevant only if question 3 is yes. A per-note folder shows which notes have
attachments, how many, how large, and under the tar scheme when each was
attached. Options: accept it and add it to the "deliberately outside the
model" list of `docs/design/encryption.md`, or choose a layout that hides the
association (E blurs it, an archive per note hides the count and rewrites the
blob on every change). Both researchers recommend accepting.

## 6. Should `delete` remove the asset folder by default?

Technically settled: when another note references a file in the folder, the
note is deleted, the folder is kept and the referrers are named, and a
dedicated flag removes it anyway. Open: the default when nothing else
references it.

- Remove the folder with the note, and say so in the confirmation with the
  file count. This is what both researchers converged on.
- Keep the folder and say so, as `view remove` does for a removed view's
  directory today (`src/bin/ntropy/run/mod.rs:830-838`).

## 7. PDF and non-image files in the note's own folder

Today the PDF carries a relative `/URI` that works only beside `all-notes/`.
Options:

- Attach the file with `#pdf.attach`, which works in typst 0.15.1. Opus leans
  to it for files in the note's own folder, since a PDF sent by mail then
  carries them. Fable agrees with the lean and adds a condition: no silent
  attachment of large files, so a size threshold above which the file is
  listed in a warning, or an opt-out flag.
- Keep the relative link, dead outside `all-notes/`.
- Print the name as text.

Files elsewhere in the vault: the lead's technical recommendation is plain
text with a `note:` line that `--strict` ignores, see options.md.

## 8. Commands beyond `asset add` and `asset list`

Do you want `new --attach <file>`? Opus proposed it as an optional shortcut.
Fable first proposed `new --assets` (create the folder) and `asset dir`, then
agreed with opus's set including `new --attach` and dropped `asset dir` for
`asset list --path`. Command names are the user's to veto.

## 9. Links into another note's asset folder

May a note link another note's folder, with the owner protected by `delete`
and the check reporting cross-note references, or should a check push shared
files out to an unmanaged place in the vault, such as vault-root
`assets/<topic>/`? Both researchers lean to allowing it with the owner
protected, and to keeping vault-wide files unmanaged. This depends on the
first measurement below, which found no shared asset in `primary`.

## 10. A view-directory `assets` symlink

Fable withdrew it and recommends no. Opus first wanted views left alone, then
judged it technically feasible with a rule that renames a colliding group
directory, at the price of an extra entry in every view directory, and left it
to the user without a recommendation.

## Measurements

1. **Cross-note links in `primary`: done.** Result (details in
   [current-state.md](current-state.md#measured-in-the-primary-vault)): 80 links into
   `all-notes/assets/<basename>/` folders, all into the linking note's own
   folder, none into another note's, no folder linked by more than one note,
   and 16 links into `all-notes/images/<ULID>-<name>.png`, all to the linking
   note's own file. No asset is shared between notes in this vault, which
   removes the case for option E (first-class asset objects) as far as this
   vault goes. The conversion command must cover both conventions, the 25
   folders and the flat `images/` files.
2. **Typst staging root for encrypted assets.** Unprototyped. Encrypted
   renders need a compile root in the runtime directory that holds the
   decrypted assets, the document and the theme. Whether this breaks themes
   that read other vault files by a vault-absolute path is the open question of
   todo [01M66B32ZP0A6WRDY0QV26AHNX](../../../todos/all-notes/01M66B32ZP0A6WRDY0QV26AHNX-decide-how-much-of-the-vault-a-typst-theme-may-read.md).
   To be decided together with that todo. If themes need broad vault access,
   phase 2 gets harder, or PDF from encrypted vaults keeps reading decrypted
   assets from a staged root while the theme is copied in (fable).

## Also unverified

- Which editors resolve a relative link against a view symlink and which
  against its target.
- Whether editors that sanitize paths have trouble with a 26-character
  uppercase folder name. Fable knows of none.
- The cost of decrypting assets on every render, edit and LSP jump in a vault
  with large attachments, and the cost of one header decrypt per blob.
- Whether editors write `%20` for spaces when pasting images (fable, from
  memory).
