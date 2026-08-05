// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! ntropy: an opinionated Markdown note-taking and management library.
//!
//! The crate is organized into layers, lowest (pure, no I/O) to highest
//! (orchestration), so dependencies only ever point downward:
//!
//! ```text
//! ops → {query, view, reconcile, config, scan, template, link, gitignore, render}
//!     → {note, vault, cipher} → {crypto, fsutil, id, datetime, text}
//! ```
//!
//! [`render`] sits alongside [`query`] and [`link`], depending on [`note`],
//! [`link`], [`datetime`], and [`id`].
//!
//! [`cipher`] is the seam every note read and write passes through, so vaults
//! that store their notes encrypted and vaults that store them as Markdown
//! share one code path (ADR 0041). It and [`crypto`] behind it are the only
//! modules that know encryption exists; [`crypto`] is compiled only with the
//! `encryption` feature.
//!
//! [`keys`] sits beside them and answers a different question: not how a note
//! is encrypted, but where this machine keeps the key that opens it.
//!
//! [`error`] sits to the side, used by every layer. The library is headless:
//! it performs no terminal I/O, spawns no editor, and runs no picker. Those
//! concerns live in the binary (`src/bin/ntropy/`).

pub mod error;
pub(crate) mod fsutil;

pub mod datetime;
pub mod id;
pub mod text;

pub mod cipher;
#[cfg(feature = "encryption")]
pub mod crypto;
pub mod keys;

pub mod note;
pub mod session;
pub mod vault;

pub mod config;
pub mod gitignore;
#[cfg(feature = "encryption")]
pub mod migrate;

pub mod link;
pub mod query;
pub mod reconcile;
pub mod render;
pub mod scan;
pub mod template;
pub mod view;

pub mod ops;

#[cfg(test)]
mod test_support;

/// Common re-exports for callers of the library.
pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::id::Id;
    pub use crate::note::Note;
}
