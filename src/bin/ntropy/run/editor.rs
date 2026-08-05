// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Launching the user's editor (ADR 0015, ADR 0036).
//!
//! The editor is `$VISUAL`, then `$EDITOR`; if neither is set, that is an
//! explicit error rather than a built-in fallback. The child talks to the
//! controlling terminal so a full-screen editor works even when ntropy's own
//! stdout or stdin is redirected, and ntropy waits for it to exit.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};

/// Build the command that opens `path`, without spawning anything.
///
/// Separated from the spawn so the argument list and the environment the child
/// inherits can be asserted without a terminal (ADR 0021).
pub fn build_command(editor: &OsStr, path: &Path, vault_hint: Option<&Path>) -> Command {
    let mut command = Command::new(editor);
    command.arg(path);
    if let Some(root) = vault_hint {
        // An encrypted vault's note is edited through a file outside the
        // vault, which resolves to no vault at all. The language server the
        // editor starts inherits this and uses it as the fallback, so
        // completion and navigation keep working (ADR 0041).
        command.env("NTROPY_VAULT_HINT", root);
    }
    command
}

/// Open `path` in the user's editor and wait for it to exit.
pub fn open(path: &Path, vault_hint: Option<&Path>) -> Result<()> {
    let editor = resolve_editor()?;
    let mut command = build_command(&editor, path, vault_hint);

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The child's environment overrides, as `(key, value)` pairs.
    fn env_of(command: &Command) -> Vec<(String, Option<String>)> {
        command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }

    #[test]
    fn the_note_path_is_the_only_argument() {
        let command = build_command(
            OsStr::new("vim"),
            Path::new("/v/all-notes/01ARZ-a.md"),
            None,
        );
        let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy()).collect();
        assert_eq!(args, ["/v/all-notes/01ARZ-a.md"]);
    }

    #[test]
    fn no_hint_means_no_environment_override() {
        let command = build_command(OsStr::new("vim"), Path::new("/v/note.md"), None);
        assert!(env_of(&command).is_empty());
    }

    #[test]
    fn a_hint_reaches_the_child() {
        // The language server the editor starts inherits this, and without it
        // a decrypted temp file resolves to no vault at all.
        let command = build_command(
            OsStr::new("vim"),
            Path::new("/run/user/1000/ntropy-x/01ARZ.md"),
            Some(Path::new("/v/vault")),
        );
        assert_eq!(
            env_of(&command),
            [(
                "NTROPY_VAULT_HINT".to_string(),
                Some("/v/vault".to_string())
            )]
        );
    }

    #[test]
    fn the_editor_is_the_program() {
        let command = build_command(OsStr::new("hx"), Path::new("/v/note.md"), None);
        assert_eq!(command.get_program(), OsStr::new("hx"));
    }
}
