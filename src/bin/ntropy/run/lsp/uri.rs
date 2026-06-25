// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Conversion between LSP document URIs and filesystem paths.
//!
//! Editors identify documents by `file:` URIs with percent-encoded paths. ntropy
//! is Unix-only (ADR 0020), so a path is a byte string: percent-decoding yields
//! raw bytes that become an `OsString` directly, which also tolerates non-UTF-8
//! paths. Non-`file:` URIs have no filesystem path and yield `None`.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use lsp_types::Uri;

/// The filesystem path of a `file:` URI, or `None` for any other scheme.
pub fn to_path(uri: &Uri) -> Option<PathBuf> {
    let text = uri.as_str();
    let rest = text.strip_prefix("file://")?;
    // Drop an optional authority (e.g. `localhost`) before the absolute path.
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(index) => &rest[index..],
        None => return None,
    };
    Some(PathBuf::from(OsString::from_vec(percent_decode(path))))
}

/// Decode `%XX` escapes into raw bytes, leaving everything else as-is.
fn percent_decode(encoded: &str) -> Vec<u8> {
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2]))
        {
            out.push(hi * 16 + lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// The numeric value of a single hex digit, or `None`.
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn uri(text: &str) -> Uri {
        Uri::from_str(text).expect("valid uri")
    }

    #[test]
    fn plain_absolute_path() {
        assert_eq!(
            to_path(&uri("file:///Users/x/all-notes/a.md")),
            Some(PathBuf::from("/Users/x/all-notes/a.md"))
        );
    }

    #[test]
    fn percent_encoded_spaces_are_decoded() {
        assert_eq!(
            to_path(&uri("file:///Users/x/my%20notes/a.md")),
            Some(PathBuf::from("/Users/x/my notes/a.md"))
        );
    }

    #[test]
    fn non_ascii_directory_round_trips() {
        // "Übung" encoded as UTF-8 percent escapes.
        assert_eq!(
            to_path(&uri("file:///vault/%C3%9Cbung/a.md")),
            Some(PathBuf::from("/vault/Übung/a.md"))
        );
    }

    #[test]
    fn localhost_authority_is_dropped() {
        assert_eq!(
            to_path(&uri("file://localhost/vault/a.md")),
            Some(PathBuf::from("/vault/a.md"))
        );
    }

    #[test]
    fn non_file_scheme_is_none() {
        assert_eq!(to_path(&uri("untitled:Untitled-1")), None);
        assert_eq!(to_path(&uri("https://example.com/a.md")), None);
    }
}
