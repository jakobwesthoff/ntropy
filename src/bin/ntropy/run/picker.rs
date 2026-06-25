// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The interactive fuzzy picker (ADR 0014).
//!
//! Renders the library-produced candidate set with `nucleo-picker` and returns
//! the single selected note. Only rendering lives here; the candidate set and
//! its data come from `ops::select`, so the selection logic stays testable
//! without a TTY (ADR 0021). Each row shows the title, the local date and the
//! tags; the fuzzy matcher runs over that whole row (a v1 simplification of the
//! title+tags-only match noted in the design).

use std::borrow::Cow;

use anyhow::{Result, anyhow};
use ntropy::ops::Candidate;
use nucleo_picker::Picker;

/// Render a candidate as one picker row.
fn render_candidate(candidate: &Candidate) -> Cow<'_, str> {
    let row = if candidate.tags.is_empty() {
        format!("{}  ({})", candidate.title, candidate.date)
    } else {
        format!(
            "{}  ({})  [{}]",
            candidate.title,
            candidate.date,
            candidate.tags.join(", ")
        )
    };
    Cow::Owned(row)
}

/// Present `candidates` in the picker and return the chosen one.
///
/// Returns `Ok(None)` when there are no candidates or the user aborts the
/// picker without selecting.
pub fn pick(candidates: Vec<Candidate>) -> Result<Option<Candidate>> {
    if candidates.is_empty() {
        return Ok(None);
    }

    let mut picker = Picker::new(render_candidate as fn(&Candidate) -> Cow<'_, str>);
    let injector = picker.injector();
    for candidate in candidates {
        injector.push(candidate);
    }

    let selected = picker
        .pick()
        .map_err(|e| anyhow!("while running the picker: {e}"))?;
    Ok(selected.cloned())
}
