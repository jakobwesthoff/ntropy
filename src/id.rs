// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Note identity: a ULID newtype.
//!
//! A note's canonical identity is a 26-character Crockford base32 ULID that
//! leads its filename (ADR 0004). Because a ULID's leading bits are a
//! millisecond timestamp, a lexical sort of ids is chronological, which is what
//! makes "newest first" a plain descending sort (ADR 0025).
//!
//! The newtype exists to give identity a domain name, a controlled parsing
//! surface with positioned-free semantic errors, and the disambiguator helper
//! the view layer needs (ADR 0023), rather than leaking `ulid::Ulid` across the
//! crate.

use std::fmt;
use std::str::FromStr;

use ulid::Ulid;

/// The fixed width of a ULID in Crockford base32 characters.
pub const ULID_LEN: usize = 26;

/// A note identifier backed by a ULID.
///
/// `Ord`/`PartialOrd` derive from the underlying 128-bit value, so sorting ids
/// ascending is chronological and descending is newest-first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(Ulid);

/// An invalid ULID string (wrong length or out-of-alphabet character).
#[derive(Debug, thiserror::Error)]
#[error("`{input}` is not a valid 26-character ULID")]
pub struct IdError {
    input: String,
}

impl Id {
    /// Generate a fresh identity stamped with the current time.
    pub fn generate() -> Self {
        Id(Ulid::new())
    }

    /// Construct an identity from an explicit millisecond timestamp with a zero
    /// random component. Used where a deterministic, time-anchored id is needed
    /// (notably tests of date rendering and ordering).
    pub fn from_timestamp_ms(ms: u64) -> Self {
        Id(Ulid::from_parts(ms, 0))
    }

    /// The embedded creation time in milliseconds since the Unix epoch.
    pub fn timestamp_ms(&self) -> u64 {
        self.0.timestamp_ms()
    }

    /// The trailing `n` characters of the canonical 26-character form.
    ///
    /// The view collision disambiguator appends these (ADR 0023). The trailing
    /// portion is the random part of the ULID; same-millisecond ids share their
    /// leading timestamp characters, so the tail is what actually distinguishes
    /// them. `n` is clamped to the ULID width.
    pub fn tail(&self, n: usize) -> String {
        let s = self.to_string();
        let n = n.min(ULID_LEN);
        s[ULID_LEN - n..].to_string()
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `ulid::Ulid` renders the canonical fixed-width uppercase form.
        write!(f, "{}", self.0)
    }
}

impl FromStr for Id {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ulid::from_string(s).map(Id).map_err(|_| IdError {
            input: s.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_roundtrips_through_string() {
        let id = Id::generate();
        let parsed: Id = id.to_string().parse().expect("roundtrip");
        assert_eq!(id, parsed);
    }

    #[test]
    fn display_is_fixed_width_26() {
        assert_eq!(Id::generate().to_string().len(), ULID_LEN);
    }

    #[test]
    fn parse_accepts_canonical_uppercase() {
        let s = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let id: Id = s.parse().expect("valid ULID");
        assert_eq!(id.to_string(), s);
    }

    #[test]
    fn parse_is_case_insensitive() {
        // Crockford base32 decoding is case-insensitive; the canonical render
        // is uppercase regardless of input case.
        let lower = "01arz3ndektsv4rrffq69g5fav";
        let id: Id = lower.parse().expect("valid ULID");
        assert_eq!(id.to_string(), lower.to_uppercase());
    }

    #[test]
    fn parse_rejects_wrong_length() {
        assert!("01ARZ3NDEK".parse::<Id>().is_err());
        assert!("".parse::<Id>().is_err());
    }

    #[test]
    fn parse_rejects_out_of_alphabet_characters() {
        // `I`, `L`, `O`, `U` are excluded from Crockford base32.
        let bad = "01ARZ3NDEKTSV4RRFFQ69G5FAU";
        assert!(bad.parse::<Id>().is_err());
    }

    #[test]
    fn timestamp_roundtrips() {
        let id = Id::from_timestamp_ms(1_700_000_000_000);
        assert_eq!(id.timestamp_ms(), 1_700_000_000_000);
    }

    #[test]
    fn ordering_is_chronological() {
        let earlier = Id::from_timestamp_ms(1_000);
        let later = Id::from_timestamp_ms(2_000);
        assert!(earlier < later);
    }

    #[test]
    fn tail_returns_trailing_characters() {
        let id: Id = "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse().expect("valid");
        assert_eq!(id.tail(3), "FAV");
        assert_eq!(id.tail(0), "");
    }

    #[test]
    fn tail_clamps_to_full_width() {
        let id = Id::generate();
        assert_eq!(id.tail(999), id.to_string());
    }
}
