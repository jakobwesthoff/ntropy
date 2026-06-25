// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! CLI contract tests: run the real binary and snapshot stdout, stderr and exit
//! code with `insta-cmd` (ADR 0021).
//!
//! Only non-interactive paths are exercised; the picker and editor TUIs are
//! validated manually (ADR 0021). Snapshots redact the temporary vault path and
//! ULIDs so they are stable across runs.

use std::fs;
use std::path::Path;
use std::process::Command;

use insta_cmd::assert_cmd_snapshot;

const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

/// Build a vault directly (faster and more deterministic than running `init`),
/// with a `by-tag` view configured.
fn setup_vault() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("all-notes")).expect("all-notes");
    fs::create_dir_all(root.join(".ntropy")).expect(".ntropy");
    fs::write(
        root.join(".ntropy/config.toml"),
        "[[view]]\nname = \"by-tag\"\nfield = \"tags\"\n",
    )
    .expect("config");
    dir
}

fn write_note(vault: &Path, ulid: &str, slug: &str, content: &str) {
    fs::write(
        vault.join("all-notes").join(format!("{ulid}-{slug}.md")),
        content,
    )
    .expect("write note");
}

/// A `ntropy` command run from inside `vault`, so the vault resolves by cwd
/// walk-up and its (temp) path never enters the snapshotted argument list.
/// `EDITOR=true` makes any editor launch a no-op.
fn ntropy(vault: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(vault);
    cmd.env("EDITOR", "true");
    cmd.env_remove("VISUAL");
    cmd.env_remove("NTROPY_VAULT");
    cmd
}

/// Snapshot settings that redact the vault path and any ULIDs.
fn redacted(vault: &Path) -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    if let Ok(canon) = fs::canonicalize(vault) {
        settings.add_filter(&regex::escape(&canon.to_string_lossy()), "[VAULT]");
    }
    settings.add_filter(&regex::escape(&vault.to_string_lossy()), "[VAULT]");
    settings.add_filter(r"[0-9A-HJKMNP-TV-Z]{26}", "[ULID]");
    // Derived dates render in the local timezone (ADR 0010), so redact them to
    // keep snapshots stable across machines.
    settings.add_filter(r"\d{4}-\d{2}-\d{2}", "[DATE]");
    settings
}

#[test]
fn bare_invocation_prints_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.env_remove("NTROPY_VAULT");
    assert_cmd_snapshot!(cmd);
}

#[test]
fn init_creates_and_is_idempotent() {
    let dir = tempfile::tempdir().expect("temp dir");

    // Run from the temp dir and create a relative `vault/` subdir, so neither
    // the args nor the printed path carry the (variable) temp path.
    let invoke = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
        cmd.current_dir(dir.path());
        cmd.args(["init", "vault"]);
        cmd.env_remove("NTROPY_VAULT");
        cmd
    };

    assert_cmd_snapshot!("init_first", invoke());
    assert_cmd_snapshot!("init_second", invoke());
}

#[test]
fn init_uses_vault_flag_when_path_omitted() {
    let dir = tempfile::tempdir().expect("temp dir");

    // Run from the temp dir with a relative `--vault`, so the printed path is
    // stable and the cwd (which has no vault) is clearly not the target.
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(dir.path());
    cmd.args(["--vault", "via-flag", "init"]);
    cmd.env_remove("NTROPY_VAULT");
    assert_cmd_snapshot!(cmd);

    assert!(dir.path().join("via-flag/.ntropy").exists());
    // The cwd itself must not have become a vault.
    assert!(!dir.path().join(".ntropy").exists());
}

#[test]
fn init_rejects_path_and_vault_together() {
    let dir = tempfile::tempdir().expect("temp dir");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(dir.path());
    cmd.args(["--vault", "from-flag", "init", "from-arg"]);
    cmd.env_remove("NTROPY_VAULT");
    assert_cmd_snapshot!(cmd);

    // Neither candidate target is created on the conflict.
    assert!(!dir.path().join("from-flag").exists());
    assert!(!dir.path().join("from-arg").exists());
}

#[test]
fn new_no_edit_prints_path() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "My First Note", "--no-edit"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn search_lists_all_notes_newest_first() {
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "older",
        "---\ntitle: Older\n---\nbody\n",
    );
    write_note(
        dir.path(),
        ULID_B,
        "newer",
        "---\ntitle: Newer\n---\nbody\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn search_filters_by_tag() {
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "work",
        "---\ntitle: Work Note\ntags: [area/work]\n---\nbody\n",
    );
    write_note(
        dir.path(),
        ULID_B,
        "home",
        "---\ntitle: Home Note\ntags: [area/home]\n---\nbody\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "tag:area/work", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn search_full_text() {
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "a",
        "---\ntitle: A\n---\nthe deadline is friday\n",
    );
    write_note(
        dir.path(),
        ULID_B,
        "b",
        "---\ntitle: B\n---\nnothing here\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "text:deadline", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn tags_lists_counts() {
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "a",
        "---\ntitle: A\ntags: [area/work, programming/rust]\n---\n",
    );
    write_note(
        dir.path(),
        ULID_B,
        "b",
        "---\ntitle: B\ntags: [area/work]\n---\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.arg("tags");
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn view_crud() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut add = ntropy(dir.path());
        add.args(["view", "add", "by-status", "--field", "status"]);
        assert_cmd_snapshot!("view_add", add);

        let mut list = ntropy(dir.path());
        list.args(["view", "list"]);
        assert_cmd_snapshot!("view_list", list);

        let mut remove = ntropy(dir.path());
        remove.args(["view", "remove", "by-status"]);
        assert_cmd_snapshot!("view_remove", remove);
    });
}

#[test]
fn delete_with_force() {
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "doomed", "---\ntitle: Doomed\n---\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["delete", ULID_A, "-f"]);
        assert_cmd_snapshot!(cmd);
    });
    assert!(
        !dir.path()
            .join(format!("all-notes/{ULID_A}-doomed.md"))
            .exists()
    );
}

#[test]
fn malformed_note_warns_but_continues() {
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "good", "---\ntitle: Good\n---\n");
    // Missing title: skipped with a warning.
    write_note(dir.path(), ULID_B, "bad", "---\ntags: [x]\n---\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn strict_makes_malformed_fatal() {
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "good", "---\ntitle: Good\n---\n");
    write_note(dir.path(), ULID_B, "bad", "---\ntags: [x]\n---\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n", "--strict"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn ambiguous_edit_piped_errors() {
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "alpha",
        "---\ntitle: Alpha\ntags: [work]\n---\n",
    );
    write_note(
        dir.path(),
        ULID_B,
        "beta",
        "---\ntitle: Beta\ntags: [work]\n---\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["edit", "tag:work", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn delete_non_interactive_without_force_refuses() {
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "keep", "---\ntitle: Keep\n---\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["delete", ULID_A, "-n"]);
        assert_cmd_snapshot!(cmd);
    });
    // The note is untouched.
    assert!(
        dir.path()
            .join(format!("all-notes/{ULID_A}-keep.md"))
            .exists()
    );
}

#[test]
fn reconcile_renames_and_reports() {
    let dir = setup_vault();
    // The on-disk slug `old` no longer matches the title `Brand New`.
    write_note(
        dir.path(),
        ULID_A,
        "old",
        "---\ntitle: Brand New\ntags: [x]\n---\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.arg("reconcile");
        assert_cmd_snapshot!(cmd);
    });
    assert!(
        dir.path()
            .join(format!("all-notes/{ULID_A}-brand-new.md"))
            .exists()
    );
}

#[test]
fn reconcile_noop_prints_summary() {
    let dir = setup_vault();
    // An aligned note: nothing to rename, but the summary still prints.
    write_note(dir.path(), ULID_A, "aligned", "---\ntitle: Aligned\n---\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.arg("reconcile");
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn edit_no_match_errors() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["edit", ULID_A, "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn delete_no_match_errors() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["delete", "tag:nonexistent", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn query_parse_error_is_reported() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "tag:", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}
