// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Launching the user's editor (ADR 0015, ADR 0036).
//!
//! The editor is `$VISUAL`, then `$EDITOR`; if neither is set, that is an
//! explicit error rather than a built-in fallback. The child talks to the
//! controlling terminal so a full-screen editor works even when ntropy's own
//! stdout or stdin is redirected, and ntropy waits for it to exit.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};

/// Open `path` in the user's editor and wait for it to exit.
pub fn open(path: &Path) -> Result<()> {
    let editor = resolve_editor()?;
    let mut command = Command::new(&editor);
    command.arg(path);

    // A full-screen editor needs the terminal on all three fds. Inherited fds
    // would hand it ntropy's redirected stdout in a pipeline like
    // `search -p | pbcopy` (vim then warns "Output is not to a terminal" and
    // corrupts the pipe), so each fd is bound to the controlling terminal
    // (ADR 0036). The editor only ever launches in interactive mode, which
    // guarantees that terminal exists.
    let tty = super::interact::open_tty().context("while opening the controlling terminal")?;
    command
        .stdin(Stdio::from(tty.try_clone().context(
            "while cloning the terminal handle for the editor's stdin",
        )?))
        .stdout(Stdio::from(tty.try_clone().context(
            "while cloning the terminal handle for the editor's stdout",
        )?))
        .stderr(Stdio::from(tty));

    let status = command
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
