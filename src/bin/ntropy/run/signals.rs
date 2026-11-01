// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Signal dispositions changed for the length of one exchange with a child
//! process, and restored when it ends.

/// Ignore `SIGPIPE` process-wide for the lifetime of the value, restoring the
/// previous disposition on drop.
///
/// A pipe write to a closed read end raises `SIGPIPE`; with the signal
/// ignored, the write fails with `EPIPE` instead, which the caller handles as
/// an ordinary error. The disposition is process-wide state (a thread-local
/// mask does not contain it on macOS, where the signal is delivered
/// process-wide), so the scope is kept to the piped exchange and the
/// previous disposition — the fatal default that gives ntropy its quiet exit
/// when its own stdout pipe closes — comes back the moment the scope ends.
pub(super) struct SigpipeIgnoredScope {
    previous: libc::sighandler_t,
}

impl SigpipeIgnoredScope {
    pub(super) fn new() -> Self {
        // SAFETY: `SIGPIPE` and `SIG_IGN` are valid signal/handler constants,
        // and `signal` returns the previous handler for the restore.
        let previous = unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
        SigpipeIgnoredScope { previous }
    }
}

impl Drop for SigpipeIgnoredScope {
    fn drop(&mut self) {
        // SAFETY: `self.previous` came from the paired `signal` call above.
        unsafe {
            libc::signal(libc::SIGPIPE, self.previous);
        }
    }
}

/// Catch `SIGINT` with a handler that does nothing for the lifetime of the
/// value, restoring the previous disposition on drop.
///
/// Ctrl-C reaches every process in the terminal's foreground group: a pager
/// ntropy waits for, and ntropy itself. Caught, the signal leaves ntropy
/// waiting until the pager is done, so the shell does not take the terminal
/// back while the pager still draws on it. It is caught rather than ignored
/// because an ignored signal stays ignored across `exec`, so the pager would
/// inherit it, while a caught one is reset to the default there.
pub(super) struct SigintCaughtScope {
    previous: libc::sigaction,
}

/// The handler [`SigintCaughtScope`] installs: the signal arrives and nothing
/// happens.
extern "C" fn ignore_interrupt(_signal: libc::c_int) {}

impl SigintCaughtScope {
    pub(super) fn new() -> Self {
        // SAFETY: a zeroed `sigaction` is a valid starting value; the handler
        // is an `extern "C" fn(c_int)` that touches nothing, and `SA_RESTART`
        // resumes the write and the wait the signal interrupts.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = ignore_interrupt as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            let mut previous: libc::sigaction = std::mem::zeroed();
            libc::sigaction(libc::SIGINT, &action, &mut previous);
            SigintCaughtScope { previous }
        }
    }
}

impl Drop for SigintCaughtScope {
    fn drop(&mut self) {
        // SAFETY: `self.previous` came from the paired `sigaction` call above.
        unsafe {
            libc::sigaction(libc::SIGINT, &self.previous, std::ptr::null_mut());
        }
    }
}

/// Held by every test that changes or reads the `SIGINT` disposition, which
/// is process-wide while tests run on threads side by side.
#[cfg(test)]
pub(super) static SIGINT_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    /// The handler `SIGINT` has now.
    fn current_sigint_handler() -> libc::sighandler_t {
        // SAFETY: a zeroed `sigaction` is a valid out-parameter, and a null
        // new action only reads the current one.
        unsafe {
            let mut current: libc::sigaction = std::mem::zeroed();
            libc::sigaction(libc::SIGINT, std::ptr::null(), &mut current);
            current.sa_sigaction
        }
    }

    #[test]
    fn sigint_is_caught_inside_the_scope_and_restored_after_it() {
        let _serial = SIGINT_TESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let before = current_sigint_handler();
        {
            let _scope = SigintCaughtScope::new();
            let inside = current_sigint_handler();
            assert_ne!(inside, libc::SIG_IGN, "ignored, so a child inherits it");
            assert_ne!(inside, libc::SIG_DFL, "not caught");
            assert_ne!(inside, before);
        }
        assert_eq!(current_sigint_handler(), before);
    }
}
