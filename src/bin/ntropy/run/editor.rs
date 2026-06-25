// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Launching the user's editor (ADR 0015).
//!
//! The editor is `$VISUAL`, then `$EDITOR`; if neither is set, that is an
//! explicit error rather than a built-in fallback. The child inherits the
//! terminal so a full-screen editor works, and ntropy waits for it to exit.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};

/// Open `path` in the user's editor and wait for it to exit.
pub fn open(path: &Path) -> Result<()> {
    let editor = resolve_editor()?;
    let status = Command::new(&editor)
        .arg(path)
        .status()
        .with_context(|| format!("while launching editor `{}`", editor.to_string_lossy()))?;

    if !status.success() {
        bail!(
            "editor `{}` exited with a non-zero status",
            editor.to_string_lossy()
        );
    }
    Ok(())
}

/// Resolve the editor command from `$VISUAL` then `$EDITOR`.
fn resolve_editor() -> Result<OsString> {
    for var in ["VISUAL", "EDITOR"] {
        if let Some(value) = std::env::var_os(var)
            && !value.is_empty()
        {
            return Ok(value);
        }
    }
    Err(anyhow!(
        "no editor configured: set $VISUAL or $EDITOR to open notes"
    ))
}
