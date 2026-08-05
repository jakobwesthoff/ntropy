// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Vault cryptography: age keypairs, note encryption, and the passphrase wrap
//! around a vault identity (ADR 0041).
//!
//! [`age_io`] is the only module in the crate that names `age::` types.
//! Everything above it works with the [`Identity`] and [`Recipient`] newtypes
//! re-exported here, so swapping the underlying implementation would not
//! ripple outward.
//!
//! Nothing written here is ntropy-specific: every artifact is a standard age
//! file, readable by the stock `age` CLI given the passphrase.

pub mod age_io;

pub use age_io::{Identity, Recipient};

/// Why a cryptographic operation failed.
///
/// `WrongPassphrase` is separated from the general decrypt failure so the CLI
/// can say "wrong passphrase" instead of the useless "decryption failed" that
/// covers both a bad key and corrupt bytes.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Encryption failed.
    #[error("while encrypting")]
    Encrypt(#[source] std::io::Error),

    /// Decryption failed: the bytes are not a well-formed age file, or they
    /// are truncated or corrupt.
    #[error("while decrypting")]
    Decrypt(#[source] std::io::Error),

    /// The ciphertext is well-formed but no supplied key opens it.
    #[error("no matching key for this ciphertext")]
    NoMatchingKey,

    /// The passphrase did not unwrap the identity.
    #[error("wrong passphrase")]
    WrongPassphrase,

    /// Decrypted bytes were not valid UTF-8, so they are not a note.
    #[error("decrypted content is not valid UTF-8")]
    NotUtf8(#[source] std::string::FromUtf8Error),

    /// A string was not a valid `age1...` recipient.
    #[error("`{0}` is not a valid age recipient")]
    BadRecipient(String),

    /// A string was not a valid `AGE-SECRET-KEY-1...` identity.
    #[error("not a valid age identity")]
    BadIdentity,

    /// An empty passphrase was supplied where one is required.
    #[error("the passphrase must not be empty")]
    EmptyPassphrase,
}
