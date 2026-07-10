# ntropy

An opinionated Markdown note-taking and management CLI. Architecture decisions
live in `docs/adr/`; narrative design docs in `docs/design/`.

## Licensing

ntropy is licensed under the **Mozilla Public License 2.0** (`LICENSE`).

**Every source file must begin with the MPL-2.0 header comment.** For Rust
files, use line comments:

```rust
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
```

Adapt the comment syntax to the file's language, keeping the wording verbatim.

The one exception is the vault seed content under `src/vault/seed/` (ADR 0039).
Those files are copied verbatim into user vaults, so they carry no header. A
unit test in `vault::seed` enforces this.

## Documentation conventions

- ADRs (`docs/adr/`) are precise and brief: no filler written to satisfy
  the template. Context in a few sentences, consequences only for
  genuine, validated trade-offs.
- Design docs (`docs/design/`) describe the lasting design and stay
  timeless: no transition narration, no deferred-work notes, no
  references to todos. Deferred and future work lives in `todos/`
  (one `<ulid>-slug.md` file per topic, linking back to the design
  where useful); todo files are deleted once completed.
- Keep documentation current: when a decision lands or code changes make
  a doc stale, update the affected design docs and todos in the same
  unit of work, not afterwards.
- README.md links into the repository must be absolute GitHub URLs
  (`https://github.com/jakobwesthoff/ntropy/blob/main/...`); relative
  paths break the generated project page. In-page anchors stay as-is.

## Test conventions

- Fixtures that stand for nonexistent things (unknown engines, missing
  formats, absent directories) use obviously fake names like
  `no-such-engine`, never plausible real-world names. Realistic names
  are for fixtures representing real things.
