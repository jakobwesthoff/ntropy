// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `ntropy` binary: a thin CLI/UI/process shell over the headless library.
//!
//! Top-level error handling uses `anyhow` (ADR 0013); the library's semantic
//! errors collapse to human-facing messages here. Cargo auto-discovers this
//! file as the `ntropy` binary because it lives at `src/bin/ntropy/main.rs`,
//! so there is no `[[bin]]` stanza and no `src/main.rs`.

use anyhow::Result;
use clap::{CommandFactory, Parser};

/// The full command surface is built out in a later phase. For now the binary
/// establishes the entrypoint and honors the "bare `ntropy` prints help"
/// contract (ADR 0018).
#[derive(Parser)]
#[command(
    name = "ntropy",
    version,
    about = "An opinionated Markdown note-taking and management CLI."
)]
struct Cli {}

fn main() -> Result<()> {
    let _cli = Cli::parse();

    // With no subcommands wired up yet, every successful parse means a bare
    // invocation, which prints help.
    Cli::command().print_help()?;
    println!();
    Ok(())
}
