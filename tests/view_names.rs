// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! CLI contract tests for view names that must never become paths (ADR 0064).
//!
//! A view's directory is emptied of everything that is not one of its links,
//! so these tests configure names that point elsewhere and check that every
//! command leaves the target alone and says why. Each vault sits below its
//! test's own temporary directory, with the decoy beside it, so even a
//! regression can only delete files the test created. Every invocation
//! passes `--vault`, so vault discovery can never reach a real vault.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use insta_cmd::assert_cmd_snapshot;

const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

/// A vault at `<base>/<at>` whose config holds the given views verbatim.
fn vault_at(base: &Path, at: &str, views: &[(&str, &str)]) -> PathBuf {
    let root = base.join(at);
    fs::create_dir_all(root.join("all-notes")).expect("all-notes");
    fs::create_dir_all(root.join(".ntropy/templates")).expect(".ntropy");
    fs::write(
        root.join(".ntropy/templates/today.md"),
        "---\ntitle: {{title}}\n---\n",
    )
    .expect("today template");
    let config: String = views
        .iter()
        .map(|(name, field)| format!("[[view]]\nname = \"{name}\"\nfield = \"{field}\"\n"))
        .collect();
    fs::write(root.join(".ntropy/config.toml"), config).expect("config");
    root
}

/// Plant a file a sync must never touch at `<dir>/sub/keep.txt`.
fn plant_decoy(guard: &tempfile::TempDir, dir: &Path) -> PathBuf {
    let victim = dir.join("sub").join("keep.txt");
    assert!(
        victim.starts_with(guard.path()),
        "the decoy must stay in the temp dir"
    );
    fs::create_dir_all(victim.parent().expect("the decoy has a parent")).expect("decoy dir");
    fs::write(&victim, "keep me").expect("decoy");
    victim
}

fn write_note(vault: &Path, content: &str) {
    fs::write(
        vault.join("all-notes").join(format!("{ULID_A}-note.md")),
        content,
    )
    .expect("write note");
}

/// An `ntropy` command pinned to `vault`, run from inside it, with no editor,
/// no inherited vault and no inherited identity.
fn ntropy(vault: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(vault);
    // Relative to the working directory, which is the vault: it pins the
    // vault all the same and keeps the temp path out of the snapshots.
    cmd.args(["--vault", "."]);
    cmd.env("EDITOR", "true");
    cmd.env_remove("VISUAL");
    cmd.env_remove("NTROPY_VAULT");
    cmd.env_remove("NTROPY_IDENTITY");
    cmd
}

fn run(cmd: &mut Command) -> Output {
    cmd.output().expect("run ntropy")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Snapshot settings that redact the temporary paths.
fn redacted(base: &Path) -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    if let Ok(canon) = fs::canonicalize(base) {
        settings.add_filter(&regex::escape(&canon.to_string_lossy()), "[BASE]");
    }
    settings.add_filter(&regex::escape(&base.to_string_lossy()), "[BASE]");
    settings
}

const SKIPPED_DECOY: &str =
    "warning: skipped view `../decoy`: the segment `..` starts with a dot (try `decoy`)";

#[test]
fn reconcile_skips_a_traversing_view_and_warns() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("../decoy", "tags")]);
    let victim = plant_decoy(&dir, &dir.path().join("decoy"));

    redacted(dir.path()).bind(|| {
        assert_cmd_snapshot!(ntropy(&vault).arg("reconcile"));
    });
    assert!(victim.exists(), "the decoy was deleted");
}

#[test]
fn strict_reconcile_fails_on_a_skipped_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("By_Tag", "tags")]);

    let output = run(ntropy(&vault).args(["reconcile", "--strict"]));
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("skipped view `By_Tag`"));
}

#[test]
fn today_warns_and_keeps_the_decoy() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("../decoy", "tags")]);
    let victim = plant_decoy(&dir, &dir.path().join("decoy"));

    let output = run(ntropy(&vault).args(["today", "--print", "-n"]));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(victim.exists(), "the decoy was deleted");
    assert!(
        stderr(&output).contains(SKIPPED_DECOY),
        "{}",
        stderr(&output)
    );
}

#[test]
fn write_warns_and_keeps_the_decoy() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("../decoy", "tags")]);
    write_note(&vault, "---\ntitle: Note\n---\n");
    let victim = plant_decoy(&dir, &dir.path().join("decoy"));

    let mut child = ntropy(&vault)
        .args(["write", ULID_A])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ntropy");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(b"---\ntitle: Note\ntags: [work]\n---\nbody\n")
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait for ntropy");

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(victim.exists(), "the decoy was deleted");
    assert!(
        stderr(&output).contains(SKIPPED_DECOY),
        "{}",
        stderr(&output)
    );
}

#[test]
fn delete_warns_and_keeps_the_decoy() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("../decoy", "tags")]);
    write_note(&vault, "---\ntitle: Note\n---\n");
    let victim = plant_decoy(&dir, &dir.path().join("decoy"));

    let output = run(ntropy(&vault).args(["delete", ULID_A, "-f", "-n"]));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(victim.exists(), "the decoy was deleted");
    assert!(
        stderr(&output).contains(SKIPPED_DECOY),
        "{}",
        stderr(&output)
    );
}

#[test]
fn view_add_refuses_a_traversing_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[]);

    redacted(dir.path()).bind(|| {
        assert_cmd_snapshot!(ntropy(&vault).args(["view", "add", "../escape", "--field", "tags"]));
    });
    assert!(!dir.path().join("escape").exists());
}

#[test]
fn view_add_suggests_a_valid_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[]);

    let output = run(ntropy(&vault).args(["view", "add", "By_Tag", "--field", "tags"]));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("(try `by-tag`)"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn view_add_reports_another_skipped_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("../decoy", "tags")]);
    let victim = plant_decoy(&dir, &dir.path().join("decoy"));

    let output = run(ntropy(&vault).args(["view", "add", "bytag", "--field", "tags"]));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(victim.exists(), "the decoy was deleted");
    assert!(
        stderr(&output).contains(SKIPPED_DECOY),
        "{}",
        stderr(&output)
    );
}

#[test]
fn view_list_and_remove_work_on_an_invalid_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("../decoy", "tags")]);
    let victim = plant_decoy(&dir, &dir.path().join("decoy"));

    let list = run(ntropy(&vault).args(["view", "list"]));
    assert!(list.status.success(), "{}", stderr(&list));
    assert!(stdout(&list).contains("../decoy"), "{}", stdout(&list));

    let remove = run(ntropy(&vault).args(["view", "remove", "../decoy"]));
    assert!(remove.status.success(), "{}", stderr(&remove));
    assert_eq!(stdout(&remove), "Removed view `../decoy`.\n");
    assert!(victim.exists(), "the decoy was deleted");
}

#[test]
fn init_reports_a_skipped_view_of_an_existing_vault() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("By_Tag", "tags")]);

    let output = run(ntropy(&vault).arg("init"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("skipped view `By_Tag`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn site_gives_a_view_with_an_invalid_name_no_pages() {
    // `views/<name>/index.html` is joined onto the output directory, so a
    // name with two `..` would land beside it, at `<base>/sub/decoy`. The
    // vault sits deep enough that nothing it could sync leaves `<base>`.
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "x/y/vault", &[("../../decoy", "status")]);
    write_note(&vault, "---\ntitle: Note\nstatus: open\n---\nbody\n");
    let out = dir.path().join("sub").join("out");
    let escaped = dir.path().join("sub").join("decoy").join("index.html");

    let output = run(ntropy(&vault).args(["site", "-n", "-o"]).arg(&out));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        !escaped.exists(),
        "a page was written outside the output dir"
    );
    assert!(
        stderr(&output).contains("skipped view `../../decoy`"),
        "{}",
        stderr(&output)
    );

    let strict = run(ntropy(&vault)
        .args(["site", "-n", "--force", "--strict", "-o"])
        .arg(&out));
    assert_eq!(strict.status.code(), Some(1), "{}", stderr(&strict));
}

#[test]
fn reconcile_skips_a_view_behind_a_symlink() {
    // A cloned tree can carry a symlink where a view's directory belongs.
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault", &[("by-tag", "tags")]);
    let victim = plant_decoy(&dir, &dir.path().join("decoy"));
    std::os::unix::fs::symlink(dir.path().join("decoy"), vault.join("by-tag"))
        .expect("plant symlink");

    let output = run(ntropy(&vault).args(["reconcile", "--strict"]));
    assert!(victim.exists(), "the decoy was deleted");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("warning: skipped view `by-tag`: `")
            && stderr(&output).contains("by-tag` is a symlink"),
        "{}",
        stderr(&output)
    );
}

/// `vault encrypt` against vaults whose views it could not remove.
#[cfg(feature = "encryption")]
mod encrypt {
    use super::*;

    const PASSPHRASE_FILE: &str = "test-passphrase.txt";

    /// `ntropy` for a conversion: non-interactive, with a passphrase file
    /// beside the vault so nothing prompts.
    fn ntropy_convert(vault: &Path) -> Command {
        fs::write(
            vault.join(PASSPHRASE_FILE),
            "correct horse battery staple\n",
        )
        .expect("passphrase file");
        let mut cmd = ntropy(vault);
        cmd.args(["-n", "--passphrase-file", PASSPHRASE_FILE]);
        cmd
    }

    #[test]
    fn encrypt_refuses_a_view_with_an_invalid_name_until_it_is_removed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = vault_at(dir.path(), "vault", &[("By_Tag", "tags")]);
        write_note(
            &vault,
            "---\ntitle: Secret Plans\ntags: [work]\n---\nbody\n",
        );
        // The tree an earlier ntropy built for the view, link names and all.
        fs::create_dir_all(vault.join("By_Tag/work")).expect("old view tree");

        let refused = run(ntropy_convert(&vault).args(["vault", "encrypt", "-y"]));
        assert!(!refused.status.success());
        assert!(
            stderr(&refused).contains("remove it with `ntropy view remove 'By_Tag'`"),
            "{}",
            stderr(&refused)
        );
        assert!(!vault.join(".ntropy/identity.pub").exists());
        assert!(!vault.join(".ntropy/identity.age").exists());
        assert!(!vault.join(".ntropy/migration.toml").exists());

        // The way out the message names.
        let removed = run(ntropy(&vault).args(["view", "remove", "By_Tag"]));
        assert!(removed.status.success(), "{}", stderr(&removed));
        fs::remove_dir_all(vault.join("By_Tag")).expect("delete the old tree");

        let encrypted = run(ntropy_convert(&vault).args(["vault", "encrypt", "-y"]));
        assert!(encrypted.status.success(), "{}", stderr(&encrypted));
        assert!(vault.join(".ntropy/identity.pub").is_file());
    }

    #[test]
    fn encrypt_refuses_a_view_behind_a_symlink() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = vault_at(dir.path(), "vault", &[("by-tag", "tags")]);
        let victim = plant_decoy(&dir, &dir.path().join("decoy"));
        std::os::unix::fs::symlink(dir.path().join("decoy"), vault.join("by-tag"))
            .expect("plant symlink");

        let refused = run(ntropy_convert(&vault).args(["vault", "encrypt", "-y"]));
        assert!(!refused.status.success());
        assert!(
            stderr(&refused).contains("remove the symlink itself (not what it points to)"),
            "{}",
            stderr(&refused)
        );
        assert!(victim.exists(), "the decoy was deleted");
        assert!(!vault.join(".ntropy/identity.pub").exists());
        assert!(!vault.join(".ntropy/migration.toml").exists());
    }

    #[test]
    fn a_resumed_encrypt_skips_a_view_with_an_invalid_name() {
        // A resumed conversion is past the up-front check, so a view that
        // turned invalid in between is skipped at the commit and reported.
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = vault_at(dir.path(), "vault", &[]);
        write_note(&vault, "---\ntitle: Note\n---\nbody\n");
        let victim = plant_decoy(&dir, &dir.path().join("decoy"));

        let interrupted = run(ntropy_convert(&vault)
            .args(["vault", "encrypt", "-y"])
            .env("NTROPY_MIGRATION_FAIL_AFTER", "produce"));
        assert!(!interrupted.status.success());
        fs::write(
            vault.join(".ntropy/config.toml"),
            "[[view]]\nname = \"../decoy\"\nfield = \"tags\"\n",
        )
        .expect("config");

        let resumed = run(ntropy_convert(&vault).args(["vault", "encrypt", "--resume", "-y"]));
        assert!(resumed.status.success(), "{}", stderr(&resumed));
        assert!(victim.exists(), "the decoy was deleted");
        assert!(
            stderr(&resumed).contains(&format!("{SKIPPED_DECOY}; its directory was left in place")),
            "{}",
            stderr(&resumed)
        );
        assert!(!vault.join(".ntropy/migration.toml").exists());
    }
}
