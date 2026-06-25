// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The crate-wide error type.
//!
//! Per ADR 0013 the library groups errors into semantically meaningful types
//! that each live with their own concern (`fsutil::FsError`, and later
//! `query::ParseError`, `scan::ScanError`, `config::ConfigError`,
//! `vault::ResolveError`). This module owns only the aggregating [`Error`]
//! enum that unifies them via `#[from]`, so callers can propagate any of them
//! through a single crate `Result` while still being able to match on the
//! specific variant.

use crate::datetime::DateError;
use crate::fsutil::FsError;
use crate::id::IdError;
use crate::note::NoteError;
use crate::scan::ScanError;

/// The unified error type returned across the library surface.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A filesystem primitive (write, symlink, rename, dir wipe) failed.
    #[error(transparent)]
    Fs(#[from] FsError),

    /// A string was not a valid note identity (ULID).
    #[error(transparent)]
    Id(#[from] IdError),

    /// A derived date could not be rendered.
    #[error(transparent)]
    Date(#[from] DateError),

    /// A note file was not well-formed.
    #[error(transparent)]
    Note(#[from] NoteError),

    /// A vault scan could not run.
    #[error(transparent)]
    Scan(#[from] ScanError),
}

/// Convenience alias for results carrying the crate [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
