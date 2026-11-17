// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! File sizes as people write and read them: `1.2 MiB`, `25 MiB`, `512 KiB`.
//!
//! [`format_size`] prints the figure in the asset listings and in the PDF
//! attachment badge; [`parse_size`] reads the `[render] attach_max` setting.

/// A byte count as a short human-readable figure: whole bytes below a KiB,
/// otherwise one decimal in binary units, matching the sizes `ls -lh` shows.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64 / 1024.0;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

/// Parse a size written as a number and a unit (`B`, `KiB`, `MiB`, `GiB`),
/// with or without a space between them. A number without a unit is bytes.
/// Returns `None` for anything else, for a negative number and for a size
/// that does not fit in `u64`.
pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let number_end = text
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(number_end);
    let factor: u64 = match unit.trim_start() {
        "" | "B" => 1,
        "KiB" => 1 << 10,
        "MiB" => 1 << 20,
        "GiB" => 1 << 30,
        _ => return None,
    };
    if number.contains('.') {
        // A fractional figure such as the `1.5 GiB` that `format_size`
        // prints. f64 is exact enough at these magnitudes, and the cast
        // saturates, so the range check runs before it.
        let bytes = number.parse::<f64>().ok()? * factor as f64;
        return (bytes < u64::MAX as f64).then_some(bytes as u64);
    }
    number.parse::<u64>().ok()?.checked_mul(factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_picks_the_binary_unit() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(9), "9 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KiB");
        assert_eq!(format_size(12_700), "12.4 KiB");
        assert_eq!(format_size(1024 * 1024), "1.0 MiB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024 / 2), "1.5 GiB");
        assert_eq!(format_size(u64::MAX), "16777216.0 TiB");
    }

    #[test]
    fn parse_size_reads_each_unit_with_and_without_a_space() {
        assert_eq!(parse_size("512 B"), Some(512));
        assert_eq!(parse_size("512 KiB"), Some(512 * 1024));
        assert_eq!(parse_size("25 MiB"), Some(25 * 1024 * 1024));
        assert_eq!(parse_size("25MiB"), Some(25 * 1024 * 1024));
        assert_eq!(parse_size("1 GiB"), Some(1024 * 1024 * 1024));
    }

    #[test]
    fn parse_size_reads_a_bare_number_as_bytes() {
        assert_eq!(parse_size("2048"), Some(2048));
        assert_eq!(parse_size("0"), Some(0));
    }

    #[test]
    fn parse_size_accepts_the_figure_format_size_prints() {
        assert_eq!(parse_size("1.5 GiB"), Some(1536 * 1024 * 1024));
        assert_eq!(parse_size("1.0 KiB"), Some(1024));
    }

    #[test]
    fn parse_size_rejects_what_is_not_a_size() {
        for bad in [
            "",
            " ",
            "MiB",
            "25 megabytes",
            "25 mib",
            "-1 MiB",
            "1 2 MiB",
            "25 MiB x",
        ] {
            assert_eq!(parse_size(bad), None, "{bad:?} is not a size");
        }
    }

    #[test]
    fn parse_size_rejects_a_size_beyond_u64() {
        assert_eq!(parse_size("99999999999 GiB"), None);
        assert_eq!(parse_size("18446744073709551616"), None);
    }
}
