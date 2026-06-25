// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! View leaf naming and collision disambiguation (ADRs 0009, 0023).
//!
//! A leaf symlink is named `<date>-<slug>.md`, where `<date>` is the readable
//! creation date and `<slug>` derives from the note title. When two notes in
//! the same view group would collide on that base name, a trailing slice of
//! each colliding note's ULID is appended to every collider. The slice starts
//! at 3 characters and grows until all collisions are resolved, up to the full
//! 26-character ULID; the *trailing* portion is used because same-day
//! collisions share the leading timestamp characters of their ULIDs.

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
    // Bucket input indices by their base name so collisions are handled
    // per-group while the output stays aligned to the original order.
    let mut by_base: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, input) in inputs.iter().enumerate() {
        by_base.entry(input.base()).or_default().push(i);
    }

    let mut names = vec![String::new(); inputs.len()];
    for (base, indices) in by_base {
        if indices.len() == 1 {
            names[indices[0]] = format!("{base}.md");
            continue;
        }

        let tail_len = disambiguating_tail_len(inputs, &indices);
        for &i in &indices {
            names[i] = format!("{base}-{}.md", inputs[i].id.tail(tail_len));
        }
    }
    names
}

/// Find the shortest tail length (from [`MIN_TAIL`] up to the full ULID) at
/// which the colliding notes' tails are all distinct.
///
/// Distinct ULIDs always become unique at full length, so the full width is the
/// guaranteed terminating fallback.
fn disambiguating_tail_len(inputs: &[LeafInput], indices: &[usize]) -> usize {
    (MIN_TAIL..ULID_LEN)
        .find(|&len| {
            let mut tails: Vec<String> = indices.iter().map(|&i| inputs[i].id.tail(len)).collect();
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
