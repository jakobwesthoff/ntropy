// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Assets the build script embeds (`build.rs`): every file under an
//! embedded directory, zstd-compressed where that made it smaller. The
//! tables live in `OUT_DIR` and are included by the modules that own the
//! directories; this is the entry type they share and its inflation.

use std::io::Read;

/// One embedded file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    /// The path relative to the embedded directory, with `/` separators.
    pub path: &'static str,
    /// Whether `bytes` is a zstd frame of the file rather than the file.
    pub compressed: bool,
    pub bytes: &'static [u8],
}

impl Asset {
    /// The file's contents, inflated when stored compressed. The frame was
    /// written by the build script from the file, so decoding cannot fail
    /// short of a broken build.
    pub fn contents(&self) -> Vec<u8> {
        if !self.compressed {
            return self.bytes.to_vec();
        }
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(self.bytes)
            .expect("the build script embeds valid zstd frames");
        let mut out = Vec::new();
        decoder
            .read_to_end(&mut out)
            .expect("an embedded frame inflates completely");
        out
    }

    /// The file's contents as text, for the assets that are text.
    pub fn text(&self) -> String {
        String::from_utf8(self.contents()).expect("the embedded text assets are UTF-8")
    }
}

/// The asset at `path` within `table`.
pub fn find<'a>(table: &'a [Asset], path: &str) -> Option<&'a Asset> {
    table.iter().find(|asset| asset.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_assets_pass_through_and_lookup_is_by_path() {
        static TABLE: &[Asset] = &[Asset {
            path: "a/b.txt",
            compressed: false,
            bytes: b"plain",
        }];
        assert_eq!(find(TABLE, "a/b.txt").expect("found").contents(), b"plain");
        assert_eq!(find(TABLE, "a/b.txt").expect("found").text(), "plain");
        assert!(find(TABLE, "no-such-file").is_none());
    }

    /// The theme table the build script generated holds both kinds: the
    /// stylesheet compresses, a woff2 font does not.
    #[test]
    fn the_generated_theme_table_inflates_both_kinds() {
        let table = crate::site::theme::BUILTIN_FILES;
        let css = find(table, "style.css").expect("the stylesheet is embedded");
        assert!(css.compressed, "text compresses");
        assert!(css.text().contains(":root"));
        let font = table
            .iter()
            .find(|asset| asset.path.ends_with(".woff2"))
            .expect("a font is embedded");
        assert!(!font.compressed, "woff2 is compressed already");
        assert_eq!(&font.contents()[..4], b"wOF2");
    }
}
