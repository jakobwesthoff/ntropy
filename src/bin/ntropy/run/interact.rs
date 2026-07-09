// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The interactive-vs-plain decision (ADR 0036).
//!
//! ntropy is interactive whenever a controlling terminal is available;
//! `--non-interactive`/`-n` forces plain behavior. Redirecting stdout does not
//! demote to plain mode: stdout is purely a data channel, and all human
//! interaction (picker, prompts, editor) goes through the controlling
//! terminal, so `ntropy search -p | pbcopy` shows the picker and pipes only
//! the selected path.

use std::fs::{File, OpenOptions};
use std::io;

/// Whether to behave interactively (launch the picker, open the editor).
///
/// `non_interactive` is the `-n` flag: when set, the answer is always plain.
pub fn is_interactive(non_interactive: bool) -> bool {
    !non_interactive && open_tty().is_ok()
}

/// The controlling terminal, opened for reading and writing.
///
/// Fails where no controlling terminal exists (cron, CI, `docker exec`
/// without `-t`), which is exactly the environment that must never block on
/// input.
pub fn open_tty() -> io::Result<File> {
    OpenOptions::new().read(true).write(true).open("/dev/tty")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_n_flag_always_forces_plain_mode() {
        // Holds with or without a controlling terminal, so it is the one
        // branch a test can pin down deterministically.
        assert!(!is_interactive(true));
    }
}
