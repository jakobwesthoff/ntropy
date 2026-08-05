// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `age` binding: the only module in the crate that names `age::` types.
//!
//! Two artifact shapes exist, and they are deliberately different:
//!
//! - **Notes** are encrypted to the vault recipient in age's binary format.
//!   They are opaque blobs nothing but ntropy reads, so armoring would only
//!   inflate them.
//! - **The vault identity** is wrapped with age's scrypt passphrase recipient
//!   and ASCII-armored, so `.ntropy/identity.age` stays text and survives a
//!   text-mode transport that would corrupt binary.

use std::io::{Read, Write};
use std::str::FromStr;

use age::armor::{ArmoredReader, ArmoredWriter, Format};
use age::secrecy::{ExposeSecret, SecretString};

use super::CryptoError;
use crate::cipher::Passphrase;

type Result<T> = std::result::Result<T, CryptoError>;

/// scrypt work factor used when the crate is built for tests. See the comment
/// at its use site in [`wrap_identity`].
#[cfg(test)]
const TEST_WORK_FACTOR: u8 = 4;

// =============================================================================
// Key material
// =============================================================================

/// A vault's secret key: what decrypts its notes.
#[derive(Clone)]
pub struct Identity(age::x25519::Identity);

/// A vault's public key: what encrypts notes to it.
///
/// This is the value stored in `.ntropy/identity.pub`, and its presence is
/// what marks a vault as encrypted.
#[derive(Clone, PartialEq, Eq)]
pub struct Recipient(age::x25519::Recipient);

impl Identity {
    /// Derive the matching recipient.
    pub fn to_public(&self) -> Recipient {
        Recipient(self.0.to_public())
    }

    /// The `AGE-SECRET-KEY-1...` form, for handing to the OS keychain.
    ///
    /// Named to make every call site read as a deliberate exposure of secret
    /// material rather than an incidental `to_string`.
    pub fn expose_secret(&self) -> String {
        self.0.to_string().expose_secret().to_owned()
    }
}

impl FromStr for Identity {
    type Err = CryptoError;

    fn from_str(s: &str) -> Result<Self> {
        age::x25519::Identity::from_str(s.trim())
            .map(Identity)
            .map_err(|_| CryptoError::BadIdentity)
    }
}

impl Recipient {
    /// The `age1...` form, for `.ntropy/identity.pub` and keychain lookup.
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl FromStr for Recipient {
    type Err = CryptoError;

    fn from_str(s: &str) -> Result<Self> {
        age::x25519::Recipient::from_str(s.trim())
            .map(Recipient)
            .map_err(|_| CryptoError::BadRecipient(s.trim().to_owned()))
    }
}

impl std::fmt::Display for Recipient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// `Debug` is hand-written for both: the derived form would print the key
// material, and these types end up inside error and log values.
impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Identity(<redacted>)")
    }
}

impl std::fmt::Debug for Recipient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recipient({})", self.0)
    }
}

/// Generate a fresh vault keypair.
pub fn generate_keypair() -> (Identity, Recipient) {
    let identity = Identity(age::x25519::Identity::generate());
    let recipient = identity.to_public();
    (identity, recipient)
}

// =============================================================================
// Note encryption
// =============================================================================

/// Encrypt `plaintext` to `recipient` in age's binary format.
///
/// Needs no identity: this is the asymmetric property the whole locking model
/// rests on, which is what lets `ntropy new` work on a locked vault.
pub fn encrypt_to(recipient: &Recipient, plaintext: &str) -> Result<Vec<u8>> {
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient.0 as &dyn age::Recipient))
            .map_err(|e| CryptoError::Encrypt(std::io::Error::other(e)))?;

    let mut ciphertext = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut ciphertext)
        .map_err(|e| CryptoError::Encrypt(std::io::Error::other(e)))?;
    writer
        .write_all(plaintext.as_bytes())
        .map_err(CryptoError::Encrypt)?;
    writer.finish().map_err(CryptoError::Encrypt)?;

    Ok(ciphertext)
}

/// Decrypt `ciphertext` with `identity`.
pub fn decrypt_with(identity: &Identity, ciphertext: &[u8]) -> Result<String> {
    let decryptor = age::Decryptor::new_buffered(ciphertext).map_err(decrypt_error)?;
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity.0 as &dyn age::Identity))
        .map_err(decrypt_error)?;

    let mut plaintext = Vec::new();
    reader
        .read_to_end(&mut plaintext)
        .map_err(CryptoError::Decrypt)?;

    String::from_utf8(plaintext).map_err(CryptoError::NotUtf8)
}

// =============================================================================
// The passphrase wrap around the identity
// =============================================================================

/// Wrap `identity` under `passphrase`, ASCII-armored.
///
/// The construction is age's own scrypt recipient, the same one `age -p`
/// produces, so the stock `age` CLI can unwrap the result.
pub fn wrap_identity(identity: &Identity, passphrase: &Passphrase) -> Result<String> {
    if passphrase.is_empty() {
        return Err(CryptoError::EmptyPassphrase);
    }

    let recipient = scrypt_recipient(passphrase);
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|e| CryptoError::Encrypt(std::io::Error::other(e)))?;

    let mut armored = Vec::new();
    let armor_writer = ArmoredWriter::wrap_output(&mut armored, Format::AsciiArmor)
        .map_err(CryptoError::Encrypt)?;
    let mut writer = encryptor
        .wrap_output(armor_writer)
        .map_err(|e| CryptoError::Encrypt(std::io::Error::other(e)))?;
    writer
        .write_all(identity.expose_secret().as_bytes())
        .map_err(CryptoError::Encrypt)?;
    writer
        .finish()
        .and_then(|armor| armor.finish())
        .map_err(CryptoError::Encrypt)?;

    String::from_utf8(armored).map_err(CryptoError::NotUtf8)
}

/// Unwrap an armored, passphrase-wrapped identity.
///
/// A passphrase that does not open the file is reported as
/// [`CryptoError::WrongPassphrase`] rather than a generic decrypt failure,
/// since for a scrypt-wrapped file those are the same condition and the user
/// needs to hear the useful one.
pub fn unwrap_identity(armored: &str, passphrase: &Passphrase) -> Result<Identity> {
    let identity = age::scrypt::Identity::new(secret_string(passphrase));

    let reader = ArmoredReader::new(armored.as_bytes());
    let decryptor = age::Decryptor::new(reader).map_err(decrypt_error)?;
    // Only the key-matching step is remapped. A malformed or truncated file
    // fails earlier, in `Decryptor::new`, and keeps its own error: claiming
    // "wrong passphrase" for a corrupt file would send the user hunting for
    // the wrong problem.
    let mut plaintext_reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| match e {
            // A scrypt stanza that will not unwrap and a stanza no key matches
            // are the same condition here: there is exactly one possible key,
            // and it is derived from the passphrase.
            age::DecryptError::DecryptionFailed | age::DecryptError::NoMatchingKeys => {
                CryptoError::WrongPassphrase
            }
            other => decrypt_error(other),
        })?;

    let mut secret = Vec::new();
    plaintext_reader
        .read_to_end(&mut secret)
        .map_err(CryptoError::Decrypt)?;

    let secret = String::from_utf8(secret).map_err(CryptoError::NotUtf8)?;
    Identity::from_str(&secret)
}

// =============================================================================
// Shared helpers
// =============================================================================

/// Bridge our passphrase type into the one age's API requires.
fn secret_string(passphrase: &Passphrase) -> SecretString {
    SecretString::from(passphrase.expose().to_owned())
}

/// The scrypt recipient the identity wrap is built on.
///
/// The work factor is age's own default, which targets roughly a second of
/// derivation. That cost is the entire point of a passphrase wrap, and it is
/// paid once per `unlock` rather than per command, because the keychain stores
/// the unwrapped secret key.
#[cfg(not(test))]
fn scrypt_recipient(passphrase: &Passphrase) -> age::scrypt::Recipient {
    age::scrypt::Recipient::new(secret_string(passphrase))
}

/// The test build derives with the cheapest factor the format allows.
///
/// Wrapping and unwrapping happen dozens of times across this module, `keys`
/// and `ops::init`, and nothing any of those tests assert depends on how
/// expensive the derivation was.
#[cfg(test)]
fn scrypt_recipient(passphrase: &Passphrase) -> age::scrypt::Recipient {
    let mut recipient = age::scrypt::Recipient::new(secret_string(passphrase));
    recipient.set_work_factor(TEST_WORK_FACTOR);
    recipient
}

/// Map age's decrypt failures onto ours, keeping "no key matched" distinct
/// from "these bytes are not a decryptable age file".
fn decrypt_error(error: age::DecryptError) -> CryptoError {
    match error {
        age::DecryptError::NoMatchingKeys => CryptoError::NoMatchingKey,
        age::DecryptError::Io(e) => CryptoError::Decrypt(e),
        other => CryptoError::Decrypt(std::io::Error::other(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passphrase(value: &str) -> Passphrase {
        Passphrase::new(value)
    }

    // -------------------------------------------------------------------------
    // Keys
    // -------------------------------------------------------------------------

    #[test]
    fn generated_recipient_matches_its_identity() {
        let (identity, recipient) = generate_keypair();
        assert_eq!(identity.to_public().as_str(), recipient.as_str());
    }

    #[test]
    fn recipient_roundtrips_through_its_string_form() {
        let (_, recipient) = generate_keypair();
        let parsed: Recipient = recipient.as_str().parse().expect("parse recipient");
        assert_eq!(parsed.as_str(), recipient.as_str());
    }

    #[test]
    fn identity_roundtrips_through_its_secret_form() {
        let (identity, recipient) = generate_keypair();
        let parsed: Identity = identity.expose_secret().parse().expect("parse identity");
        assert_eq!(parsed.to_public().as_str(), recipient.as_str());
    }

    #[test]
    fn key_strings_use_the_documented_prefixes() {
        // Pinned because both forms are user-visible: the recipient lands in
        // `.ntropy/identity.pub` and the secret in the OS keychain.
        let (identity, recipient) = generate_keypair();
        assert!(recipient.as_str().starts_with("age1"), "{recipient}");
        assert!(identity.expose_secret().starts_with("AGE-SECRET-KEY-1"));
    }

    #[test]
    fn surrounding_whitespace_is_tolerated_when_parsing() {
        // `identity.pub` is a text file; a trailing newline is the norm.
        let (identity, recipient) = generate_keypair();
        let padded = format!("  {}\n", recipient.as_str());
        assert_eq!(
            padded.parse::<Recipient>().expect("parse").as_str(),
            recipient.as_str()
        );

        let padded = format!("{}\n", identity.expose_secret());
        assert_eq!(
            padded
                .parse::<Identity>()
                .expect("parse")
                .to_public()
                .as_str(),
            recipient.as_str()
        );
    }

    #[test]
    fn a_non_key_string_is_rejected() {
        assert!(matches!(
            "not-a-key".parse::<Recipient>(),
            Err(CryptoError::BadRecipient(_))
        ));
        assert!(matches!(
            "not-a-key".parse::<Identity>(),
            Err(CryptoError::BadIdentity)
        ));
    }

    #[test]
    fn a_recipient_is_not_accepted_as_an_identity() {
        let (_, recipient) = generate_keypair();
        assert!(matches!(
            recipient.as_str().parse::<Identity>(),
            Err(CryptoError::BadIdentity)
        ));
    }

    #[test]
    fn debug_output_hides_the_secret() {
        let (identity, _) = generate_keypair();
        let rendered = format!("{identity:?}");
        assert!(!rendered.contains("AGE-SECRET-KEY"), "{rendered}");
    }

    // -------------------------------------------------------------------------
    // Note encryption
    // -------------------------------------------------------------------------

    #[test]
    fn encrypt_then_decrypt_roundtrips() {
        let (identity, recipient) = generate_keypair();
        let plaintext = "---\ntitle: Weekly Sync\n---\n\nBody.\n";
        let ciphertext = encrypt_to(&recipient, plaintext).expect("encrypt");
        assert_eq!(
            decrypt_with(&identity, &ciphertext).expect("decrypt"),
            plaintext
        );
    }

    #[test]
    fn roundtrip_preserves_non_ascii_and_crlf() {
        // Notes are reconstructed byte-for-byte from their header and body, so
        // the cipher must not normalize anything.
        let (identity, recipient) = generate_keypair();
        let plaintext = "Über Größe — 日本語\r\nline two\r\n";
        let ciphertext = encrypt_to(&recipient, plaintext).expect("encrypt");
        assert_eq!(
            decrypt_with(&identity, &ciphertext).expect("decrypt"),
            plaintext
        );
    }

    #[test]
    fn roundtrip_handles_empty_content() {
        let (identity, recipient) = generate_keypair();
        let ciphertext = encrypt_to(&recipient, "").expect("encrypt");
        assert_eq!(decrypt_with(&identity, &ciphertext).expect("decrypt"), "");
    }

    #[test]
    fn ciphertext_differs_on_every_encryption() {
        // The fact the whole test strategy rests on: age uses a fresh file key
        // and ephemeral share per file, so ciphertext is never snapshot-stable
        // and tests must assert round trips instead.
        let (_, recipient) = generate_keypair();
        let first = encrypt_to(&recipient, "same input").expect("encrypt");
        let second = encrypt_to(&recipient, "same input").expect("encrypt");
        assert_ne!(first, second);
    }

    #[test]
    fn ciphertext_does_not_contain_the_plaintext() {
        let (_, recipient) = generate_keypair();
        let ciphertext = encrypt_to(&recipient, "Quarterly Review").expect("encrypt");
        assert!(
            !ciphertext
                .windows("Quarterly Review".len())
                .any(|w| w == b"Quarterly Review"),
        );
    }

    #[test]
    fn decrypting_with_the_wrong_identity_finds_no_key() {
        let (_, recipient) = generate_keypair();
        let (other, _) = generate_keypair();
        let ciphertext = encrypt_to(&recipient, "secret").expect("encrypt");
        assert!(matches!(
            decrypt_with(&other, &ciphertext),
            Err(CryptoError::NoMatchingKey)
        ));
    }

    #[test]
    fn decrypting_garbage_is_an_error_not_a_panic() {
        let (identity, _) = generate_keypair();
        assert!(matches!(
            decrypt_with(&identity, b"not an age file at all"),
            Err(CryptoError::Decrypt(_))
        ));
    }

    #[test]
    fn decrypting_empty_input_is_an_error() {
        let (identity, _) = generate_keypair();
        assert!(decrypt_with(&identity, b"").is_err());
    }

    #[test]
    fn decrypting_truncated_ciphertext_is_an_error() {
        // The half-written-file case: a crash mid-write must surface as an
        // error, never as silently truncated note content.
        let (identity, recipient) = generate_keypair();
        let ciphertext = encrypt_to(&recipient, "a reasonably long note body\n").expect("encrypt");
        let truncated = &ciphertext[..ciphertext.len() / 2];
        assert!(decrypt_with(&identity, truncated).is_err());
    }

    #[test]
    fn decrypting_tampered_ciphertext_is_an_error() {
        // The payload is authenticated, so a flipped byte must not decrypt.
        let (identity, recipient) = generate_keypair();
        let mut ciphertext = encrypt_to(&recipient, "trustworthy content\n").expect("encrypt");
        let last = ciphertext.len() - 1;
        ciphertext[last] ^= 0xff;
        assert!(decrypt_with(&identity, &ciphertext).is_err());
    }

    #[test]
    fn decrypting_non_utf8_plaintext_reports_the_encoding_failure() {
        // Reachable only for a file ntropy did not write, but it must be a
        // typed error rather than a lossy conversion.
        let (identity, recipient) = generate_keypair();
        let encryptor =
            age::Encryptor::with_recipients(std::iter::once(&recipient.0 as &dyn age::Recipient))
                .expect("encryptor");
        let mut ciphertext = Vec::new();
        let mut writer = encryptor.wrap_output(&mut ciphertext).expect("wrap");
        writer.write_all(&[0xff, 0xfe]).expect("write");
        writer.finish().expect("finish");

        assert!(matches!(
            decrypt_with(&identity, &ciphertext),
            Err(CryptoError::NotUtf8(_))
        ));
    }

    // -------------------------------------------------------------------------
    // The passphrase wrap
    // -------------------------------------------------------------------------

    #[test]
    fn wrap_then_unwrap_roundtrips_the_identity() {
        let (identity, recipient) = generate_keypair();
        let wrapped = wrap_identity(&identity, &passphrase("correct horse")).expect("wrap");
        let unwrapped = unwrap_identity(&wrapped, &passphrase("correct horse")).expect("unwrap");
        assert_eq!(unwrapped.to_public().as_str(), recipient.as_str());
    }

    #[test]
    fn the_wrapped_identity_is_armored_text() {
        // `.ntropy/identity.age` must survive a text-mode transport, so the
        // armor is not decoration.
        let (identity, _) = generate_keypair();
        let wrapped = wrap_identity(&identity, &passphrase("hunter2")).expect("wrap");
        assert!(wrapped.starts_with("-----BEGIN AGE ENCRYPTED FILE-----"));
        assert!(
            wrapped
                .trim_end()
                .ends_with("-----END AGE ENCRYPTED FILE-----")
        );
        assert!(wrapped.is_ascii(), "armor must be plain ASCII");
    }

    #[test]
    fn the_wrapped_identity_does_not_contain_the_secret() {
        let (identity, _) = generate_keypair();
        let wrapped = wrap_identity(&identity, &passphrase("hunter2")).expect("wrap");
        assert!(!wrapped.contains(&identity.expose_secret()));
    }

    #[test]
    fn wrapping_is_salted_so_two_wraps_differ() {
        let (identity, _) = generate_keypair();
        let first = wrap_identity(&identity, &passphrase("same")).expect("wrap");
        let second = wrap_identity(&identity, &passphrase("same")).expect("wrap");
        assert_ne!(first, second);
    }

    #[test]
    fn unwrapping_with_the_wrong_passphrase_says_so() {
        // The distinction that matters to a user: "wrong passphrase" rather
        // than a generic decrypt failure.
        let (identity, _) = generate_keypair();
        let wrapped = wrap_identity(&identity, &passphrase("right")).expect("wrap");
        assert!(matches!(
            unwrap_identity(&wrapped, &passphrase("wrong")),
            Err(CryptoError::WrongPassphrase)
        ));
    }

    #[test]
    fn unwrapping_a_non_age_file_is_an_error() {
        assert!(unwrap_identity("just some text\n", &passphrase("p")).is_err());
    }

    #[test]
    fn unwrapping_a_truncated_wrap_is_an_error() {
        let (identity, _) = generate_keypair();
        let wrapped = wrap_identity(&identity, &passphrase("p")).expect("wrap");
        let truncated = &wrapped[..wrapped.len() / 2];
        assert!(unwrap_identity(truncated, &passphrase("p")).is_err());
    }

    #[test]
    fn an_empty_passphrase_is_rejected_at_the_wrap_boundary() {
        // Refused where the vault is created rather than left to produce a
        // file that anyone can unwrap.
        let (identity, _) = generate_keypair();
        assert!(matches!(
            wrap_identity(&identity, &passphrase("")),
            Err(CryptoError::EmptyPassphrase)
        ));
    }

    #[test]
    fn a_corrupt_wrap_is_not_reported_as_a_wrong_passphrase() {
        // Distinguishing the two matters: one tells the user to try another
        // passphrase, the other tells them the file is damaged.
        let (identity, _) = generate_keypair();
        let wrapped = wrap_identity(&identity, &passphrase("p")).expect("wrap");
        let corrupted = wrapped.replace("-----BEGIN AGE ENCRYPTED FILE-----", "-----BEGIN X-----");
        assert!(matches!(
            unwrap_identity(&corrupted, &passphrase("p")),
            Err(CryptoError::Decrypt(_))
        ));
    }

    #[test]
    fn a_passphrase_is_compared_byte_for_byte() {
        // No trimming, no case folding: whitespace is part of the passphrase.
        let (identity, _) = generate_keypair();
        let wrapped = wrap_identity(&identity, &passphrase(" pass ")).expect("wrap");
        assert!(unwrap_identity(&wrapped, &passphrase("pass")).is_err());
        assert!(unwrap_identity(&wrapped, &passphrase(" pass ")).is_ok());
    }

    #[test]
    fn a_non_ascii_passphrase_roundtrips() {
        let (identity, recipient) = generate_keypair();
        let wrapped = wrap_identity(&identity, &passphrase("übergröße-日本語")).expect("wrap");
        let unwrapped = unwrap_identity(&wrapped, &passphrase("übergröße-日本語")).expect("unwrap");
        assert_eq!(unwrapped.to_public().as_str(), recipient.as_str());
    }
}
