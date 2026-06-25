// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pure text normalization: slugs and tags (ADR 0023).
//!
//! No I/O, no allocation beyond the strings produced. These rules are shared by
//! filenames, tag matching and view grouping, so they live in one place rather
//! than being re-derived per call site.

pub mod slug;
pub mod tag;
