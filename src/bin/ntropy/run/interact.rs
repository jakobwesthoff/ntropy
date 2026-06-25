// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The interactive-vs-plain decision (ADR 0014).
//!
//! ntropy is interactive on a TTY and non-interactive when piped;
//! `--non-interactive`/`-n` forces plain behavior even on a TTY. The decision
//! keys off whether stdout is a terminal.

use std::io::IsTerminal;

/// Whether to behave interactively (launch the picker, open the editor).
///
/// `non_interactive` is the `-n` flag: when set, the answer is always plain.
pub fn is_interactive(non_interactive: bool) -> bool {
    !non_interactive && std::io::stdout().is_terminal()
}
