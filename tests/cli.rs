// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! CLI contract tests: run the real binary and snapshot its stdout, stderr and
//! exit code with `insta-cmd` (ADR 0021).

use insta_cmd::assert_cmd_snapshot;
use std::process::Command;

/// The path to the freshly built `ntropy` binary, provided by Cargo to
/// integration tests.
fn ntropy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ntropy"))
}

#[test]
fn bare_invocation_prints_help() {
    assert_cmd_snapshot!(ntropy());
}
