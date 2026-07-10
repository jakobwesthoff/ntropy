// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The content a fresh vault is seeded with (ADR 0039).
//!
//! [`layout`](super::layout) names a vault's well-known files; this module
//! holds what goes inside them. Each one lives on disk under `src/vault/seed/`
//! in a tree shaped like the vault it produces, and reaches the binary through
//! [`include_str!`]. Markdown stays Markdown, and the tree shows byte-for-byte
//! what `init` writes.
//!
//! That tree spells the config directory `ntropy/` rather than `.ntropy/`: a
//! literal dot-directory would satisfy [`layout::is_vault`](super::layout::is_vault),
//! making `src/vault/seed/` resolve as a vault to ntropy's own path lookup.
//!
//! The seed files carry no MPL header, the one exception in this repository.
//! They are copied verbatim into user vaults, where a license comment atop
//! someone's note template would be noise.

/// The root `README.md` seeded into a vault, so someone who discovers the
/// directory without knowing ntropy can identify it and install the tooling.
pub const VAULT_README: &str = include_str!("seed/README.md");

/// The built-in default template.
///
/// Frontmatter carries the required `title` and an empty `tags` list; the body
/// is a single heading echoing the title.
pub const DEFAULT_TEMPLATE: &str = include_str!("seed/ntropy/templates/default.md");

/// The built-in `today` template.
///
/// The daily note is titled by its date and carries a `daily` tag; the `today`
/// command finds an existing note with today's date as title before creating one.
pub const TODAY_TEMPLATE: &str = include_str!("seed/ntropy/templates/today.md");

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the header exemption: these files land in user vaults verbatim,
    /// so a license comment added by reflex would ship with every new vault.
    #[test]
    fn seed_files_carry_no_license_header() {
        for content in [VAULT_README, DEFAULT_TEMPLATE, TODAY_TEMPLATE] {
            assert!(!content.contains("Mozilla Public"), "got: {content}");
        }
    }
}
