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

use crate::config::ConfigError;
use crate::datetime::DateError;
use crate::fsutil::FsError;
use crate::id::IdError;
use crate::note::NoteError;
use crate::ops::ViewAdminError;
use crate::query::QueryError;
use crate::render::RenderError;
use crate::scan::ScanError;
use crate::template::TemplateError;
use crate::vault::ResolveError;

/// The unified error type returned across the library surface.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A filesystem primitive (write, symlink, rename, directory read/removal) failed.
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

    /// A query could not be parsed or compiled.
    #[error(transparent)]
    Query(#[from] QueryError),

    /// A configuration file could not be read, parsed or written.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// A vault could not be resolved.
    #[error(transparent)]
    Resolve(#[from] ResolveError),

    /// A template file could not be read.
    #[error(transparent)]
    Template(#[from] TemplateError),

    /// A view administration command was rejected.
    #[error(transparent)]
    ViewAdmin(#[from] ViewAdminError),

    /// A note could not be rendered to an output artifact.
    #[error(transparent)]
    Render(#[from] RenderError),
}

/// Convenience alias for results carrying the crate [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
