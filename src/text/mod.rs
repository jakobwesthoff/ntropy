// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pure text rules: slug and tag normalization (ADR 0023), and the escaping
//! of text ntropy shows but did not write.
//!
//! No I/O, no allocation beyond the strings produced. These rules are shared by
//! filenames, tag matching, view grouping and every place that shows such
//! text, so they live in one place rather than being re-derived per call site.

pub mod escape;
pub mod slug;
pub mod tag;

pub use escape::escape_controls;
