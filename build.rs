// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Embeds the site's asset directories. Every file under each directory in
//! [`EMBEDDED`] becomes one entry of a generated table, so a file dropped
//! into a directory ships in the binary without being listed anywhere. Each
//! file is zstd-compressed first and stored compressed when that is
//! smaller, raw otherwise (fonts and the grammar blob are compressed
//! formats already); `crate::site::embedded::Asset` inflates on demand.
//!
//! The encoder is `ruzstd`, a build dependency only: it never links into
//! the binary, which decodes with the same crate.

use std::path::{Path, PathBuf};

use ruzstd::encoding::{CompressionLevel, compress_to_vec};

/// The directories to embed and the table each one generates into
/// `OUT_DIR`, as `(directory relative to the manifest, table file)`.
const EMBEDDED: &[(&str, &str)] = &[
    ("src/site/theme", "site_theme_files.rs"),
    ("src/site/dist", "site_dist_files.rs"),
];

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets it"));
    for (dir, table) in EMBEDDED {
        let blobs = out.join("embedded").join(table.trim_end_matches(".rs"));
        std::fs::create_dir_all(&blobs).expect("the build script can create OUT_DIR dirs");
        let source = embed(&manifest.join(dir), &blobs);
        std::fs::write(out.join(table), source).expect("the build script can write into OUT_DIR");
    }
}

/// The table for one directory: the Rust source of a `&[Asset]` slice,
/// entries sorted by path. Compressed blobs are written under `blobs`.
fn embed(dir: &Path, blobs: &Path) -> String {
    let mut files = Vec::new();
    collect(dir, dir, &mut files);
    files.sort();

    let mut table = String::from("&[\n");
    for (relative, absolute) in &files {
        let raw = std::fs::read(absolute).expect("a collected file is readable");
        let compressed = compress_to_vec(&raw[..], CompressionLevel::Fastest);
        let (stored, path) = if compressed.len() < raw.len() {
            let blob = blobs.join(format!("{}.zst", relative.replace('/', "__")));
            std::fs::write(&blob, &compressed).expect("the build script can write a blob");
            (true, blob)
        } else {
            (false, absolute.clone())
        };
        table.push_str(&format!(
            "    crate::site::embedded::Asset {{ path: {relative:?}, compressed: {stored}, bytes: include_bytes!({:?}) }},\n",
            path.display()
        ));
    }
    table.push(']');
    table
}

/// Every regular file under `dir`, recursively, as `(relative path with `/`
/// separators, absolute path)`. Hidden entries (`.DS_Store` and the like)
/// are left out. Each visited directory and file is registered with cargo,
/// so adding, removing, or editing a file rebuilds its table.
fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let entries = std::fs::read_dir(dir).expect("an embedded directory is readable");
    for entry in entries {
        let path = entry.expect("a directory entry is readable").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("embedded file names are UTF-8");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(base, &path, out);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
            let relative = path
                .strip_prefix(base)
                .expect("walked paths lie under the base")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, path));
        }
    }
}
