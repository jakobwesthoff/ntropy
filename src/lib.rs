// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! ntropy: an opinionated Markdown note-taking and management library.
//!
//! The crate is organized into layers, lowest (pure, no I/O) to highest
//! (orchestration), so dependencies only ever point downward:
//!
//! ```text
//! ops → {query, view, reconcile, config, scan, template, link}
//!     → {note, vault} → {fsutil, id, datetime, text}
//! ```
//!
//! [`error`] sits to the side, used by every layer. The library is headless:
//! it performs no terminal I/O, spawns no editor, and runs no picker. Those
//! concerns live in the binary (`src/bin/ntropy/`).

pub mod error;
pub mod fsutil;

pub mod datetime;
pub mod id;
pub mod text;

pub mod note;
pub mod vault;

pub mod config;
pub mod link;
pub mod query;
pub mod reconcile;
pub mod scan;
pub mod template;
pub mod view;

pub mod ops;

/// Common re-exports for callers of the library.
pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::id::Id;
    pub use crate::note::Note;
}
