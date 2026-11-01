// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `show`: print one note drawn for the terminal (ADR 0083,
//! `docs/design/cli.md`).

use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal, Write};
use std::process::{Command, ExitCode, Stdio};

use anyhow::{Context, Result, bail};

use ntropy::render::ansi::{self, AnsiOptions, DEFAULT_WIDTH, Form};
use ntropy::session::VaultSession;

use crate::cli::GlobalArgs;

use super::exit_for_warnings;
use super::one_note::Selection;
use super::signals::{SigintCaughtScope, SigpipeIgnoredScope};

/// The shell a `$PAGER` value runs through.
const SHELL: &str = "/bin/sh";

/// Print the note named by `selector`, drawn for the terminal.
///
/// Selection is `render`'s: a blank selector browses every note, several
/// matches open the picker and are an error under `-n` (ADR 0037). The note
/// is drawn from the scan's copy, decrypted in memory for an encrypted vault,
/// so nothing is written to disk.
pub fn cmd_show(
    global: &GlobalArgs,
    session: &VaultSession,
    selector: String,
    width_flag: Option<u16>,
    interactive: bool,
) -> Result<ExitCode> {
    let (selection, warnings) =
        super::one_note::select_one(session, &selector, interactive, false, "showing")?;
    let note = match selection {
        Selection::Note(note) => note,
        Selection::Exit(code) => return Ok(code),
    };

    // The terminal that matters is the one stdout is, as that is where the
    // drawing lands; its size is not asked for when stdout is not one.
    let is_terminal = io::stdout().is_terminal();
    let size = is_terminal.then(stdout_size).flatten();
    let options = AnsiOptions {
        width: width(width_flag, size.map(|size| size.columns)),
        form: form(is_terminal, std::env::var_os("NO_COLOR").as_deref()),
    };
    // A parsed note's header is its fenced block, so the split always finds
    // one; the empty fallback only keeps this infallible.
    let frontmatter = ntropy::note::frontmatter::split(&note.raw_header)
        .frontmatter
        .unwrap_or_default();
    let drawn = ansi::draw_source(frontmatter, &note.body, &options);

    let rows_needed = size.map_or(0, |size| drawn.rows(size.columns));
    let mut stdout = io::stdout().lock();
    if should_page(interactive, size.map(|size| size.rows), rows_needed) {
        let pager = choose_pager(std::env::var_os("PAGER"));
        let command = pager_command(
            &pager,
            std::env::var_os("LESS").as_deref(),
            session.is_encrypted(),
        );
        page(&drawn.text, command, &pager, &mut stdout)?;
    } else {
        stdout
            .write_all(drawn.text.as_bytes())
            .and_then(|()| stdout.flush())
            .context("while writing the note")?;
    }

    Ok(exit_for_warnings(global.strict, &warnings, 0))
}

/// The size of the terminal stdout is.
#[derive(Debug, Clone, Copy)]
struct TerminalSize {
    columns: usize,
    rows: usize,
}

/// The size of the terminal on stdout, as the terminal reports it; zero where
/// it reports none.
fn stdout_size() -> Option<TerminalSize> {
    // SAFETY: a zeroed `winsize` is a valid out-parameter for `TIOCGWINSZ`,
    // which only writes into it.
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: stdout is an open descriptor and `size` outlives the call.
    // `TIOCGWINSZ` is a `c_ulong` on macOS and an `Ioctl` on Linux; `as _`
    // fits both.
    let rc = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ as _, &mut size) };
    (rc == 0).then(|| TerminalSize {
        columns: size.ws_col.into(),
        rows: size.ws_row.into(),
    })
}

/// Which escape sequences the drawing carries: none where stdout is not a
/// terminal, and no colours where `NO_COLOR` is set and not empty, which asks
/// for no colour and leaves bold, italic and underline alone
/// (<https://no-color.org>).
fn form(is_terminal: bool, no_color: Option<&OsStr>) -> Form {
    if !is_terminal {
        Form::Plain
    } else if no_color.is_some_and(|value| !value.is_empty()) {
        Form::Styled
    } else {
        Form::Colored
    }
}

/// The width the note is laid out at: `--width`, else the columns of the
/// terminal stdout is, else the default. A terminal that reports no columns
/// tells nothing.
fn width(flag: Option<u16>, columns: Option<usize>) -> usize {
    flag.map(usize::from)
        .or(columns.filter(|&columns| columns > 0))
        .unwrap_or(DEFAULT_WIDTH)
}

/// Whether the drawing goes through a pager: only interactively (ADR 0036),
/// only to a terminal that reports its rows, and only when the drawing needs
/// as many rows as the terminal has. At one row fewer, the prompt that
/// follows still fits beneath it.
fn should_page(interactive: bool, terminal_rows: Option<usize>, rows_needed: usize) -> bool {
    interactive && terminal_rows.is_some_and(|rows| rows > 0 && rows_needed >= rows)
}

/// The pager the drawing goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pager {
    /// A `$PAGER` value, shell code run through `/bin/sh` as the editor value
    /// is (ADR 0066).
    Shell(OsString),
    /// `less -R` from `PATH`, when `$PAGER` says nothing.
    Less,
}

/// The pager for the value of `$PAGER`. A value that is empty or only
/// whitespace counts as unset: run through the shell, it would start nothing.
fn choose_pager(pager: Option<OsString>) -> Pager {
    match pager {
        Some(value) if !value.as_encoded_bytes().iter().all(u8::is_ascii_whitespace) => {
            Pager::Shell(value)
        }
        _ => Pager::Less,
    }
}

/// The command that runs `pager`, given the value of `$LESS` and whether the
/// note comes from an encrypted vault.
///
/// `LESS=R`, set only where `$LESS` is unset, lets a `$PAGER` of a bare
/// `less` show the styles; a `$LESS` of the user's own wins. For an encrypted
/// vault, `LESSSECURE=1` keeps less from saving the decrypted note, editing
/// it or piping it to a command. It constrains less alone; another pager gets
/// the plaintext as any pager would.
fn pager_command(pager: &Pager, less: Option<&OsStr>, encrypted: bool) -> Command {
    let mut command = match pager {
        // `sh -c '<value>' '<value>'`: the value doubles as `$0`, which the
        // shell names in its own errors.
        Pager::Shell(value) => {
            let mut command = Command::new(SHELL);
            command.arg("-c").arg(value).arg(value);
            command
        }
        Pager::Less => {
            let mut command = Command::new("less");
            command.arg("-R");
            command
        }
    };
    if less.is_none() {
        command.env("LESS", "R");
    }
    if encrypted {
        command.env("LESSSECURE", "1");
    }
    command
}

/// Write `text` into the pager's pipe. A pager that quits before it has read
/// everything closes the pipe, and the write fails with a broken pipe: the
/// person has seen what they wanted, so that is no error.
fn feed(text: &[u8], pipe: &mut impl Write) -> io::Result<()> {
    match pipe.write_all(text) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

/// Show `text` through the pager `command` runs, and wait for it.
///
/// A pager that cannot be started at all leaves the text to `fallback`,
/// stdout in production: the note is shown either way. Only a `$PAGER` value
/// the shell could not run (status 126 or 127) is an error; any other status
/// is the pager's own business, such as less `-K` quitting with 2 on Ctrl-C
/// after the note was read.
fn page(text: &str, mut command: Command, pager: &Pager, fallback: &mut impl Write) -> Result<()> {
    command.stdin(Stdio::piped());
    // Installed before the spawn, so no Ctrl-C between the two can end
    // ntropy while the pager already draws.
    let interrupts = SigintCaughtScope::new();
    let Ok(mut child) = command.spawn() else {
        drop(interrupts);
        fallback
            .write_all(text.as_bytes())
            .and_then(|()| fallback.flush())
            .context("while writing the note")?;
        return Ok(());
    };

    // The pipe is closed and the pager waited for whatever the write came to:
    // a `Child` is neither waited for nor killed when dropped, and ntropy
    // ending first would hand the terminal back while the pager still draws.
    let fed = {
        let _sigpipe = SigpipeIgnoredScope::new();
        let mut stdin = child.stdin.take().expect("stdin was configured as a pipe");
        feed(text.as_bytes(), &mut stdin)
    };
    let status = child.wait().context("while waiting for the pager")?;
    drop(interrupts);
    fed.context("while writing to the pager")?;

    if let Pager::Shell(value) = pager
        && matches!(status.code(), Some(126 | 127))
    {
        bail!("pager `{}` could not be run", value.to_string_lossy());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::signals::SIGINT_TESTS;
    use super::*;

    // =========================================================
    // Styles
    // =========================================================

    #[test]
    fn a_terminal_gets_every_style() {
        assert_eq!(form(true, None), Form::Colored);
    }

    #[test]
    fn output_that_is_not_a_terminal_gets_the_text_alone() {
        assert_eq!(form(false, None), Form::Plain);
        assert_eq!(form(false, Some(OsStr::new("1"))), Form::Plain);
    }

    #[test]
    fn no_color_takes_the_colours_and_leaves_the_styles() {
        assert_eq!(form(true, Some(OsStr::new("1"))), Form::Styled);
    }

    #[test]
    fn an_empty_no_color_counts_as_unset() {
        assert_eq!(form(true, Some(OsStr::new(""))), Form::Colored);
    }

    // =========================================================
    // Width and paging
    // =========================================================

    #[test]
    fn the_width_flag_wins_over_the_terminal() {
        assert_eq!(width(Some(30), Some(120)), 30);
    }

    #[test]
    fn the_terminal_gives_the_width_without_a_flag() {
        assert_eq!(width(None, Some(120)), 120);
    }

    #[test]
    fn the_default_width_applies_without_flag_or_terminal_columns() {
        assert_eq!(width(None, None), DEFAULT_WIDTH);
        assert_eq!(width(None, Some(0)), DEFAULT_WIDTH);
    }

    #[test]
    fn a_drawing_that_fills_the_screen_is_paged() {
        assert!(should_page(true, Some(24), 24));
        assert!(should_page(true, Some(24), 100));
    }

    #[test]
    fn a_drawing_shorter_than_the_screen_is_not_paged() {
        // The prompt that follows 23 lines still fits on a 24-row screen.
        assert!(!should_page(true, Some(24), 23));
    }

    #[test]
    fn nothing_is_paged_without_interaction_a_terminal_or_its_rows() {
        assert!(!should_page(false, Some(24), 100));
        assert!(!should_page(true, None, 100));
        assert!(!should_page(true, Some(0), 100));
    }

    // =========================================================
    // The pager
    // =========================================================

    #[test]
    fn pager_is_run_through_the_shell() {
        assert_eq!(
            choose_pager(Some(OsString::from("less -FRX"))),
            Pager::Shell(OsString::from("less -FRX"))
        );
    }

    #[test]
    fn an_unset_or_blank_pager_is_less() {
        assert_eq!(choose_pager(None), Pager::Less);
        assert_eq!(choose_pager(Some(OsString::new())), Pager::Less);
        assert_eq!(choose_pager(Some(OsString::from(" \t"))), Pager::Less);
    }

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

    fn args_of(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_shell_pager_runs_as_sh_c_with_the_value_as_its_name() {
        let command = pager_command(
            &Pager::Shell(OsString::from("most -s")),
            Some(OsStr::new("")),
            false,
        );
        assert_eq!(command.get_program(), "/bin/sh");
        assert_eq!(args_of(&command), ["-c", "most -s", "most -s"]);
        assert!(env_of(&command).is_empty());
    }

    #[test]
    fn less_runs_with_raw_control_characters() {
        let command = pager_command(&Pager::Less, Some(OsStr::new("X")), false);
        assert_eq!(command.get_program(), "less");
        assert_eq!(args_of(&command), ["-R"]);
        assert!(env_of(&command).is_empty());
    }

    #[test]
    fn an_unset_less_variable_is_set_to_r() {
        let command = pager_command(&Pager::Shell(OsString::from("less")), None, false);
        assert_eq!(
            env_of(&command),
            [("LESS".to_string(), Some("R".to_string()))]
        );
    }

    #[test]
    fn an_encrypted_vault_puts_less_in_its_secure_mode() {
        let command = pager_command(&Pager::Less, Some(OsStr::new("X")), true);
        assert_eq!(
            env_of(&command),
            [("LESSSECURE".to_string(), Some("1".to_string()))]
        );
    }

    /// A writer whose every write fails with `kind`.
    struct Failing(io::ErrorKind);

    impl Write for Failing {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(self.0))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_pager_that_stops_reading_is_no_error() {
        assert!(feed(b"text", &mut Failing(io::ErrorKind::BrokenPipe)).is_ok());
    }

    #[test]
    fn any_other_write_error_is_one() {
        let err = feed(b"text", &mut Failing(io::ErrorKind::Other)).expect_err("fails");
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    fn sh(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(script);
        command
    }

    #[test]
    fn the_pager_receives_the_text_on_its_stdin() {
        let _serial = SIGINT_TESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempfile::tempdir().expect("temp dir");
        let record = dir.path().join("record");
        let script = format!("cat > '{}'", record.display());
        let mut fallback = Vec::new();
        page(
            "drawn\n",
            sh(&script),
            &Pager::Shell(script.clone().into()),
            &mut fallback,
        )
        .expect("the pager runs");
        assert_eq!(std::fs::read_to_string(&record).expect("record"), "drawn\n");
        assert!(fallback.is_empty());
    }

    #[test]
    fn a_pager_quitting_before_reading_everything_is_no_error() {
        let _serial = SIGINT_TESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let text = "x".repeat(1 << 20);
        let mut fallback = Vec::new();
        page(
            &text,
            sh("exit 0"),
            &Pager::Shell("exit 0".into()),
            &mut fallback,
        )
        .expect("an early quit is fine");
    }

    #[test]
    fn a_pager_the_shell_cannot_run_is_an_error() {
        let _serial = SIGINT_TESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut fallback = Vec::new();
        let err = page(
            "x",
            sh("exit 127"),
            &Pager::Shell("no-such-pager".into()),
            &mut fallback,
        )
        .expect_err("status 127 means the shell found no such program");
        assert_eq!(err.to_string(), "pager `no-such-pager` could not be run");
    }

    #[test]
    fn any_other_pager_status_is_ignored() {
        let _serial = SIGINT_TESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // less -K quits with status 2 on Ctrl-C after the note was shown.
        let mut fallback = Vec::new();
        page(
            "x",
            sh("cat >/dev/null; exit 2"),
            &Pager::Shell("less -K".into()),
            &mut fallback,
        )
        .expect("a pager's own status is not ntropy's");
    }

    #[test]
    fn a_pager_that_cannot_start_leaves_the_text_to_stdout() {
        let _serial = SIGINT_TESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut fallback = Vec::new();
        page(
            "drawn\n",
            Command::new("ntropy-test-no-such-pager"),
            &Pager::Less,
            &mut fallback,
        )
        .expect("the text is printed instead");
        assert_eq!(fallback, b"drawn\n");
    }
}
