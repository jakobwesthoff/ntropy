// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! `ntropy skill`: the agent skill compiled into the binary, printed whole,
//! by part, or installed into a skills directory (ADR 0092).
//!
//! The binary's output is compared with the files under `skills/ntropy/`,
//! read at run time, so a text that drifted from the repository fails here.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use insta_cmd::assert_cmd_snapshot;

/// The parts of the skill by the name `ntropy skill <part>` takes, each
/// `references/<name>.md` beside `SKILL.md`.
const PARTS: [&str; 7] = [
    "writing-notes",
    "querying",
    "vaults",
    "views",
    "site",
    "site-themes",
    "document-themes",
];

/// A global config directory that does not exist, as `tests/cli.rs` uses,
/// so no test sees the host's configuration and every snapshot records the
/// same value.
const NO_CONFIG_DIR: &str = "no-such-config-dir";

/// A directory to run in. Nothing in it decides a test.
fn empty_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("the temp dir is writable")
}

/// `ntropy` run in `cwd`, isolated from the host's vault selection and
/// global config.
fn ntropy(cwd: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(cwd);
    cmd.env_remove("NTROPY_VAULT");
    cmd.env("NTROPY_CONFIG_DIR", NO_CONFIG_DIR);
    cmd
}

fn skill_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/ntropy")
}

/// A file of the skill as the repository holds it.
fn repository_file(relative: &str) -> String {
    let file = skill_dir().join(relative);
    fs::read_to_string(&file)
        .unwrap_or_else(|error| panic!("{} is part of the skill: {error}", file.display()))
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("the skill is UTF-8")
}

// =========================================================
// Printing
// =========================================================

#[test]
fn prints_the_core_of_the_skill() {
    let dir = empty_dir();

    let output = ntropy(dir.path()).arg("skill").output().expect("a run");

    assert!(output.status.success());
    assert_eq!(stdout_of(&output), repository_file("SKILL.md"));
    assert_eq!(output.stderr, b"");
}

#[test]
fn prints_each_part_of_the_skill() {
    let dir = empty_dir();

    for name in PARTS {
        let output = ntropy(dir.path())
            .args(["skill", name])
            .output()
            .expect("a run");

        assert!(output.status.success(), "{name}");
        assert_eq!(
            stdout_of(&output),
            repository_file(&format!("references/{name}.md")),
            "{name}"
        );
    }
}

#[test]
fn skill_resolves_no_vault() {
    let dir = empty_dir();

    let output = ntropy(dir.path())
        .args(["--vault", "no-such-vault", "skill"])
        .output()
        .expect("a run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
}

#[test]
fn an_unknown_part_is_a_usage_error_that_lists_the_parts() {
    let dir = empty_dir();

    assert_cmd_snapshot!(ntropy(dir.path()).args(["skill", "no-such-part"]));
}

#[test]
fn a_part_and_install_together_are_a_usage_error() {
    let dir = empty_dir();

    assert_cmd_snapshot!(ntropy(dir.path()).args(["skill", "views", "--install", "skills-root"]));
}

// =========================================================
// Installing
// =========================================================

#[test]
fn install_writes_the_core_and_every_part_and_names_each_file() {
    let dir = empty_dir();

    assert_cmd_snapshot!(ntropy(dir.path()).args(["skill", "--install", "skills-root"]));

    let installed = dir.path().join("skills-root/ntropy");
    assert_eq!(
        fs::read_to_string(installed.join("SKILL.md")).expect("the core is installed"),
        repository_file("SKILL.md")
    );
    for name in PARTS {
        let relative = format!("references/{name}.md");
        assert_eq!(
            fs::read_to_string(installed.join(&relative)).expect("the part is installed"),
            repository_file(&relative),
            "{name}"
        );
    }
}

#[test]
fn install_replaces_the_references_and_keeps_other_files() {
    let dir = empty_dir();
    let installed = dir.path().join("skills-root/ntropy");
    fs::create_dir_all(installed.join("references")).expect("a creatable directory");
    fs::write(installed.join("references/retired.md"), "old").expect("a writable file");
    fs::write(installed.join("notes.txt"), "mine").expect("a writable file");

    let output = ntropy(dir.path())
        .args(["skill", "--install", "skills-root"])
        .output()
        .expect("a run");

    assert!(output.status.success());
    assert!(!installed.join("references/retired.md").exists());
    assert_eq!(
        fs::read_to_string(installed.join("notes.txt")).expect("the file stays"),
        "mine"
    );
}

#[test]
fn install_takes_an_absolute_directory() {
    let dir = empty_dir();
    let target = empty_dir();

    let output = ntropy(dir.path())
        .arg("skill")
        .arg("--install")
        .arg(target.path())
        .output()
        .expect("a run");

    assert!(output.status.success());
    assert!(target.path().join("ntropy/SKILL.md").is_file());
}

#[test]
fn install_into_a_file_is_an_error_that_names_the_path() {
    let dir = empty_dir();
    fs::write(dir.path().join("skills-root"), "in the way").expect("a writable file");

    assert_cmd_snapshot!(ntropy(dir.path()).args(["skill", "--install", "skills-root"]));
}
