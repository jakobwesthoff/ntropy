// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The HTML engine: the HTML output of the shared Markdown walk
//! (ADR 0049, `docs/design/html-engine.md`).
//!
//! The emitter turns a note body into an HTML body fragment whose markup a
//! stylesheet can address. Correctness rests on escaping: every character the
//! emitter writes belongs to exactly one of three contexts, text content,
//! attribute values, and raw pass-through, and the [`writer`] binds each
//! context to its own write method so the context is chosen lexically at the
//! call site, never through a mode flag.

pub mod emitter;
pub mod writer;

pub use emitter::{Emitted, SiblingArtifacts, Targets, emit};
