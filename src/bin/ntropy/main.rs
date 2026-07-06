// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `ntropy` binary: a thin CLI/UI/process shell over the headless library.
//!
//! Top-level error handling uses `anyhow` (ADR 0013): the library's semantic
//! errors collapse to one human-facing message printed to stderr, and the
//! process exits non-zero. Cargo auto-discovers this file as the `ntropy`
//! binary because it lives at `src/bin/ntropy/main.rs`.

mod cli;
mod run;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;

fn main() -> ExitCode {
    // The Rust runtime sets `SIGPIPE` to `SIG_IGN` before `main` runs, so a
    // write to a stdout pipe with no reader left (e.g. piping into `head`)
    // returns `EPIPE` as an ordinary I/O error instead of killing the
    // process. `println!` and friends treat that error as fatal and panic,
    // which is not the Unix CLI convention: `grep`, `cat`, and friends die
    // quietly with status 141 (128 + `SIGPIPE`) instead. Restoring the
    // default disposition here makes ntropy behave the same way.
    #[cfg(unix)]
    // SAFETY: `SIGPIPE` and `SIG_DFL` are valid signal/handler constants, and
    // installing a handler at startup, before any other thread exists, has no
    // preconditions beyond that.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cli = Cli::parse();
    match run::run(cli) {
        Ok(code) => code,
        Err(error) => {
            // `{:#}` renders the full anyhow context chain on one line.
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
