// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Use cases: one module per command, the only layer the binary calls.
//!
//! Each use case orchestrates the lower layers (scan, query, view, config,
//! template) into a complete operation and is headless: no terminal, picker or
//! editor. The binary's `run` layer supplies all of that around these.

pub mod create;

pub use create::create_note;
