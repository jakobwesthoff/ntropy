// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! View leaf naming and collision disambiguation (ADRs 0009, 0023, 0068).
//!
//! A leaf symlink is named `<date>-<slug>.md`, where `<date>` is the readable
//! creation date and `<slug>` derives from the note title. When two notes in
//! the same view group would collide on that base name, a trailing slice of
//! each colliding note's ULID is appended to every collider. The slice starts
//! at 3 characters and grows until all collisions are resolved, up to the full
//! 26-character ULID; the *trailing* portion is used because same-day
//! collisions share the leading timestamp characters of their ULIDs. Names
//! are compared without regard to case, so they stay distinct on a
//! case-insensitive filesystem too.

use std::collections::{BTreeSet, HashMap};

use crate::id::{Id, ULID_LEN};

/// The disambiguator starts this short and grows only as needed.
const MIN_TAIL: usize = 3;

/// One note's inputs to leaf naming within a group.
#[derive(Debug, Clone)]
pub struct LeafInput {
    pub id: Id,
    /// Readable creation date, `YYYY-MM-DD`.
    pub date: String,
    /// Slug derived from the note's current title.
    pub slug: String,
}

impl LeafInput {
    /// The base leaf name (`<date>-<slug>`) before any disambiguation.
    fn base(&self) -> String {
        format!("{}-{}", self.date, self.slug)
    }
}

/// Assign a unique `.md` leaf filename to each input, in input order.
///
/// Inputs sharing a base name are disambiguated together by appending the
/// shortest equal-length ULID tail that makes them all distinct.
pub fn leaf_names(inputs: &[LeafInput]) -> Vec<String> {
    let bases: Vec<(Id, String)> = inputs
        .iter()
        .map(|input| (input.id, input.base()))
        .collect();
    disambiguate(&bases)
        .into_iter()
        .map(|name| format!("{name}.md"))
        .collect()
}

/// Make each `(id, base)` name unique, in input order, compared without
/// regard to case (ADRs 0023, 0068).
///
/// First, a base that occurs once stays as it is, and the bases that collide
/// get `-<tail>` appended, the tail being the shortest equal-length slice of
/// the colliders' ULIDs that tells them apart. A tail can still produce a
/// name that equals another note's, `review` with the tail `123` against a
/// slug `review-123`, or differs from it only in case, which a
/// case-insensitive filesystem stores as one entry. So, second, while two
/// names are equal ignoring case, every base group holding one of them grows
/// its tail by one character, a plain name gaining a tail of three. Notes
/// with distinct ULIDs always end up with distinct names: two names at the
/// full ULID length that are equal ignoring case have equal bases and equal
/// ULIDs. The same rule names the site's note pages (ADR 0054).
pub fn disambiguate(inputs: &[(Id, String)]) -> Vec<String> {
    // Bucket input indices by their base name, ignoring case, so collisions
    // are handled per group while the output stays aligned to the input.
    let mut group_of = Vec::with_capacity(inputs.len());
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut by_base: HashMap<String, usize> = HashMap::new();
    for (i, (_, base)) in inputs.iter().enumerate() {
        let group = *by_base.entry(base.to_lowercase()).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(i);
        group_of.push(group);
    }

    let mut tail_lens: Vec<usize> = groups
        .iter()
        .map(|indices| match indices.len() {
            1 => 0,
            _ => disambiguating_tail_len(inputs, indices),
        })
        .collect();

    loop {
        let names: Vec<String> = inputs
            .iter()
            .zip(&group_of)
            .map(|((id, base), &group)| match tail_lens[group] {
                0 => base.clone(),
                len => format!("{base}-{}", id.tail(len)),
            })
            .collect();

        let mut first_holder: HashMap<String, usize> = HashMap::new();
        let mut colliding = BTreeSet::new();
        for (i, name) in names.iter().enumerate() {
            if let Some(&other) = first_holder.get(&name.to_lowercase()) {
                colliding.insert(group_of[other]);
                colliding.insert(group_of[i]);
            } else {
                first_holder.insert(name.to_lowercase(), i);
            }
        }

        // Only notes sharing a ULID can stay equal once every tail is the
        // full ULID; the scan never passes two of those (ADR 0067), and the
        // loop must end either way.
        let growable: Vec<usize> = colliding
            .into_iter()
            .filter(|&group| tail_lens[group] < ULID_LEN)
            .collect();
        if growable.is_empty() {
            return names;
        }
        for group in growable {
            tail_lens[group] = match tail_lens[group] {
                0 => MIN_TAIL,
                len => len + 1,
            };
        }
    }
}

/// Find the shortest tail length (from [`MIN_TAIL`] up to the full ULID) at
/// which the colliding notes' tails are all distinct.
///
/// Distinct ULIDs always become unique at full length, so the full width is the
/// guaranteed terminating fallback.
fn disambiguating_tail_len(inputs: &[(Id, String)], indices: &[usize]) -> usize {
    (MIN_TAIL..ULID_LEN)
        .find(|&len| {
            let mut tails: Vec<String> = indices.iter().map(|&i| inputs[i].0.tail(len)).collect();
            tails.sort();
            tails.windows(2).all(|w| w[0] != w[1])
        })
        .unwrap_or(ULID_LEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: &str, date: &str, slug: &str) -> LeafInput {
        LeafInput {
            id: id.parse().expect("valid ULID"),
            date: date.into(),
            slug: slug.into(),
        }
    }

    #[test]
    fn unique_bases_get_plain_names() {
        let inputs = vec![
            input("01ARZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-25", "alpha"),
            input("01BRZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-25", "beta"),
        ];
        assert_eq!(
            leaf_names(&inputs),
            vec!["2026-06-25-alpha.md", "2026-06-25-beta.md"]
        );
    }

    #[test]
    fn collision_appends_three_char_tail_to_all() {
        // Same date and slug, ULIDs differing within the last three chars.
        let inputs = vec![
            input("01ARZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-25", "review"),
            input("01ARZ3NDEKTSV4RRFFQ69G5FBW", "2026-06-25", "review"),
        ];
        let names = leaf_names(&inputs);
        assert_eq!(names[0], "2026-06-25-review-FAV.md");
        assert_eq!(names[1], "2026-06-25-review-FBW.md");
    }

    #[test]
    fn collision_grows_tail_until_unique() {
        // The last three chars are identical (`FAV`); disambiguation must grow
        // to five chars (`G5FAV` vs `X5FAV`) to separate them.
        let inputs = vec![
            input("01ARZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-25", "review"),
            input("01ARZ3NDEKTSV4RRFFQ69X5FAV", "2026-06-25", "review"),
        ];
        let names = leaf_names(&inputs);
        assert_eq!(names[0], "2026-06-25-review-G5FAV.md");
        assert_eq!(names[1], "2026-06-25-review-X5FAV.md");
    }

    #[test]
    fn disambiguate_touches_only_the_colliding_bases() {
        let id = |s: &str| -> Id { s.parse().expect("valid ULID") };
        let inputs = vec![
            (id("01ARZ3NDEKTSV4RRFFQ69G5FAV"), "review".to_string()),
            (id("01BRZ3NDEKTSV4RRFFQ69G5FAV"), "notes".to_string()),
            (id("01ARZ3NDEKTSV4RRFFQ69G5FBW"), "review".to_string()),
        ];
        assert_eq!(
            disambiguate(&inputs),
            vec!["review-FAV", "notes", "review-FBW"]
        );
    }

    #[test]
    fn a_disambiguated_name_never_equals_another_plain_name() {
        // `review` collides and gains the all-digit tail `123`; `review-123`
        // is another note's slug. Both tails grow until the names differ.
        let inputs = vec![
            input("01ARZ3NDEKTSV4RRFFQ69G5123", "2026-06-25", "review"),
            input("01ARZ3NDEKTSV4RRFFQ69G5FBW", "2026-06-25", "review"),
            input("01ARZ3NDEKTSV4RRFFQ69G5YYY", "2026-06-25", "review-123"),
        ];
        assert_eq!(
            leaf_names(&inputs),
            vec![
                "2026-06-25-review-5123.md",
                "2026-06-25-review-5FBW.md",
                "2026-06-25-review-123-YYY.md",
            ]
        );
    }

    #[test]
    fn names_differing_only_in_case_are_disambiguated() {
        // The tail `FAV` and a slug ending in `-fav` name one entry on a
        // case-insensitive filesystem such as default APFS.
        let inputs = vec![
            input("01ARZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-25", "review"),
            input("01ARZ3NDEKTSV4RRFFQ69G5FBW", "2026-06-25", "review"),
            input("01ARZ3NDEKTSV4RRFFQ69G5YYY", "2026-06-25", "review-fav"),
        ];
        assert_eq!(
            leaf_names(&inputs),
            vec![
                "2026-06-25-review-5FAV.md",
                "2026-06-25-review-5FBW.md",
                "2026-06-25-review-fav-YYY.md",
            ]
        );
    }

    #[test]
    fn a_slug_holding_a_whole_ulid_still_gets_a_unique_name() {
        // The site's page names have no date, so two ULIDs can differ only in
        // their first character and need the full 26-character tail. A plain
        // name equal to one of those cannot be fixed by growing that tail any
        // further, so the plain name gains a tail of its own.
        let id = |s: &str| -> Id { s.parse().expect("valid ULID") };
        let a = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let inputs = vec![
            (id(a), "review".to_string()),
            (id("71ARZ3NDEKTSV4RRFFQ69G5FAV"), "review".to_string()),
            (
                id("01BX5ZZKBKACTAV9WEVGEMMVRZ"),
                format!("review-{}", a.to_lowercase()),
            ),
        ];
        assert_eq!(
            disambiguate(&inputs),
            vec![
                format!("review-{a}"),
                "review-71ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
                format!("review-{}-VRZ", a.to_lowercase()),
            ]
        );
    }

    #[test]
    fn identical_ids_and_bases_end_without_unique_names() {
        // Only one note per ULID reaches leaf naming (ADR 0067); given two
        // anyway, the growth stops at the full ULID instead of looping.
        let inputs = vec![
            input("01ARZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-25", "review"),
            input("01ARZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-25", "review"),
        ];
        let names = leaf_names(&inputs);
        assert_eq!(names[0], names[1]);
    }

    #[test]
    fn different_dates_same_slug_do_not_collide() {
        let inputs = vec![
            input("01ARZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-25", "review"),
            input("01BRZ3NDEKTSV4RRFFQ69G5FAV", "2026-06-26", "review"),
        ];
        assert_eq!(
            leaf_names(&inputs),
            vec!["2026-06-25-review.md", "2026-06-26-review.md"]
        );
    }
}
