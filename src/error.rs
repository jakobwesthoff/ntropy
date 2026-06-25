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

use crate::fsutil::FsError;

/// The unified error type returned across the library surface.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A filesystem primitive (write, symlink, rename, dir wipe) failed.
    #[error(transparent)]
    Fs(#[from] FsError),
}

/// Convenience alias for results carrying the crate [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
