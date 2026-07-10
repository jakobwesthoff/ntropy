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
