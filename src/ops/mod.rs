// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Use cases: one module per command, the only layer the binary calls.
//!
//! Each use case orchestrates the lower layers (scan, query, view, config,
//! template) into a complete operation and is headless: no terminal, picker or
//! editor. The binary's `run` layer supplies all of that around these.

pub mod create;
pub mod delete;
pub mod info;
pub mod init;
pub mod select;
pub mod tags;
pub mod view_admin;

pub use create::{TodayOutcome, create_note, today_note};
pub use delete::delete_note;
pub use info::{VaultStats, vault_stats};
pub use init::init_vault;
pub use select::{Candidate, Matches, resolve_selection, search, to_candidates};
pub use tags::{TagCount, list_tags};
pub use view_admin::{ViewAdminError, add_view, list_views, remove_view};
