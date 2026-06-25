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
