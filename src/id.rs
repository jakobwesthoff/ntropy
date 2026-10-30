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

use std::collections::HashSet;
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
        Id(Ulid::generate())
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

    /// A new identity for a copy of the note with this one (ADR 0067).
    ///
    /// The time part stays, so the copy keeps its creation date. The other
    /// 80 bits are derived from this identity and the copy's file name rather
    /// than drawn at random: every machine that reassigns the same copy of a
    /// synced vault computes the same ULID, so the vault converges instead of
    /// ending up with the note twice. A counter joins the hash input and
    /// counts up while the result is `taken` or this identity itself.
    pub fn reassigned(&self, copy_name: &str, taken: &HashSet<Id>) -> Id {
        let ulid = self.to_string();
        (0u64..)
            .map(|counter| {
                let hash = fnv1a_128(&[
                    ulid.as_bytes(),
                    copy_name.as_bytes(),
                    &counter.to_be_bytes(),
                ]);
                Id(Ulid::from_parts(self.timestamp_ms(), hash & RANDOM_MASK))
            })
            .find(|candidate| candidate != self && !taken.contains(candidate))
            .expect("the counter runs until an identity is free, and a vault holds finitely many")
    }
}

/// The bits of a ULID below its 48-bit timestamp.
const RANDOM_MASK: u128 = (1 << 80) - 1;

/// The 128-bit FNV-1a hash of `parts`, concatenated.
///
/// Written out because the value must be the same on every machine and in
/// every release; `std`'s hashers promise neither. The ULID text is fixed
/// width and the counter is eight bytes, so concatenating `reassigned`'s
/// parts cannot make two different inputs read the same.
fn fnv1a_128(parts: &[&[u8]]) -> u128 {
    const OFFSET_BASIS: u128 = 0x6c62272e07bb014262b821756295c58d;
    const PRIME: u128 = 0x0000000001000000000000000000013b;
    parts
        .iter()
        .flat_map(|part| part.iter())
        .fold(OFFSET_BASIS, |hash, &byte| {
            (hash ^ u128::from(byte)).wrapping_mul(PRIME)
        })
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

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    #[test]
    fn fnv1a_128_matches_the_published_test_values() {
        // The empty input yields the offset basis; "a" and "foobar" are the
        // reference vectors of the FNV test suite.
        assert_eq!(fnv1a_128(&[]), 0x6c62272e07bb014262b821756295c58d);
        assert_eq!(fnv1a_128(&[b"a"]), 0xd228cb696f1a8caf78912b704e4a8964);
        assert_eq!(
            fnv1a_128(&[b"foo", b"bar"]),
            0x343e1662793c64bf6f0d3597ba446f18
        );
    }

    #[test]
    fn reassigned_keeps_the_time_and_changes_the_rest() {
        let id: Id = ULID.parse().expect("valid");
        let new = id.reassigned("plan-b.md", &HashSet::new());
        assert_ne!(new, id);
        assert_eq!(new.timestamp_ms(), id.timestamp_ms());
        assert_eq!(new.to_string()[..10], ULID[..10]);
    }

    #[test]
    fn reassigned_is_the_same_for_the_same_copy() {
        // Every machine that reassigns the same copy must arrive at the same
        // ULID, so a synced vault converges.
        let id: Id = ULID.parse().expect("valid");
        let taken = HashSet::new();
        assert_eq!(
            id.reassigned("plan-b.md", &taken),
            id.reassigned("plan-b.md", &taken)
        );
    }

    #[test]
    fn reassigned_differs_for_different_copy_names() {
        let id: Id = ULID.parse().expect("valid");
        let taken = HashSet::new();
        assert_ne!(
            id.reassigned("plan-b.md", &taken),
            id.reassigned("plan copy.md", &taken)
        );
    }

    #[test]
    fn reassigned_avoids_a_taken_id() {
        let id: Id = ULID.parse().expect("valid");
        let first = id.reassigned("plan-b.md", &HashSet::new());
        let taken = HashSet::from([first]);

        let next = id.reassigned("plan-b.md", &taken);

        assert_ne!(next, first);
        assert_ne!(next, id);
        assert_eq!(next.timestamp_ms(), id.timestamp_ms());
    }
}
