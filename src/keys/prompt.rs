// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Asking a human for a passphrase.
//!
//! The library never talks to a terminal (ADR 0013), so it declares what it
//! needs and the binary supplies it over `/dev/tty` (ADR 0036). The same seam
//! is what lets the retrieval chain be tested without one.

use crate::cipher::Passphrase;

use super::KeyError;

/// Something that can ask a human for a passphrase.
pub trait PassphrasePrompt {
    /// Ask for an existing passphrase, to unwrap a vault identity.
    ///
    /// `label` names what is being unlocked, so a prompt can say which vault
    /// it is asking about.
    fn existing(&self, label: &str) -> Result<Passphrase, KeyError>;

    /// Ask for a new passphrase, confirming it before returning.
    ///
    /// Confirmation is the implementation's business: a mistyped passphrase on
    /// a freshly created vault makes every note in it unreadable, and nothing
    /// downstream can detect that.
    fn new_passphrase(&self, label: &str) -> Result<Passphrase, KeyError>;
}

/// A prompt that always answers with the same passphrase, for tests.
#[derive(Debug, Clone)]
pub struct FixedPassphrase {
    passphrase: Passphrase,
    /// How many times the prompt was consulted, so tests can assert that a
    /// path which should not prompt did not.
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl FixedPassphrase {
    /// A prompt answering with `passphrase`.
    pub fn new(passphrase: &str) -> Self {
        Self {
            passphrase: Passphrase::new(passphrase),
            calls: std::sync::Arc::default(),
        }
    }

    /// How many times this prompt has been asked anything.
    pub fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn answer(&self) -> Result<Passphrase, KeyError> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(self.passphrase.clone())
    }
}

impl PassphrasePrompt for FixedPassphrase {
    fn existing(&self, _label: &str) -> Result<Passphrase, KeyError> {
        self.answer()
    }

    fn new_passphrase(&self, _label: &str) -> Result<Passphrase, KeyError> {
        self.answer()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_prompt_answers_with_what_it_was_given() {
        let prompt = FixedPassphrase::new("correct horse");
        assert_eq!(
            prompt.existing("vault").expect("ask").expose(),
            "correct horse"
        );
        assert_eq!(
            prompt.new_passphrase("vault").expect("ask").expose(),
            "correct horse"
        );
    }

    #[test]
    fn a_fixed_prompt_counts_how_often_it_was_asked() {
        // Several tests assert that a path never reached the prompt; that
        // assertion is only meaningful if the counter works.
        let prompt = FixedPassphrase::new("p");
        assert_eq!(prompt.calls(), 0);
        prompt.existing("vault").expect("ask");
        prompt.existing("vault").expect("ask");
        assert_eq!(prompt.calls(), 2);
    }
}
