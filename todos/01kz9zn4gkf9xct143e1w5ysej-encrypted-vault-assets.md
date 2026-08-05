# Non-note assets in encrypted vaults

Notes reference images, PDFs and other attachments as ordinary relative
Markdown links. Those files are not notes, and vault encryption
([docs/design/encryption.md](../docs/design/encryption.md)) does not cover
them: it encrypts `all-notes/<ulid>.md` into `<ulid>.age` and says nothing
about anything else in the directory.

The scanner treats non-note files as resources and leaves them alone. It walks
`all-notes/` at depth 1 only, keeps files whose extension matches the vault's
note extension, and silently ignores everything else; subdirectories are never
descended into.

So in an encrypted vault every asset stays plaintext in the synced directory.
Its filename, size and contents all reach whoever stores or syncs the vault,
which is the party the threat model exists to defend against. An image named
`2026-q3-layoffs-org-chart.png` leaks as much as a note title would.

## What needs discussing

- Whether assets are encrypted at all, or stay plaintext with the leak stated
  explicitly in the threat model's "deliberately outside the model" list.
- How a note's link resolves if the asset becomes `<name>.age`. A body written
  as `![Chart](chart.png)` no longer points at a real file, and unlike
  note-to-note links there is no ULID to resolve by.
- How rendering materializes them. The typst engine runs with its working
  directory set to the note's own directory so relative assets resolve, which
  means it expects real files sitting beside the note. Staging decrypted copies
  into the render staging directory would need the link targets rewritten to
  match.
- How an editor previews an encrypted image while editing a decrypted note in
  the runtime directory, where the asset is neither present nor readable.
- Whether subdirectories change anything, given the scanner does not descend
  into them but users do put asset folders there.

## Directions to weigh

- **Encrypt in place** as `<name>.age`, decrypt into the render staging
  directory on demand, rewrite link targets during staging. Closes the leak;
  costs a resolution rule for non-note files and breaks plain-Markdown
  previewing entirely.
- **Leave assets plaintext**, document the leak, and let users keep sensitive
  attachments out of the vault. Zero work; the threat model gains an honest
  and fairly large hole.
- **Give assets their own area** with explicit handling, so the rules are
  stated rather than inherited from the resource-file convention.
