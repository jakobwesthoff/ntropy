// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The whole-vault conversions (ADR 0041).
//!
//! Each of these rewrites every note in the vault, so each confirms first
//! unless told not to, and each prints what it did. The crash-safe machinery
//! itself lives in `ntropy::migrate`; this layer supplies the keys, the
//! confirmation, and the report.

use std::sync::Arc;

use anyhow::{Context, Result, bail};

use ntropy::cipher::{AgeCipher, PlaintextCipher};
use ntropy::crypto::age_io;
use ntropy::keys;
use ntropy::migrate::{self, Marker, Migration, NoHooks, Operation};
use ntropy::session::VaultSession;
use ntropy::vault::{Vault, layout};

use crate::cli::{GlobalArgs, VaultCommand};

use super::keys::KeyContext;
use super::{confirm, plural};

/// Dispatch a `vault` subcommand.
pub fn run(global: &GlobalArgs, command: VaultCommand, interactive: bool) -> Result<()> {
    let vault = super::resolve_vault(global)?;
    let context = KeyContext::from_args(global);

    match command {
        VaultCommand::Encrypt { resume, yes } => {
            encrypt(global, &context, vault, resume, yes, interactive)
        }
        VaultCommand::Decrypt { resume, yes } => {
            decrypt(global, &context, vault, resume, yes, interactive)
        }
        VaultCommand::Rekey {
            resume,
            yes,
            new_passphrase_file,
        } => rekey(
            global,
            &context,
            vault,
            resume,
            yes,
            interactive,
            new_passphrase_file,
        ),
        VaultCommand::Passphrase {
            new_passphrase_file,
        } => passphrase(&context, vault, new_passphrase_file),
    }
}

/// Convert a plaintext vault to encrypted storage.
fn encrypt(
    global: &GlobalArgs,
    context: &KeyContext,
    vault: Vault,
    resume: bool,
    yes: bool,
    interactive: bool,
) -> Result<()> {
    if layout::is_encrypted(vault.root()) && !resume {
        bail!("this vault is already encrypted");
    }
    ask(yes, interactive, "Encrypt every note in this vault?")?;

    // The keypair is written before the marker, so a crash between the two
    // leaves a key the user still has rather than notes encrypted to one that
    // was never recorded.
    let layout = vault.layout().clone();
    let recipient = match keys::read_recipient_at(&layout.identity_pub())? {
        Some(existing) => existing,
        None => {
            let passphrase = context.new_passphrase(&vault.root().display().to_string())?;
            let (identity, recipient) = age_io::generate_keypair();
            keys::write_keypair_at(
                &layout.identity_pub(),
                &layout.identity_file(),
                &identity,
                &passphrase,
            )
            .context("while writing the vault's keypair")?;
            recipient
        }
    };

    // Verification reads every produced note back, so the target cipher needs
    // the identity, not just the recipient.
    let identity = context.identity_for(&vault, &recipient)?;
    let target = AgeCipher::new(recipient.clone(), Some(identity));
    let session = VaultSession::with_cipher(vault.clone(), Arc::new(PlaintextCipher));

    let report = migrate::run(
        &session,
        &Migration {
            operation: Operation::Encrypt,
            source: &PlaintextCipher,
            target: &target,
            resume,
            marker: Marker::new(Operation::Encrypt, Some(recipient.as_str()), now()),
            hooks: hooks(global),
        },
    )
    .context("while encrypting the vault")?;

    println!(
        "Encrypted {} ({} already done).",
        plural(report.produced, "note", "notes"),
        report.already_done
    );
    for dir in &report.views_removed {
        println!("removed view directory {}", dir.display());
    }
    history_notice();
    Ok(())
}

/// Convert an encrypted vault back to plaintext storage.
fn decrypt(
    global: &GlobalArgs,
    context: &KeyContext,
    vault: Vault,
    resume: bool,
    yes: bool,
    interactive: bool,
) -> Result<()> {
    if !layout::is_encrypted(vault.root()) {
        bail!("this vault is not encrypted");
    }
    ask(yes, interactive, "Decrypt every note in this vault?")?;

    let layout = vault.layout().clone();
    let recipient = keys::read_recipient_at(&layout.identity_pub())?
        .ok_or_else(|| anyhow::anyhow!("this vault is not encrypted"))?;
    let identity = context.identity_for(&vault, &recipient)?;
    let source = AgeCipher::new(recipient.clone(), Some(identity));
    let session = VaultSession::with_cipher(vault.clone(), Arc::new(PlaintextCipher));

    let report = migrate::run(
        &session,
        &Migration {
            operation: Operation::Decrypt,
            source: &source,
            target: &PlaintextCipher,
            resume,
            marker: Marker::new(Operation::Decrypt, None, now()),
            hooks: hooks(global),
        },
    )
    .context("while decrypting the vault")?;

    // The key files go once the notes no longer need them, and the credential
    // store entry with them: an entry for a vault that no longer has a key is
    // state the user cannot discover or clean up.
    keys::remove_keypair_at(&layout.identity_pub(), &layout.identity_file())
        .context("while removing the vault's key files")?;
    let _ = context.lock_recipient(&recipient);

    println!(
        "Decrypted {} ({} already done).",
        plural(report.produced, "note", "notes"),
        report.already_done
    );
    Ok(())
}

/// Re-encrypt every note to a fresh keypair.
fn rekey(
    global: &GlobalArgs,
    context: &KeyContext,
    vault: Vault,
    resume: bool,
    yes: bool,
    interactive: bool,
    new_passphrase_file: Option<std::path::PathBuf>,
) -> Result<()> {
    if !layout::is_encrypted(vault.root()) {
        bail!("this vault is not encrypted");
    }
    ask(yes, interactive, "Re-encrypt every note to a new key?")?;

    let layout = vault.layout().clone();
    let old_recipient = keys::read_recipient_at(&layout.identity_pub())?
        .ok_or_else(|| anyhow::anyhow!("this vault is not encrypted"))?;
    let old_identity = context.identity_for(&vault, &old_recipient)?;
    let source = AgeCipher::new(old_recipient.clone(), Some(old_identity));

    // The new key needs a passphrase to be wrapped under. Naming one changes
    // it; leaving it out reuses whatever opened the old key, so the vault keeps
    // its passphrase and only the key changes — which is what `rekey` is for.
    let label = vault.root().display().to_string();
    let passphrase = match new_passphrase_file.as_deref() {
        Some(path) => context.passphrase_from(Some(path), "the new passphrase")?,
        None => context.new_passphrase(&label)?,
    };
    let (new_identity, new_recipient) = age_io::generate_keypair();
    let target = AgeCipher::new(new_recipient.clone(), Some(new_identity.clone()));

    let mut marker = Marker::new(Operation::Rekey, Some(new_recipient.as_str()), now());
    marker.source_recipient = Some(old_recipient.as_str());

    let session = VaultSession::with_cipher(vault.clone(), Arc::new(PlaintextCipher));
    let report = migrate::run(
        &session,
        &Migration {
            operation: Operation::Rekey,
            source: &source,
            target: &target,
            resume,
            marker,
            hooks: hooks(global),
        },
    )
    .context("while rekeying the vault")?;

    // Only now, with every note readable by the new key, does the vault stop
    // recording the old one. Doing this earlier would leave an interrupted
    // rekey unable to read its own sources.
    keys::write_keypair_at(
        &layout.identity_pub(),
        &layout.identity_file(),
        &new_identity,
        &passphrase,
    )
    .context("while writing the new keypair")?;
    let _ = context.lock_recipient(&old_recipient);

    println!(
        "Re-encrypted {} to {}.",
        plural(report.produced, "note", "notes"),
        new_recipient.as_str()
    );
    Ok(())
}

/// Change the passphrase protecting the vault's identity.
fn passphrase(
    context: &KeyContext,
    vault: Vault,
    new_passphrase_file: Option<std::path::PathBuf>,
) -> Result<()> {
    if !layout::is_encrypted(vault.root()) {
        bail!("this vault is not encrypted");
    }
    let layout = vault.layout().clone();

    let old = context
        .existing_passphrase(&vault.root().display().to_string())
        .context("while reading the current passphrase")?;
    let new = context
        .passphrase_from(new_passphrase_file.as_deref(), "the new passphrase")
        .context("while reading the new passphrase")?;

    keys::change_passphrase_at(&layout.identity_file(), &old, &new)
        .context("while re-wrapping the vault's identity")?;

    println!("Passphrase changed. Notes were not touched.");
    Ok(())
}

/// Confirm a conversion that rewrites the whole vault.
fn ask(yes: bool, interactive: bool, question: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !interactive {
        bail!("this rewrites every note in the vault; pass `-y` to confirm");
    }
    if !confirm(&format!("{question} [y/N] "))? {
        bail!("cancelled");
    }
    Ok(())
}

/// The unconditional reminder that converting does not reach into history.
///
/// Printed for every vault because no reliable marker distinguishes one that
/// has been synced or committed from one that has not, and a vault that never
/// left the machine loses nothing by hearing it.
fn history_notice() {
    eprintln!(
        "note: plaintext revisions recorded before this conversion remain readable \
         in git history or a sync provider's version history; cleaning those is up to you"
    );
}

/// The current time, for the marker's informational timestamp.
fn now() -> String {
    jiff::Zoned::now().timestamp().to_string()
}

/// The hooks a run uses.
///
/// Debug builds honour `NTROPY_MIGRATION_FAIL_AFTER=produce|verify` so the CLI
/// contract tests can interrupt a conversion at a phase boundary through the
/// real binary. Release builds have no such seam; `cargo test` builds in debug,
/// so a release-profile test run simply skips those tests.
#[cfg(debug_assertions)]
fn hooks(_global: &GlobalArgs) -> &'static dyn migrate::MigrationHooks {
    use ntropy::migrate::{MigrationHooks, Phase};

    struct FailAfter(Phase);
    impl MigrationHooks for FailAfter {
        fn after_produce(&self) -> std::result::Result<(), migrate::MigrateError> {
            match self.0 {
                Phase::Produce => Err(migrate::MigrateError::Interrupted(Phase::Produce)),
                _ => Ok(()),
            }
        }
        fn after_verify(&self) -> std::result::Result<(), migrate::MigrateError> {
            match self.0 {
                Phase::Verify => Err(migrate::MigrateError::Interrupted(Phase::Verify)),
                _ => Ok(()),
            }
        }
    }

    match std::env::var("NTROPY_MIGRATION_FAIL_AFTER").as_deref() {
        Ok("produce") => &FailAfter(Phase::Produce),
        Ok("verify") => &FailAfter(Phase::Verify),
        _ => &NoHooks,
    }
}

#[cfg(not(debug_assertions))]
fn hooks(_global: &GlobalArgs) -> &'static dyn migrate::MigrationHooks {
    &NoHooks
}
