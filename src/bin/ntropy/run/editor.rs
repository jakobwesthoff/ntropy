// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Launching the user's editor (ADR 0015, ADR 0036, ADR 0066).
//!
//! The editor is `$VISUAL`, then `$EDITOR`; if neither is set, that is an
//! explicit error rather than a built-in fallback. The value is shell code, run
//! through `/bin/sh`, so it may carry arguments such as `code -w`. The child
//! talks to the controlling terminal so a full-screen editor works even when
//! ntropy's own stdout or stdin is redirected, and ntropy waits for it to exit.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};

/// The shell every editor value runs through.
const SHELL: &str = "/bin/sh";

/// Build the command that opens `path`, without spawning anything.
///
/// Separated from the spawn so the argument list and the environment the child
/// inherits can be asserted without a terminal (ADR 0021).
pub fn build_command(editor: &OsStr, path: &Path, vault_hint: Option<&Path>) -> Command {
    // `sh -c '<value> "$@"' '<value>' <path>`: the value is used verbatim, so
    // quoting, `~` and `$VARIABLES` in it work as in the shell, while the path
    // arrives as a positional parameter and is never part of the shell string.
    // The value doubles as `$0`, which the shell names in its own errors.
    let mut script = editor.to_os_string();
    script.push(" \"$@\"");
    let mut command = Command::new(SHELL);
    command.arg("-c").arg(script).arg(editor).arg(path);
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
    choose_editor(std::env::var_os("VISUAL"), std::env::var_os("EDITOR"))
}

/// Pick the editor from the values of `$VISUAL` and `$EDITOR`.
///
/// A value that is empty or only whitespace counts as unset: run through the
/// shell, it would hand the note path to the shell as the command.
///
/// Takes the values rather than reading the environment, so the choice is
/// testable without mutating the process environment that parallel tests
/// share.
fn choose_editor(visual: Option<OsString>, editor: Option<OsString>) -> Result<OsString> {
    [visual, editor]
        .into_iter()
        .flatten()
        .find(|value| !value.as_encoded_bytes().iter().all(u8::is_ascii_whitespace))
        .ok_or_else(|| anyhow!("no editor configured: set $VISUAL or $EDITOR to open notes"))
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

    /// Run `command` and return what it wrote to stdout.
    ///
    /// These tests run real commands through `/bin/sh` without a terminal: the
    /// terminal binding happens in [`open`], not in [`build_command`].
    fn stdout_of(mut command: Command) -> Vec<u8> {
        let output = command.output().expect("the command starts");
        assert!(output.status.success(), "status: {}", output.status);
        output.stdout
    }

    #[test]
    fn the_value_runs_through_the_shell_with_the_path_as_a_positional_argument() {
        // `$0` is the value itself, so the shell names the setting in its own
        // error messages, such as `code -w: 1: code: not found`.
        let command = build_command(OsStr::new("code -w"), Path::new("/v/note.md"), None);
        let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy()).collect();
        assert_eq!(command.get_program(), OsStr::new("/bin/sh"));
        assert_eq!(args, ["-c", "code -w \"$@\"", "code -w", "/v/note.md"]);
    }

    #[test]
    fn a_value_with_arguments_passes_them_before_the_path() {
        // `printf '%s\0'` prints each argument it receives, NUL-terminated, so
        // stdout is the editor's argument list byte for byte.
        let command = build_command(
            OsStr::new(r"printf '%s\0' --wait"),
            Path::new("/v/note.md"),
            None,
        );
        assert_eq!(stdout_of(command), b"--wait\0/v/note.md\0");
    }

    #[test]
    fn the_note_path_reaches_the_editor_byte_for_byte() {
        // The path is a positional parameter, never part of the shell string,
        // so nothing in it is expanded, split or run.
        let path = r#"/v/it's a "note" $HOME `echo hi` ; | \ *.md"#;
        let command = build_command(OsStr::new(r"printf '%s\0'"), Path::new(path), None);
        assert_eq!(stdout_of(command), format!("{path}\0").as_bytes());
    }

    #[test]
    fn a_quoted_program_path_is_unquoted_by_the_shell() {
        let command = build_command(OsStr::new("'/bin/echo'"), Path::new("/v/note.md"), None);
        assert_eq!(stdout_of(command), b"/v/note.md\n");
    }

    #[test]
    fn a_variable_in_the_value_expands() {
        let mut command = build_command(
            OsStr::new("$NTROPY_TEST_EDITOR"),
            Path::new("/v/note.md"),
            None,
        );
        command.env("NTROPY_TEST_EDITOR", "/bin/echo");
        assert_eq!(stdout_of(command), b"/v/note.md\n");
    }

    #[test]
    fn a_tilde_in_the_value_expands() {
        let mut command = build_command(OsStr::new("~/echo"), Path::new("/v/note.md"), None);
        command.env("HOME", "/bin");
        assert_eq!(stdout_of(command), b"/v/note.md\n");
    }

    #[test]
    fn a_single_word_value_still_receives_the_path() {
        let command = build_command(OsStr::new("echo"), Path::new("/v/note.md"), None);
        assert_eq!(stdout_of(command), b"/v/note.md\n");
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

    fn os(value: &str) -> Option<OsString> {
        Some(OsString::from(value))
    }

    #[test]
    fn visual_wins_over_editor() {
        let chosen = choose_editor(os("hx"), os("vim")).expect("an editor is set");
        assert_eq!(chosen, "hx");
    }

    #[test]
    fn editor_is_used_when_visual_is_unset_or_empty() {
        assert_eq!(
            choose_editor(None, os("vim")).expect("EDITOR is set"),
            "vim"
        );
        assert_eq!(
            choose_editor(os(""), os("vim")).expect("EDITOR is set"),
            "vim"
        );
    }

    #[test]
    fn neither_set_is_an_error() {
        let error = choose_editor(None, os("")).expect_err("no editor is set");
        assert_eq!(
            error.to_string(),
            "no editor configured: set $VISUAL or $EDITOR to open notes"
        );
    }

    #[test]
    fn a_blank_value_counts_as_unset() {
        // Run through the shell, a blank value would hand the note path to the
        // shell as the command.
        assert_eq!(
            choose_editor(os(" \t"), os("vim")).expect("EDITOR is set"),
            "vim"
        );
        assert!(choose_editor(os(" "), os("\n")).is_err());
    }
}
