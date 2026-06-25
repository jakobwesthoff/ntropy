// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Rendering derived dates.
//!
//! A note's `created` instant comes from its ULID and is a UTC instant
//! (ADR 0005). It is rendered to a readable `YYYY-MM-DD` in the machine's
//! system-local timezone, both for plain output and for view leaf names
//! (ADR 0010). Sorting and filtering use the UTC instant directly and never
//! pass through here.

use jiff::Timestamp;
use jiff::tz::TimeZone;

/// A failure converting or rendering a derived date.
///
/// In practice only an out-of-range timestamp can trigger this; a ULID encodes
/// a 48-bit millisecond field whose far end lies beyond jiff's supported year
/// range. Surfacing it as an error keeps a corrupt id from panicking a scan.
#[derive(Debug, thiserror::Error)]
#[error("while rendering the date for timestamp {millis}ms")]
pub struct DateError {
    millis: i64,
    #[source]
    source: jiff::Error,
}

/// Render a UTC millisecond instant as `YYYY-MM-DD` in the system-local
/// timezone.
pub fn render_local_date(timestamp_ms: u64) -> Result<String, DateError> {
    render_in_zone(timestamp_ms, &TimeZone::system())
}

/// Render in an explicit timezone.
///
/// Split out from [`render_local_date`] so the date arithmetic, including the
/// near-midnight day-boundary behavior called out in ADR 0010, is unit-testable
/// without depending on the host's configured zone.
fn render_in_zone(timestamp_ms: u64, tz: &TimeZone) -> Result<String, DateError> {
    let millis = timestamp_ms as i64;
    let timestamp =
        Timestamp::from_millisecond(millis).map_err(|source| DateError { millis, source })?;
    let zoned = timestamp.to_zoned(tz.clone());
    Ok(zoned.strftime("%Y-%m-%d").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `2026-06-25T12:00:00Z` in milliseconds: a midday instant that renders to
    /// the same calendar day in any of the zones tested below.
    const MIDDAY_2026_06_25: u64 = 1_782_388_800_000;

    #[test]
    fn renders_utc_date() {
        let out = render_in_zone(MIDDAY_2026_06_25, &TimeZone::UTC).expect("render");
        assert_eq!(out, "2026-06-25");
    }

    #[test]
    fn epoch_is_1970() {
        assert_eq!(
            render_in_zone(0, &TimeZone::UTC).expect("render"),
            "1970-01-01"
        );
    }

    #[test]
    fn near_midnight_shifts_with_zone() {
        // 2026-06-25T01:30:00Z is still the 25th in UTC but the 24th in a zone
        // three hours behind. ADR 0010 accepts this display-only timezone
        // sensitivity for near-midnight notes.
        let near_midnight = 1_782_351_000_000;
        let utc = render_in_zone(near_midnight, &TimeZone::UTC).expect("utc");
        let behind = TimeZone::get("America/New_York").expect("tz db");
        let local = render_in_zone(near_midnight, &behind).expect("local");
        assert_eq!(utc, "2026-06-25");
        assert_eq!(local, "2026-06-24");
    }

    #[test]
    fn system_zone_renders_without_error() {
        // The exact value depends on the host zone; we only assert it produces
        // a well-formed date string.
        let out = render_local_date(MIDDAY_2026_06_25).expect("render");
        assert_eq!(out.len(), "YYYY-MM-DD".len());
        assert_eq!(out.matches('-').count(), 2);
    }
}
