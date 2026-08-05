// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Asking for a passphrase on the controlling terminal (ADRs 0036, 0041).
//!
//! The library declares that it needs a passphrase and the binary supplies one,
//! so this is where the one piece of terminal interaction in the key path
//! lives. Everything goes through `/dev/tty` rather than stdin and stdout: a
//! redirected pipe must never swallow the prompt or feed it an answer.

use std::io::{BufRead, BufReader, Write};

use ntropy::cipher::Passphrase;
use ntropy::keys::{KeyError, PassphrasePrompt};

use super::interact;

/// How many times a mismatched confirmation may be retried.
///
/// Bounded so a scripted or misbehaving terminal cannot spin forever.
const CONFIRM_ATTEMPTS: usize = 3;

/// A passphrase prompt on the controlling terminal.
#[derive(Debug, Clone, Copy)]
pub struct TtyPassphrasePrompt;

impl PassphrasePrompt for TtyPassphrasePrompt {
    fn existing(&self, label: &str) -> Result<Passphrase, KeyError> {
        ask(&format!("Passphrase for {label}: "))
    }

    fn new_passphrase(&self, label: &str) -> Result<Passphrase, KeyError> {
        // Asked twice because a mistyped passphrase on a freshly created vault
        // makes every note in it unreadable, and nothing downstream can tell
        // that happened.
        for _ in 0..CONFIRM_ATTEMPTS {
            let first = ask(&format!("New passphrase for {label}: "))?;
            let second = ask("Repeat passphrase: ")?;
            if first == second {
                return Ok(first);
            }
            let _ = writeln_tty("Passphrases did not match; try again.");
        }
        Err(KeyError::Locked)
    }
}

/// Prompt on the terminal with echo off and read one line.
fn ask(prompt: &str) -> Result<Passphrase, KeyError> {
    let tty = interact::open_tty().map_err(|_| KeyError::Locked)?;

    // Echo is restored by the guard's `Drop`, which also runs while a panic
    // unwinds. A signal that kills the process outright still leaves the
    // terminal with echo off; that is the residual gap of doing this in-process
    // rather than through a helper.
    let _echo = EchoOffScope::new(&tty)?;

    let mut out = &tty;
    out.write_all(prompt.as_bytes())
        .map_err(|_| KeyError::Locked)?;
    out.flush().map_err(|_| KeyError::Locked)?;

    let mut line = String::new();
    BufReader::new(&tty)
        .read_line(&mut line)
        .map_err(|_| KeyError::Locked)?;

    // The user's newline is not part of the passphrase; nothing else is
    // trimmed, since leading and interior whitespace are.
    let line = line
        .strip_suffix('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .unwrap_or(&line);

    // The terminal swallowed the echoed newline, so emit one to keep the next
    // line of output from landing beside the prompt.
    let _ = writeln_tty("");
    Ok(Passphrase::new(line))
}

/// Write a line to the terminal, ignoring failure.
///
/// Used only for advisory output around a prompt, where a write failure means
/// the terminal has gone away and the read that follows will fail anyway.
fn writeln_tty(message: &str) -> std::io::Result<()> {
    let tty = interact::open_tty()?;
    writeln!(&tty, "{message}")
}

/// Turns terminal echo off for as long as it is held.
struct EchoOffScope {
    fd: std::os::fd::RawFd,
    previous: libc::termios,
}

impl EchoOffScope {
    fn new(tty: &std::fs::File) -> Result<Self, KeyError> {
        use std::os::fd::AsRawFd;
        let fd = tty.as_raw_fd();

        // SAFETY: `termios` is a plain C struct that `tcgetattr` fully
        // initializes, and `fd` is a live descriptor for the terminal.
        let mut previous: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut previous) } != 0 {
            return Err(KeyError::Locked);
        }

        let mut quiet = previous;
        quiet.c_lflag &= !libc::ECHO;
        // SAFETY: `quiet` is the value just read, with one flag cleared.
        if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &quiet) } != 0 {
            return Err(KeyError::Locked);
        }

        Ok(Self { fd, previous })
    }
}

impl Drop for EchoOffScope {
    fn drop(&mut self) {
        // SAFETY: `self.previous` came from the paired `tcgetattr` above and
        // `self.fd` is still the terminal this scope was built from.
        unsafe {
            libc::tcsetattr(self.fd, libc::TCSAFLUSH, &self.previous);
        }
    }
}
