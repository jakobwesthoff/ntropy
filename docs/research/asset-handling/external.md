# How other tools handle attachments

Only what the two proposals source. Nothing here was re-fetched when this
record was written. Opus cited a URL for most entries, fable wrote its list
from memory and named the URL where it would check. Items marked "from
memory" were not backed by a fetched page in either proposal.

| Tool | What it does | Source and status |
| :--- | :--- | :--- |
| Obsidian | A setting "Default location for new attachments": vault root (the default), a fixed folder, the same folder as the current file, or a subfolder under the current folder. Attachments are ordinary vault files with no per-note owner. Deleting a note leaves its attachments. Links are updated on rename only by the app, through a setting "Automatically update internal links". | Location options: [Obsidian help, Attachments](https://obsidian.md/help/attachments), cited by opus. The link-update setting is from memory (opus). No-owner and orphan statements are from memory (both). |
| Logseq | One `assets/` folder per graph, pages in `pages/`, links `../assets/<file>`. Moving files means fixing references by hand. Files outside `assets/` are not exported. Files get a timestamp suffix. | [Forum on attachments](https://discuss.logseq.com/t/understanding-the-proper-way-to-handle-attachements-assets/8910) and [export request](https://discuss.logseq.com/t/images-that-are-not-in-the-assets-dir-should-also-be-exported/14229), cited by opus. The timestamp suffix is from memory (fable). |
| Zettlr | A "Default image folder" setting, default `assets`, relative to the Markdown file. An absolute folder puts everything in one place. | [Zettlr settings](https://docs.zettlr.com/en/reference/settings/), cited by opus. Fable grouped Zettlr and Foam from memory as "plain relative paths, a configurable default image folder, nothing managed". |
| Dendron | Vault `assets/images/`, standard Markdown links. Paste-image writes to `${currentFileDir}/assets/images`. | [Dendron wiki](https://wiki.dendron.so/notes/a91fd8da-6895-49fe-8164-a17acd8d9a17), cited by opus. Fable's version from memory said links are root-absolute and only Dendron's preview resolves the slash. Fable conceded the point to opus's source. |
| Joplin | Attachments are "resources", stored under their ID as file name and linked `:/<32 hex>`. End-to-end encryption covers resources. The note no longer renders in any other Markdown tool. | [Attachments](https://joplinapp.org/help/apps/attachments), [forum on storage](https://discourse.joplinapp.org/t/handling-markdown-images/1509), [E2EE](https://joplinapp.org/help/apps/sync/e2ee), cited by opus. Fable adds from memory (URL joplinapp.org/help/dev/spec/e2ee) that a service deletes resources no note references after a grace period. Opus called that plausible and unverified. |
| Bear and TextBundle | A note exports as a folder `Name.textbundle/` with `text.md`, `info.json` and `assets/`, linked `assets/image.png`. | [Bear forum](https://community.bear.app/t/export-as-textbundle-or-bear-missing-assets-as-path-in-image-links/12316), [Bear export FAQ](https://bear.app/faq/export-your-notes/), cited by opus. Fable named textbundle.org/spec from memory. |
| Hugo page bundles | A leaf bundle is a directory with `index.md`. Every other file in it is a page resource published beside the page. | [Hugo page bundles](https://gohugo.io/content-management/page-bundles/), cited by opus. |
| git-annex and Git LFS | Content addressing at the storage layer: annex keys like `SHA256E-s<size>--<hash>.<ext>` with symlinks in the working tree, LFS pointer files with `oid sha256:` and `size`. | [git-annex key format](https://git-annex.branchable.com/internals/key_format/), [Git LFS spec](https://github.com/git-lfs/git-lfs/blob/main/docs/spec.md), cited by opus as "from my knowledge". |

## What the researchers took from it

Both reached the same reading, as their own synthesis and not a sourced
claim: tools that stay plain Markdown (Obsidian, Logseq, Zettlr, Foam) keep
plain relative links and do not manage the lifecycle of attachments. The tool
that manages lifecycle and encryption (Joplin) gave up plain Markdown. The
bundle formats (Hugo, TextBundle) get lifecycle from the directory but make
the note a directory. Opus found no tool that gives assets an owner and keeps
plain links. Fable pointed to ntropy's own note-link design (ADR 0028) as a
third route: a real relative path whose identity is a ULID, kept right by
`reconcile`.
