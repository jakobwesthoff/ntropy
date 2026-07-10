// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The typst render engine: ntropy's own Markdown-to-Typst emitter
//! (ADR 0040, `docs/design/typst-engine.md`).
//!
//! The engine converts a note body to Typst markup and delegates only
//! typesetting to the `typst` binary; the compiler is not a crate
//! dependency. The emitted Typst is an intermediate representation whose
//! sole job is to render into a clean PDF, so the emitter prefers the
//! mechanically foolproof form (unconditional escaping, function calls)
//! over idiomatic hand-written Typst.
//!
//! Correctness rests on escaping: note text is arbitrary and Typst markup
//! assigns meaning to many plain characters. Every character the emitter
//! writes belongs to exactly one of three escaping contexts — markup text,
//! string literals, raw content — and the [`writer`] binds each context to
//! its own write method so the context is chosen lexically at the call
//! site, never through a mode flag.

pub mod document;
pub mod emitter;
pub mod prelude;
pub mod value;
pub mod writer;
