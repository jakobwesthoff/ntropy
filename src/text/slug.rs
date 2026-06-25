// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Title-to-slug normalization (ADR 0023).
//!
//! The slug is the lossy, filename-safe form of a note's title. The same
//! per-segment normalization is reused for tag segments and view grouping
//! values (ADR 0009), so the core pipeline is factored out as
//! [`normalize_segment`] and [`slugify`] only adds the `untitled` fallback for
//! the filename case.

/// The slug length cap, applied at a `-` boundary (ADR 0023 step 6).
const MAX_SLUG_LEN: usize = 72;

/// The fallback slug for a title that normalizes to nothing (ADR 0023 step 7).
pub const UNTITLED: &str = "untitled";

/// Normalize a title into a filename slug.
///
/// Runs the full pipeline and, unlike [`normalize_segment`], guarantees a
/// non-empty result by falling back to [`UNTITLED`]. This is the form used for
/// the `<slug>` in `<ulid>-<slug>.md`.
pub fn slugify(title: &str) -> String {
    let s = normalize_segment(title);
    if s.is_empty() {
        UNTITLED.to_string()
    } else {
        s
    }
}

/// Run the normalization pipeline, returning the possibly-empty result.
///
/// This is steps 1-6 of ADR 0023 without the `untitled` fallback, because tag
/// segments and view grouping keys must be able to come back empty (an empty
/// segment is dropped by the caller rather than turned into `untitled`).
pub fn normalize_segment(input: &str) -> String {
    // Step 1: German-aware (and best-effort Latin) ASCII transliteration.
    let transliterated: String = input.chars().map(transliterate).collect();

    // Step 2: lowercase.
    let lowercased = transliterated.to_lowercase();

    // Step 3: collapse whitespace runs to a single `-`. Doing this before the
    // character filter (step 4) is what turns spaces into separators rather
    // than deleting them and fusing adjacent words.
    let mut spaced = String::with_capacity(lowercased.len());
    let mut prev_was_space = false;
    for ch in lowercased.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                spaced.push('-');
            }
            prev_was_space = true;
        } else {
            spaced.push(ch);
            prev_was_space = false;
        }
    }

    // Step 4: drop anything outside `[a-z0-9-]`. Punctuation is removed, not
    // replaced, so `C++` becomes `c` and `a&b` (spaces already handled) keeps
    // its surrounding separators.
    let filtered: String = spaced
        .chars()
        .filter(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || *ch == '-')
        .collect();

    // Step 5: collapse consecutive `-` and trim leading/trailing `-`.
    let collapsed = collapse_dashes(&filtered);

    // Step 6: cap the length at a `-` boundary.
    truncate_at_boundary(&collapsed, MAX_SLUG_LEN)
}

/// Map a single character to its ASCII transliteration.
///
/// German umlauts and `ß` expand per ADR 0023; a focused set of common accented
/// Latin letters fold to their base letter as a best-effort transliteration.
/// Any other non-ASCII character maps to the empty string and is thereby
/// dropped. ASCII passes through unchanged (later steps handle casing and
/// filtering).
fn transliterate(ch: char) -> String {
    match ch {
        // German, with explicit two-letter expansions.
        'ä' => "ae".into(),
        'ö' => "oe".into(),
        'ü' => "ue".into(),
        'ß' => "ss".into(),
        'Ä' => "Ae".into(),
        'Ö' => "Oe".into(),
        'Ü' => "Ue".into(),
        // Best-effort Latin accent folding.
        'à' | 'á' | 'â' | 'ã' | 'å' | 'ā' | 'ª' => "a".into(),
        'À' | 'Á' | 'Â' | 'Ã' | 'Å' | 'Ā' => "A".into(),
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' => "e".into(),
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' => "E".into(),
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' => "i".into(),
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ī' => "I".into(),
        'ò' | 'ó' | 'ô' | 'õ' | 'ø' | 'ō' => "o".into(),
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ø' | 'Ō' => "O".into(),
        'ù' | 'ú' | 'û' | 'ū' | 'ů' => "u".into(),
        'Ù' | 'Ú' | 'Û' | 'Ū' => "U".into(),
        'ñ' => "n".into(),
        'Ñ' => "N".into(),
        'ç' => "c".into(),
        'Ç' => "C".into(),
        'ý' | 'ÿ' => "y".into(),
        other if other.is_ascii() => other.to_string(),
        _ => String::new(),
    }
}

/// Collapse runs of `-` to one and trim them from both ends.
fn collapse_dashes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = false;
    for ch in input.chars() {
        if ch == '-' {
            if !prev_dash {
                out.push('-');
            }
            prev_dash = true;
        } else {
            out.push(ch);
            prev_dash = false;
        }
    }
    out.trim_matches('-').to_string()
}

/// Truncate to at most `max` characters, preferring to cut at a `-` so a word
/// is not left half-written. Everything here is ASCII, so byte and character
/// length coincide.
fn truncate_at_boundary(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    let window = &input[..max];
    let cut = match window.rfind('-') {
        Some(idx) => &window[..idx],
        None => window,
    };
    cut.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_lowercase_and_hyphenation() {
        assert_eq!(slugify("My First Note"), "my-first-note");
    }

    #[test]
    fn german_umlauts_and_eszett() {
        assert_eq!(slugify("Über Größe und Spaß"), "ueber-groesse-und-spass");
        assert_eq!(slugify("Äpfel"), "aepfel");
    }

    #[test]
    fn best_effort_latin_accents() {
        assert_eq!(slugify("Café résumé naïve"), "cafe-resume-naive");
        assert_eq!(slugify("Señor Niño"), "senor-nino");
    }

    #[test]
    fn uppercase_transliteration_folds_then_lowercases() {
        // Uppercase German and accented Latin transliterate before the
        // lowercase step, so they fold to the same forms as their lowercase
        // counterparts.
        assert_eq!(slugify("ÄÖÜ"), "aeoeue");
        assert_eq!(slugify("ÀÉÎÕÚ Ñ Ç"), "aeiou-n-c");
    }

    #[test]
    fn punctuation_is_removed_not_replaced() {
        assert_eq!(slugify("C++ vs. Rust!"), "c-vs-rust");
        assert_eq!(slugify("a & b"), "a-b");
    }

    #[test]
    fn collapses_and_trims_dashes() {
        assert_eq!(slugify("  --Hello---World--  "), "hello-world");
    }

    #[test]
    fn whitespace_runs_collapse() {
        assert_eq!(slugify("a\t\n   b"), "a-b");
    }

    #[test]
    fn empty_falls_back_to_untitled() {
        assert_eq!(slugify(""), UNTITLED);
        assert_eq!(slugify("   "), UNTITLED);
        assert_eq!(slugify("!!!"), UNTITLED);
    }

    #[test]
    fn non_ascii_unmapped_is_dropped() {
        // CJK, emoji and similar have no ASCII transliteration and drop out.
        assert_eq!(slugify("日本語 notes 🚀"), "notes");
        assert_eq!(slugify("中文"), UNTITLED);
    }

    #[test]
    fn digits_are_preserved() {
        assert_eq!(slugify("Plan for 2026 Q3"), "plan-for-2026-q3");
    }

    #[test]
    fn length_cap_truncates_at_dash_boundary() {
        let title =
            "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron";
        let slug = slugify(title);
        assert!(slug.len() <= MAX_SLUG_LEN);
        // The cut lands on a word boundary, so the slug never ends mid-word and
        // never ends with a dash.
        assert!(!slug.ends_with('-'));
        insta::assert_snapshot!(slug, @"alpha-beta-gamma-delta-epsilon-zeta-eta-theta-iota-kappa-lambda-mu-nu");
    }

    #[test]
    fn length_cap_single_long_token_is_hard_cut() {
        let title = "a".repeat(100);
        let slug = slugify(&title);
        assert_eq!(slug.len(), MAX_SLUG_LEN);
    }

    #[test]
    fn normalize_segment_can_return_empty() {
        // Unlike `slugify`, the segment form does not substitute `untitled`.
        assert_eq!(normalize_segment(""), "");
        assert_eq!(normalize_segment("///"), "");
    }
}
