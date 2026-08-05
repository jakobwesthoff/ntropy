// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Turning CLI flags into a vault session (ADR 0041).
//!
//! The library owns the retrieval chain; this is the layer that tells it what
//! this invocation has available: an identity file from `--identity` or
//! `$NTROPY_IDENTITY`, a passphrase file, the OS credential store, and — only
//! when a terminal is there to ask on — a prompt.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use ntropy::cipher::{AgeCipher, Passphrase};
use ntropy::keys::store::{IdentityStore, KeyringStore, NullIdentityStore};
use ntropy::keys::{self, Acquisition};
use ntropy::session::VaultSession;
use ntropy::vault::Vault;

use crate::cli::GlobalArgs;

use super::interact;
use super::prompt::TtyPassphrasePrompt;

/// What this invocation offers the retrieval chain.
pub struct KeyContext {
    identity_file: Option<PathBuf>,
    passphrase_file: Option<PathBuf>,
    /// Whether a passphrase may be asked for interactively.
    ///
    /// `-n` forces this off even on a terminal: a headless script that blocked
    /// on `/dev/tty` would hang rather than fail, which is the outcome ADR 0036
    /// exists to prevent.
    may_prompt: bool,
}

impl KeyContext {
    /// Read the key-related flags, falling back to the environment.
    ///
    /// `$NTROPY_IDENTITY` is read directly rather than through clap's `env`
    /// support, matching how `$NTROPY_VAULT` is handled and keeping the
    /// `--help` output under our control.
    pub fn from_args(global: &GlobalArgs) -> Self {
        let identity_file = global
            .identity
            .clone()
            .or_else(|| std::env::var_os("NTROPY_IDENTITY").map(PathBuf::from));
        Self {
            identity_file,
            passphrase_file: global.passphrase_file.clone(),
            may_prompt: interact::is_interactive(global.non_interactive),
        }
    }

    /// The passphrase this invocation can supply without asking, if any.
    pub fn passphrase_from_file(&self) -> Result<Option<Passphrase>> {
        let Some(path) = self.passphrase_file.as_deref() else {
            return Ok(None);
        };
        Ok(Some(keys::read_passphrase_file(path).with_context(
            || format!("while reading the passphrase file `{}`", path.display()),
        )?))
    }

    /// A passphrase for creating or re-wrapping a vault identity.
    ///
    /// The file wins when given; otherwise the terminal is asked, and a run
    /// with neither cannot proceed.
    pub fn new_passphrase(&self, label: &str) -> Result<Passphrase> {
        use ntropy::keys::PassphrasePrompt;
        if let Some(passphrase) = self.passphrase_from_file()? {
            return Ok(passphrase);
        }
        if !self.may_prompt {
            anyhow::bail!(
                "no passphrase available: pass `--passphrase-file <PATH>` or run \
                 without `-n` so one can be typed"
            );
        }
        TtyPassphrasePrompt
            .new_passphrase(label)
            .context("while reading the new passphrase")
    }

    /// The credential store to use.
    ///
    /// An invocation running against an explicitly named identity file gets a
    /// store that keeps nothing: it must not write to the user's keychain as a
    /// side effect of a command they did not ask to unlock anything, and on
    /// macOS merely opening the keychain can raise an authorization dialog.
    fn store(&self) -> Result<Box<dyn IdentityStore>> {
        if self.identity_file.is_some() {
            return Ok(Box::new(NullIdentityStore));
        }
        Ok(Box::new(
            KeyringStore::open().context("while opening the OS credential store")?,
        ))
    }

    /// Open a session over `vault`, acquiring its identity if it is encrypted.
    pub fn open(&self, vault: Vault) -> Result<VaultSession> {
        let layout = vault.layout().clone();
        let Some(recipient) = keys::read_recipient_at(&layout.identity_pub())
            .context("while reading the vault's recipient")?
        else {
            return Ok(VaultSession::plaintext(vault));
        };

        let store = self.store()?;
        let prompt = TtyPassphrasePrompt;
        let request = Acquisition {
            recipient: &recipient,
            wrapped_identity: &layout.identity_file(),
            identity_file: self.identity_file.as_deref(),
            passphrase_file: self.passphrase_file.as_deref(),
            store: store.as_ref(),
            prompt: self.may_prompt.then_some(&prompt),
        };

        // `None` is a locked vault, not a failure: writing still works, and
        // commands that need to read report it themselves with an error naming
        // `ntropy unlock`.
        let identity = keys::acquire(&request).context("while obtaining the vault's identity")?;
        Ok(VaultSession::with_cipher(
            vault,
            Arc::new(AgeCipher::new(recipient, identity)),
        ))
    }

    /// Obtain the identity, refusing to accept a locked vault.
    pub fn unlock(&self, vault: &Vault) -> Result<String> {
        let layout = vault.layout();
        let recipient = keys::read_recipient_at(&layout.identity_pub())
            .context("while reading the vault's recipient")?
            .ok_or_else(|| anyhow::anyhow!("this vault is not encrypted"))?;

        let store = self.store()?;
        let prompt = TtyPassphrasePrompt;
        let request = Acquisition {
            recipient: &recipient,
            wrapped_identity: &layout.identity_file(),
            identity_file: self.identity_file.as_deref(),
            passphrase_file: self.passphrase_file.as_deref(),
            store: store.as_ref(),
            prompt: self.may_prompt.then_some(&prompt),
        };
        keys::unlock(&request).context("while unlocking the vault")?;
        Ok(recipient.as_str())
    }

    /// Forget the stored identity, reporting whether there was one.
    pub fn lock(&self, vault: &Vault) -> Result<(String, bool)> {
        let recipient = keys::read_recipient_at(&vault.layout().identity_pub())
            .context("while reading the vault's recipient")?
            .ok_or_else(|| anyhow::anyhow!("this vault is not encrypted"))?;

        // Always the real store: `lock` exists to clear the keychain, so
        // honoring `--identity` here would make it silently do nothing.
        let store = KeyringStore::open().context("while opening the OS credential store")?;
        let removed = keys::lock(&store, &recipient).context("while locking the vault")?;
        Ok((recipient.as_str(), removed))
    }
}
