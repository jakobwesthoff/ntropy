// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The library's public surface as an external crate sees it: every type in
//! a public signature can be named from outside.

use ntropy::error::Error;

#[test]
fn fs_error_is_nameable_through_the_crate_root() {
    // A vault under a regular file cannot be created: the directory creation
    // fails, which surfaces as the filesystem variant.
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("not-a-directory");
    std::fs::write(&file, b"x").expect("seed file");

    let err = ntropy::ops::init_vault(&file.join("vault")).expect_err("not a directory");
    let fs_error: &ntropy::FsError = match &err {
        Error::Fs(fs_error) => fs_error,
        other => panic!("expected a filesystem error, got {other:?}"),
    };
    assert!(
        fs_error.to_string().contains("not-a-directory"),
        "the error names the path: {fs_error}"
    );
}
