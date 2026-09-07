// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! CLI contract tests: run the real binary and snapshot stdout, stderr and exit
//! code with `insta-cmd` (ADR 0021).
//!
//! Only non-interactive paths are exercised; the picker and editor TUIs are
//! validated manually (ADR 0021). Interactivity keys off the controlling
//! terminal (ADR 0036), which exists for a local `cargo test` but not
//! necessarily in CI, so every invocation that would branch on it must pass
//! `-n` or `--print` to stay deterministic across both. Snapshots redact the
//! temporary vault path and ULIDs so they are stable across runs.

use std::fs;
use std::path::Path;
use std::process::Command;

use insta_cmd::assert_cmd_snapshot;

const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";
const ULID_C: &str = "01CRZ3NDEKTSV4RRFFQ69G5FAV";

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
    // The ULID and date tokens are the exact width of the values they replace
    // (26 and 10 characters). The plain tables align columns on the real values
    // before redaction (ADR 0033), so a same-width token keeps the snapshot's
    // `ID` and `DATE` columns aligned with their header instead of collapsing to
    // a shorter placeholder that would look ragged.
    settings.add_filter(r"[0-9A-HJKMNP-TV-Z]{26}", "[ULID....................]");
    // Derived dates render in the local timezone (ADR 0010), so redact them to
    // keep snapshots stable across machines.
    settings.add_filter(r"\d{4}-\d{2}-\d{2}", "[DATE....]");
    settings
}

#[test]
fn print_content_emits_the_note_itself() {
    let dir = setup_vault();
    let vault = dir.path();
    write_note(
        vault,
        ULID_A,
        "a",
        "---\ntitle: A\ntags: [work]\n---\nbody line\n",
    );
    redacted(vault).bind(|| {
        assert_cmd_snapshot!(ntropy(vault).args(["-n", "search", "-P", ULID_A]));
    });
}

#[test]
fn print_content_needs_exactly_one_note() {
    // Several notes run together with nothing between them is not something a
    // caller can take apart again, so the ambiguity is reported instead.
    let dir = setup_vault();
    let vault = dir.path();
    write_note(vault, ULID_A, "a", "---\ntitle: A\n---\nbody\n");
    write_note(vault, ULID_B, "b", "---\ntitle: B\n---\nbody\n");
    redacted(vault).bind(|| {
        assert_cmd_snapshot!(ntropy(vault).args(["-n", "search", "-P"]));
    });
}

#[test]
fn print_content_and_print_are_mutually_exclusive() {
    let dir = setup_vault();
    let vault = dir.path();
    let output = ntropy(vault)
        .args(["-n", "search", "-p", "-P", ULID_A])
        .output()
        .expect("run");
    assert!(!output.status.success());
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
fn new_print_prints_path() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "My First Note", "--print"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn new_print_short_flag_prints_path() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "My First Note", "-p"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn no_edit_is_a_hidden_alias_of_print() {
    // `--no-edit` still parses as an alias of `--print` for backward
    // compatibility, but the help only documents `--print`/`-p` (ADR 0035).
    let dir = setup_vault();

    let aliased = ntropy(dir.path())
        .args(["new", "Aliased", "--no-edit"])
        .output()
        .expect("run ntropy");
    assert!(aliased.status.success(), "--no-edit must behave as --print");
    let stdout = String::from_utf8_lossy(&aliased.stdout);
    assert!(
        stdout.trim_end().ends_with("-aliased.md"),
        "--no-edit must print the created note's path, got: {stdout}"
    );

    let help = ntropy(dir.path())
        .args(["new", "--help"])
        .output()
        .expect("run ntropy");
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help_text.contains("--print"), "help must document --print");
    assert!(
        !help_text.contains("--no-edit"),
        "help must not advertise the hidden alias, got: {help_text}"
    );
}

#[test]
fn new_uses_named_template() {
    let dir = setup_vault();
    let templates = dir.path().join(".ntropy/templates");
    fs::create_dir_all(&templates).expect("templates dir");
    fs::write(
        templates.join("meeting.md"),
        "---\ntitle: {{title}}\ntags: [meeting]\n---\nAgenda for {{title}}\n",
    )
    .expect("write template");

    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "Standup", "--template", "meeting", "--print"]);
        assert_cmd_snapshot!(cmd);
    });

    // The note was created from the meeting template.
    let created: Vec<_> = fs::read_dir(dir.path().join("all-notes"))
        .expect("read all-notes")
        .map(|e| e.expect("entry").path())
        .collect();
    assert_eq!(created.len(), 1);
    let body = fs::read_to_string(&created[0]).expect("read note");
    assert!(body.contains("Agenda for Standup"));
    assert!(body.contains("tags: [meeting]"));
}

#[test]
fn new_empty_creates_a_file_with_no_content() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "--empty", "My First Note", "--print"]);
        assert_cmd_snapshot!(cmd);
    });

    // The name is canonical, the file is there, and nothing is in it.
    let created: Vec<_> = fs::read_dir(dir.path().join("all-notes"))
        .expect("read all-notes")
        .map(|e| e.expect("entry").path())
        .collect();
    assert_eq!(created.len(), 1);
    let name = created[0].file_name().expect("name").to_string_lossy();
    assert!(name.ends_with("-my-first-note.md"), "got: {name}");
    assert_eq!(fs::read_to_string(&created[0]).expect("read note"), "");
}

#[test]
fn new_empty_ignores_a_broken_default_template() {
    // No template is consulted at all, so one that could never render into a
    // well-formed note does not stand in the way.
    let dir = setup_vault();
    let templates = dir.path().join(".ntropy/templates");
    fs::create_dir_all(&templates).expect("templates dir");
    fs::write(templates.join("default.md"), "no frontmatter at all\n")
        .expect("write broken default template");

    let out = ntropy(dir.path())
        .args(["new", "--empty", "-p", "Unaffected"])
        .output()
        .expect("run ntropy");
    assert!(out.status.success(), "--empty must not read the template");
    let path = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    assert_eq!(fs::read_to_string(&path).expect("read note"), "");
}

#[test]
fn new_empty_rejects_a_template_selection() {
    // The two are contradictory: one says "stamp this template", the other
    // "stamp nothing".
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "--empty", "-t", "meeting", "-p", "Standup"]);
        assert_cmd_snapshot!(cmd);
    });
    assert_eq!(
        fs::read_dir(dir.path().join("all-notes"))
            .expect("read all-notes")
            .count(),
        0
    );
}

#[test]
fn new_empty_help_documents_the_flag() {
    let dir = setup_vault();
    let help = ntropy(dir.path())
        .args(["new", "--help"])
        .output()
        .expect("run ntropy");
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help_text.contains("--empty"), "got: {help_text}");
}

#[test]
fn new_empty_becomes_a_note_once_the_caller_fills_it() {
    // The whole workflow the flag exists for: take the path, write the note
    // yourself, reconcile. The scan warns about the empty file in between,
    // which is what `--strict` would turn into an error.
    let dir = setup_vault();
    let out = ntropy(dir.path())
        .args(["new", "--empty", "-p", "Quarterly Review"])
        .output()
        .expect("run ntropy");
    let path = String::from_utf8_lossy(&out.stdout).trim_end().to_string();

    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n"]);
        assert_cmd_snapshot!("search_warns_about_an_unfilled_empty_note", cmd);
    });

    fs::write(
        &path,
        "---\ntitle: Quarterly Review\ntags: [work, planning]\nstatus: draft\n---\n# Quarterly Review\n\nNumbers go here.\n",
    )
    .expect("fill the note");

    let reconciled = ntropy(dir.path())
        .args(["reconcile"])
        .output()
        .expect("run ntropy");
    assert!(reconciled.status.success());

    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n", "tag:planning and status:draft"]);
        assert_cmd_snapshot!("search_finds_the_filled_empty_note", cmd);
    });
}

// =============================================================================
// `write`
// =============================================================================

/// The note text the write tests feed in, and the title it carries.
const WRITTEN: &str = "---\ntitle: Written Note\ntags: [written, work]\nstatus: draft\n---\n# Written Note\n\nBody.\n";

/// Run `cmd` with `content` on stdin.
///
/// stdin has to be a pipe the test controls: `write` reads it to EOF, and an
/// inherited stdin would make the run depend on how the harness was invoked.
fn feed(cmd: &mut Command, content: &str) -> std::process::Output {
    use std::io::Write as _;
    use std::process::Stdio;

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ntropy");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(content.as_bytes())
        .expect("feed stdin");
    child.wait_with_output().expect("wait for ntropy")
}

/// Run `ntropy write <args…>` in `vault` with `content` on stdin.
fn run_write(vault: &Path, args: &[&str], content: &str) -> std::process::Output {
    let mut cmd = ntropy(vault);
    cmd.arg("write").args(args);
    feed(&mut cmd, content)
}

/// Render one run as a snapshot body, so a failure shows exit code and both
/// streams the way `assert_cmd_snapshot!` does for the stdin-less commands.
fn rendered(out: &std::process::Output) -> String {
    format!(
        "success: {}\nexit_code: {}\n----- stdout -----\n{}\n----- stderr -----\n{}",
        out.status.success(),
        out.status.code().expect("exit code"),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    )
}

#[test]
fn write_fills_an_empty_note() {
    // The whole point of the pairing: `new --empty` allocates the identity,
    // `write` supplies the content, and nothing had to be read in between.
    let dir = setup_vault();
    let created = ntropy(dir.path())
        .args(["new", "--empty", "-p", "Written Note"])
        .output()
        .expect("run ntropy");
    let path = String::from_utf8_lossy(&created.stdout)
        .trim_end()
        .to_string();

    let out = run_write(dir.path(), &[&path], WRITTEN);
    assert!(out.status.success(), "{}", rendered(&out));
    assert_eq!(fs::read_to_string(&path).expect("read note"), WRITTEN);

    // The written path is echoed back, and the note is now findable.
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim_end(), path);
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n", "tag:written and status:draft"]);
        assert_cmd_snapshot!("write_search_finds_the_written_note", cmd);
    });
}

#[test]
fn write_replaces_an_existing_notes_content() {
    let dir = setup_vault();
    let created = ntropy(dir.path())
        .args(["new", "-p", "Written Note"])
        .output()
        .expect("run ntropy");
    let path = String::from_utf8_lossy(&created.stdout)
        .trim_end()
        .to_string();

    let out = run_write(dir.path(), &[&path], WRITTEN);
    assert!(out.status.success(), "{}", rendered(&out));
    assert_eq!(fs::read_to_string(&path).expect("read note"), WRITTEN);
}

#[test]
fn write_accepts_a_bare_ulid_and_a_bare_filename() {
    let dir = setup_vault();
    let created = ntropy(dir.path())
        .args(["new", "--empty", "-p", "Written Note"])
        .output()
        .expect("run ntropy");
    let path = String::from_utf8_lossy(&created.stdout)
        .trim_end()
        .to_string();
    let name = Path::new(&path)
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    let ulid = &name[..26];

    let by_ulid = run_write(dir.path(), &[ulid], WRITTEN);
    assert!(by_ulid.status.success(), "{}", rendered(&by_ulid));
    assert_eq!(
        String::from_utf8_lossy(&by_ulid.stdout).trim_end(),
        path,
        "a bare ULID must resolve to the same file"
    );

    let by_name = run_write(dir.path(), &[&name], WRITTEN);
    assert!(by_name.status.success(), "{}", rendered(&by_name));
    assert_eq!(String::from_utf8_lossy(&by_name.stdout).trim_end(), path);
}

#[test]
fn write_realigns_the_filename_when_the_title_changed() {
    // Same post-processing the editor round trip does on exit: a written title
    // renames the file, and the path printed is the one that now exists.
    let dir = setup_vault();
    let created = ntropy(dir.path())
        .args(["new", "--empty", "-p", "Placeholder Title"])
        .output()
        .expect("run ntropy");
    let path = String::from_utf8_lossy(&created.stdout)
        .trim_end()
        .to_string();

    let out = run_write(dir.path(), &[&path], WRITTEN);
    assert!(out.status.success(), "{}", rendered(&out));

    let printed = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    assert!(
        printed.ends_with("-written-note.md"),
        "the printed path must be the realigned one, got: {printed}"
    );
    assert!(Path::new(&printed).is_file());
    assert!(!Path::new(&path).exists(), "the old filename must be gone");
}

#[test]
fn write_refreshes_the_views() {
    // The other half of reconciling: a written tag has to reach `by-tag/`
    // without the caller running `reconcile` separately.
    let dir = setup_vault();
    let created = ntropy(dir.path())
        .args(["new", "--empty", "-p", "Written Note"])
        .output()
        .expect("run ntropy");
    let path = String::from_utf8_lossy(&created.stdout)
        .trim_end()
        .to_string();

    let out = run_write(dir.path(), &[&path], WRITTEN);
    assert!(out.status.success(), "{}", rendered(&out));
    assert!(
        dir.path().join("by-tag/written").is_dir(),
        "the written tag never reached the view"
    );
}

#[test]
fn write_needs_no_interactivity_flag() {
    // stdin carries the payload, so nothing about this command branches on the
    // controlling terminal: no `-n`, no picker, no prompt.
    let dir = setup_vault();
    let created = ntropy(dir.path())
        .args(["new", "--empty", "-p", "Written Note"])
        .output()
        .expect("run ntropy");
    let path = String::from_utf8_lossy(&created.stdout)
        .trim_end()
        .to_string();

    let out = run_write(dir.path(), &[&path], WRITTEN);
    assert!(out.status.success(), "{}", rendered(&out));
}

#[test]
fn write_refuses_content_that_is_not_a_note() {
    let dir = setup_vault();
    let created = ntropy(dir.path())
        .args(["new", "-p", "Written Note"])
        .output()
        .expect("run ntropy");
    let path = String::from_utf8_lossy(&created.stdout)
        .trim_end()
        .to_string();
    let before = fs::read_to_string(&path).expect("read note");

    let out = run_write(dir.path(), &[&path], "no frontmatter at all\n");
    assert!(!out.status.success());
    redacted(dir.path()).bind(|| {
        insta::assert_snapshot!("write_rejects_non_note_content", rendered(&out));
    });
    assert_eq!(
        fs::read_to_string(&path).expect("read note"),
        before,
        "a refused write must leave the note alone"
    );
}

#[test]
fn write_refuses_an_unknown_target() {
    let dir = setup_vault();
    let out = run_write(dir.path(), &[ULID_A], WRITTEN);
    assert!(!out.status.success());
    redacted(dir.path()).bind(|| {
        insta::assert_snapshot!("write_rejects_unknown_target", rendered(&out));
    });
    assert_eq!(
        fs::read_dir(dir.path().join("all-notes"))
            .expect("read all-notes")
            .count(),
        0,
        "a failed write must not create a note"
    );
}

#[test]
fn write_refuses_a_path_outside_all_notes() {
    let dir = setup_vault();
    let outside = dir.path().join("elsewhere.md");
    fs::write(&outside, "---\ntitle: Elsewhere\n---\n").expect("write outside");

    let out = run_write(dir.path(), &[outside.to_str().expect("utf-8")], WRITTEN);
    assert!(!out.status.success());
    redacted(dir.path()).bind(|| {
        insta::assert_snapshot!("write_rejects_path_outside_all_notes", rendered(&out));
    });
    assert!(
        fs::read_to_string(&outside)
            .expect("read")
            .contains("Elsewhere"),
        "the file outside the vault must be untouched"
    );
}

#[test]
fn new_missing_named_template_errors() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "X", "-t", "absent", "--print"]);
        assert_cmd_snapshot!(cmd);
    });
    // No note was created.
    assert_eq!(
        fs::read_dir(dir.path().join("all-notes"))
            .expect("read all-notes")
            .count(),
        0
    );
}

#[test]
fn new_print_accepts_a_yaml_special_title() {
    // Reproduces the bug in
    // todos/01kwvczg18dprcrdja9dzzqzde-failed-new-leaves-malformed-note-file-in-all-notes.md:
    // a `: ` in the title used to break the default template's YAML and leave
    // no note behind. Frontmatter substitution is now YAML-aware (ADR 0034),
    // so the same title now creates a well-formed note.
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "Q3: Planning kickoff", "--print"]);
        assert_cmd_snapshot!("new_yaml_special_title", cmd);
    });

    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n"]);
        assert_cmd_snapshot!("search_shows_yaml_special_title", cmd);
    });
}

#[test]
fn new_print_with_invalid_template_leaves_no_stray_file() {
    // Aspect 1 of
    // todos/01kwvczg18dprcrdja9dzzqzde-failed-new-leaves-malformed-note-file-in-all-notes.md:
    // a template whose rendered output is not a well-formed note (here, no
    // `title` field) must fail `new` without leaving a file in `all-notes/`.
    let dir = setup_vault();
    let templates = dir.path().join(".ntropy/templates");
    fs::create_dir_all(&templates).expect("templates dir");
    fs::write(
        templates.join("default.md"),
        "---\ntags: []\n---\nBody with no title field.\n",
    )
    .expect("write broken default template");

    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["new", "Hello World", "--print"]);
        assert_cmd_snapshot!(cmd);
    });

    // No note was created.
    assert_eq!(
        fs::read_dir(dir.path().join("all-notes"))
            .expect("read all-notes")
            .count(),
        0
    );
}

#[test]
fn today_creates_then_reuses_the_daily_note() {
    let dir = setup_vault();
    let templates = dir.path().join(".ntropy/templates");
    fs::create_dir_all(&templates).expect("templates dir");
    fs::write(
        templates.join("today.md"),
        "---\ntitle: {{date}}\ntags: [daily]\n---\n# {{date}}\n",
    )
    .expect("write today template");

    redacted(dir.path()).bind(|| {
        let mut first = ntropy(dir.path());
        first.args(["today", "--print"]);
        assert_cmd_snapshot!("today_first", first);

        // A second run reuses the same note (same printed path); it also uses
        // the `-p` spelling, so both forms of the flag are exercised.
        let mut again = ntropy(dir.path());
        again.args(["today", "-p"]);
        assert_cmd_snapshot!("today_again", again);
    });

    // Only one daily note exists.
    assert_eq!(
        fs::read_dir(dir.path().join("all-notes"))
            .expect("read all-notes")
            .count(),
        1
    );
}

#[test]
fn today_without_template_errors() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["today", "--print"]);
        assert_cmd_snapshot!(cmd);
    });
    assert_eq!(
        fs::read_dir(dir.path().join("all-notes"))
            .expect("read all-notes")
            .count(),
        0
    );
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
fn edit_without_selector_lists_like_search() {
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
        cmd.args(["edit", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn list_is_an_alias_for_search() {
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "a", "---\ntitle: A\n---\nbody\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["list", "-n"]);
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

/// Columns line up even when titles differ wildly in width, including a CJK
/// title whose display width exceeds its `char` count. The padding is driven by
/// the widest title, so the `TAGS` column starts at the same offset on every
/// row (ADR 0033). ULIDs and dates redact to fixed tokens, so the alignment is
/// read off the un-redacted `TITLE`/`TAGS` columns.
#[test]
fn search_aligns_varied_width_titles() {
    // A third ULID, ordered after A and B so the newest-first listing is C,B,A.
    const ULID_C: &str = "01CRZ3NDEKTSV4RRFFQ69G5FAV";
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "mid",
        "---\ntitle: Mid Title\ntags: [area/home]\n---\nbody\n",
    );
    write_note(
        dir.path(),
        ULID_B,
        "long",
        "---\ntitle: A Much Longer Note Title\ntags: [area/work]\n---\nbody\n",
    );
    write_note(
        dir.path(),
        ULID_C,
        "wide",
        "---\ntitle: 日本語\ntags: [lang/jp]\n---\nbody\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n"]);
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
fn search_by_ulid_resolves_single_note() {
    // A full ULID selector resolves to exactly that note; non-interactively the
    // lone match prints as a one-row table (ADR 0031).
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    write_note(
        dir.path(),
        ULID_B,
        "other",
        "---\ntitle: Other\n---\nbody\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", ULID_A, "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn search_print_non_interactive_prints_paths_one_per_line() {
    // With no picker to choose one note, `--print` covers every match: one
    // path per line, newest first, nothing else on stdout (ADR 0035).
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
        cmd.args(["search", "-n", "--print"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn search_print_short_flag_resolves_single_note() {
    // The short form `-p` parses on `search`; non-interactively a lone match
    // prints as exactly one path line (ADR 0035).
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", ULID_A, "-p", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn search_print_no_match_exits_nonzero() {
    // The no-match contract of ADR 0031 holds under `--print`: message on
    // stderr, nothing on stdout, non-zero exit.
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", ULID_A, "--print", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn search_no_edit_is_a_hidden_alias_of_print() {
    // As on `new`/`today`, `--no-edit` parses as an alias of `--print` but the
    // help only documents `--print`/`-p` (ADR 0035).
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "a", "---\ntitle: A\n---\nbody\n");

    let aliased = ntropy(dir.path())
        .args(["search", "--no-edit", "-n"])
        .output()
        .expect("run ntropy");
    assert!(aliased.status.success(), "--no-edit must parse on search");
    let stdout = String::from_utf8_lossy(&aliased.stdout);
    assert!(
        stdout.trim_end().ends_with("-a.md"),
        "--no-edit must print the matching note's path, got: {stdout}"
    );

    let help = ntropy(dir.path())
        .args(["search", "--help"])
        .output()
        .expect("run ntropy");
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help_text.contains("--print"), "help must document --print");
    assert!(
        !help_text.contains("--no-edit"),
        "help must not advertise the hidden alias, got: {help_text}"
    );
}

#[test]
fn search_empty_vault_exits_nonzero() {
    // An empty result, even a bare listing of an empty vault, exits non-zero
    // with the message on stderr (ADR 0031).
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["search", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

/// The relative `PATH` entry the stub-typst tests set. It is a directory
/// inside the vault, so the entry is a fixed relative name rather than a
/// host-specific temp path: insta-cmd records set env vars verbatim in the
/// snapshot's `info` block (filters do not reach it), so a relative value keeps
/// those snapshots stable. A relative `PATH` component resolves against the
/// process's working directory, which the tests set to the vault.
const STUB_BIN: &str = "stub-bin";

/// Write a fake `typst` into `<vault>/stub-bin/`. It drains stdin (the emitted
/// document ntropy pipes in on `typst compile -`), takes its last argument as
/// the output path, writes a fixed marker there, and exits 0, so the default
/// pdf pipeline's success path is exercised without the real compiler.
/// `stub-bin` lives in the vault root, which the scanner never walks, so it is
/// invisible to selection.
fn write_stub_typst(vault: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let bin = vault.join(STUB_BIN);
    fs::create_dir_all(&bin).expect("stub-bin dir");
    // Draining stdin mirrors the real compiler consuming the document; the last
    // argument of `typst compile - <path>` is the output path the marker lands
    // at, proving ntropy handed the tool an absolute, working-directory-proof
    // location. `cat` is named by absolute path: the tests run the stub with
    // `PATH` reduced to the relative `stub-bin` entry, which resolves against
    // the child's working directory (the note's), where no `cat` exists.
    let script = r#"#!/bin/sh
/bin/cat > /dev/null
out=""
for arg in "$@"; do
  out="$arg"
done
if [ -n "$out" ]; then
  printf 'stub pdf via typst\n' > "$out"
fi
exit 0
"#;
    let path = bin.join("typst");
    fs::write(&path, script).expect("write stub typst");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod stub typst");
}

/// Write a fake `typst` that records its own argument vector instead of a
/// fixed marker, so a test can assert on how ntropy invoked the compiler.
/// One argument per line, into the last argument (the output path).
fn write_arg_recording_typst(vault: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let bin = vault.join(STUB_BIN);
    fs::create_dir_all(&bin).expect("stub-bin dir");
    let script = r#"#!/bin/sh
/bin/cat > /dev/null
out=""
for arg in "$@"; do
  out="$arg"
done
if [ -n "$out" ]; then
  : > "$out"
  for arg in "$@"; do
    printf '%s
' "$arg" >> "$out"
  done
fi
exit 0
"#;
    let path = bin.join("typst");
    fs::write(&path, script).expect("write stub typst");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod stub typst");
}

#[test]
fn render_bare_empty_vault_errors() {
    // A blank selector browses all notes, so an empty vault is a no-match,
    // reported with `search`'s wording and a non-zero exit.
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_bare_single_note_renders() {
    // With exactly one note in the vault a bare invocation needs no narrowing,
    // so it renders that note even without a picker. A bare `--to pdf` now uses
    // the default typst engine, driven here by the typst stub.
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "only", "---\ntitle: Only\n---\nbody\n");
    write_stub_typst(dir.path());
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.env("PATH", STUB_BIN);
        cmd.current_dir(dir.path());
        cmd.args(["render", "-n", "-p"]);
        assert_cmd_snapshot!(cmd);
    });
    assert_eq!(
        fs::read_to_string(dir.path().join("only.pdf")).expect("artifact exists"),
        "stub pdf via typst\n"
    );
}

#[test]
fn render_bare_several_notes_needs_a_selector_under_n() {
    // Without a picker a bare invocation has no way to narrow several notes,
    // so it asks for a selector instead of dumping an ambiguity list.
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "alpha", "---\ntitle: Alpha\n---\n");
    write_note(dir.path(), ULID_B, "beta", "---\ntitle: Beta\n---\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_help_pins_the_flag_surface() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", "--help"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_ambiguous_selector_errors_under_n() {
    // Two matches with no picker (`-n`): the candidate list prints to stderr
    // and the command fails, mirroring `delete` (ADR 0025).
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
        cmd.args(["render", "tag:work", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_no_match_errors() {
    let dir = setup_vault();
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", "tag:nonexistent", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_unknown_format_errors() {
    // The engine resolves before any scan, so an unknown format fails first.
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "wanted", "---\ntitle: Wanted\n---\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", ULID_A, "--to", "no-such-format", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_unknown_engine_errors() {
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "wanted", "---\ntitle: Wanted\n---\n");
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", ULID_A, "--engine", "no-such-engine", "-n"]);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_missing_typst_reports_unavailable() {
    // The default `pdf` engine compiles with `typst`; `PATH` points at a
    // directory that does not exist so it is not found. The error names typst as
    // the tool to install, with the per-program hint.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", ULID_A, "-n"]);
        // A relative `PATH` naming a directory that does not exist keeps the
        // recorded env deterministic while ensuring typst is not found.
        cmd.env("PATH", "no-such-bin");
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_scan_warnings_print_and_strict_fails() {
    // A malformed sibling note warns on stderr while the good note still
    // renders (stub typst, the default engine); `--strict` promotes the warning
    // to a failure.
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "good", "---\ntitle: Good\n---\nbody\n");
    write_note(dir.path(), ULID_B, "bad", "---\ntags: [x]\n---\n");
    write_stub_typst(dir.path());
    redacted(dir.path()).bind(|| {
        let mut lenient = ntropy(dir.path());
        lenient.args(["render", ULID_A, "-p", "-n"]);
        lenient.env("PATH", STUB_BIN);
        assert_cmd_snapshot!("render_warnings_lenient", lenient);

        let mut strict = ntropy(dir.path());
        strict.args(["render", ULID_A, "-p", "-n", "--strict"]);
        strict.env("PATH", STUB_BIN);
        assert_cmd_snapshot!("render_warnings_strict", strict);
    });
}

#[test]
fn render_default_output_names_the_slug() {
    // Without `-p`, stdout narrates and the completion report names the default
    // typst engine; the artifact lands at `<slug>.pdf` in the working directory
    // (here the vault root).
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    write_stub_typst(dir.path());
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", ULID_A, "-n"]);
        cmd.env("PATH", STUB_BIN);
        assert_cmd_snapshot!(cmd);
    });
    let artifact = dir.path().join("wanted.pdf");
    assert_eq!(
        fs::read_to_string(&artifact).expect("read artifact"),
        "stub pdf via typst\n"
    );
}

#[test]
fn render_print_emits_the_artifact_path() {
    // `-p` prints exactly the artifact path as one line (default typst engine).
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    write_stub_typst(dir.path());
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", ULID_A, "-p", "-n"]);
        cmd.env("PATH", STUB_BIN);
        assert_cmd_snapshot!(cmd);
    });
}

#[test]
fn render_output_flag_is_honored() {
    // `-o` overrides the default name; the artifact appears at the given path
    // (default typst engine).
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    write_stub_typst(dir.path());
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", ULID_A, "-o", "custom.pdf", "-p", "-n"]);
        cmd.env("PATH", STUB_BIN);
        assert_cmd_snapshot!(cmd);
    });
    assert!(dir.path().join("custom.pdf").exists());
    // The default name was not used.
    assert!(!dir.path().join("wanted.pdf").exists());
}

#[test]
fn render_overwrites_an_existing_artifact() {
    // A pre-existing file at the target is replaced silently (ADR 0037).
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    let target = dir.path().join("wanted.pdf");
    fs::write(&target, "stale content").expect("seed stale artifact");
    write_stub_typst(dir.path());

    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "-n"]);
    cmd.env("PATH", STUB_BIN);
    let status = cmd.status().expect("run render");
    assert!(status.success());
    assert_eq!(
        fs::read_to_string(&target).expect("read artifact"),
        "stub pdf via typst\n"
    );
}

#[test]
fn render_relative_output_is_absolutized_against_the_invocation_cwd() {
    // The default typst engine compiles in the note's own directory (all-notes),
    // so a relative `-o` path must be absolutized against the invocation's cwd
    // (the vault root) rather than resolving inside all-notes. The artifact lands
    // at the vault-root-relative location, and all-notes stays free of strays.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    write_stub_typst(dir.path());
    // The stub writes but does not create parent directories, so the target
    // subdirectory (under the vault root, where absolutization lands it) exists.
    fs::create_dir_all(dir.path().join("out")).expect("output subdir");

    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "-o", "out/report.pdf", "-n"]);
    cmd.env("PATH", STUB_BIN);
    let status = cmd.status().expect("run render");
    assert!(status.success());

    // Absolutized against the vault root (the cwd), not the note's directory.
    assert_eq!(
        fs::read_to_string(dir.path().join("out/report.pdf"))
            .expect("artifact at the cwd-relative path"),
        "stub pdf via typst\n"
    );
    // An unabsolutized path would have landed inside the note's directory.
    assert!(
        !dir.path().join("all-notes/out/report.pdf").exists(),
        "no artifact leaked into the note's directory"
    );
}

#[test]
fn render_to_typst_writes_a_real_artifact_without_any_tool() {
    // `--to typst` emits the document itself, so no stub binary is needed: with
    // `PATH` pointing at a directory that does not exist, the render still
    // succeeds and the `.typ` artifact carries the prelude application and the
    // converted body.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nThe note body text.\n",
    );
    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "--to", "typst", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    let status = cmd.status().expect("run render");
    assert!(status.success());

    let artifact = fs::read_to_string(dir.path().join("wanted.typ")).expect("typ artifact exists");
    assert!(
        artifact.contains("#show: note.with"),
        "template application missing: {artifact}"
    );
    assert!(
        artifact.contains("The note body text"),
        "converted body missing: {artifact}"
    );
}

#[test]
fn a_resolved_note_link_targets_the_targets_default_artifact_name() {
    // The cross-document link contract of ADR 0044, pinned from both ends: the
    // emitted document links to `<target-slug>.pdf`, and a default render of
    // that target lands at exactly that name in the same directory. The two
    // halves are what makes a folder of rendered notes navigable, so a change
    // to either naming rule alone must fail here.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "source",
        &format!("---\ntitle: Source\n---\nSee [the target]({ULID_B}-target-note.md).\n"),
    );
    write_note(
        dir.path(),
        ULID_B,
        "target-note",
        "---\ntitle: Target Note\n---\nTarget body.\n",
    );

    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "--to", "typst", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    assert!(cmd.status().expect("run render").success());

    let artifact = fs::read_to_string(dir.path().join("source.typ")).expect("typ artifact exists");
    assert!(
        artifact.contains(r#"#link("target-note.pdf")[#notelink[Target Note]]"#),
        "the note link does not target the target's artifact: {artifact}"
    );

    // The other half: rendering the target with no `-o` produces that file.
    write_stub_typst(dir.path());
    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_B, "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", STUB_BIN);
    assert!(cmd.status().expect("run render").success());
    assert!(
        dir.path().join("target-note.pdf").exists(),
        "the target's default artifact is not the name the link points at"
    );
}

// =========================================================================
// Render themes (ADR 0045)
// =========================================================================

/// Write a theme file into the vault's themes directory, creating it.
fn write_theme(vault: &Path, name: &str, source: &str) {
    let dir = vault.join(".ntropy/themes");
    fs::create_dir_all(&dir).expect("themes dir");
    fs::write(dir.join(format!("{name}.typ")), source).expect("write theme");
}

/// Point the vault's `[render]` section at a theme.
fn configure_theme(vault: &Path, name: &str) {
    let config = vault.join(".ntropy/config.toml");
    let mut text = fs::read_to_string(&config).expect("read config");
    text.push_str(&format!("\n[render]\ntheme = \"{name}\"\n"));
    fs::write(&config, text).expect("write config");
}

/// A theme whose `note()` renders only the body, marked so the artifact can be
/// told apart from the built-in look and from another theme.
fn marker_theme(marker: &str) -> String {
    format!(
        "#let note(title: none, frontmatter: (:), paper: \"a4\", body) = {{\n  \
         [{marker}]\n  body\n}}\n"
    )
}

/// Render `id` to a `typst` artifact and return it. No external tool is
/// involved, so the emitted document is inspectable without a compiler.
fn render_to_typst(vault: &Path, id: &str, extra: &[&str]) -> String {
    let mut cmd = ntropy(vault);
    cmd.args(["render", id, "--to", "typst", "-o", "out.typ", "-n"]);
    cmd.args(extra);
    cmd.current_dir(vault);
    cmd.env("PATH", "no-such-bin");
    let output = cmd.output().expect("run render");
    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(vault.join("out.typ")).expect("typ artifact exists")
}

/// A vault with one note and a `corporate` theme configured vault-wide.
fn themed_vault() -> tempfile::TempDir {
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "report",
        "---\ntitle: Report\ntags: [confidential]\nstatus: draft\n---\nBody text.\n",
    );
    write_theme(dir.path(), "corporate", &marker_theme("CORPORATE-THEME"));
    configure_theme(dir.path(), "corporate");
    dir
}

#[test]
fn the_vault_configured_theme_applies_with_no_flags_on_the_command_line() {
    // Goal 3: after a one-time `[render] theme`, a plain `ntropy render`
    // produces themed output. No theme-related argument appears here.
    let dir = themed_vault();
    let artifact = render_to_typst(dir.path(), ULID_A, &[]);
    assert!(
        artifact.contains("CORPORATE-THEME"),
        "the configured theme did not reach the document: {artifact}"
    );
}

#[test]
fn the_configured_theme_applies_to_every_note_in_the_vault() {
    // The same one-time configuration serves a loop over the whole vault, not
    // just the note it was tried on.
    let dir = themed_vault();
    for (ulid, slug) in [(ULID_B, "second"), (ULID_C, "third")] {
        write_note(
            dir.path(),
            ulid,
            slug,
            &format!("---\ntitle: {slug}\n---\nMore body.\n"),
        );
    }
    for ulid in [ULID_A, ULID_B, ULID_C] {
        let artifact = render_to_typst(dir.path(), ulid, &[]);
        assert!(
            artifact.contains("CORPORATE-THEME"),
            "note {ulid} rendered unthemed: {artifact}"
        );
    }
}

#[test]
fn the_theme_is_spliced_after_the_prelude_and_before_the_template_application() {
    // Order is what makes overriding work: Typst binds `note` to the last
    // `#let` above its use.
    let dir = themed_vault();
    let artifact = render_to_typst(dir.path(), ULID_A, &[]);
    let prelude = artifact.find("#let notelink").expect("prelude present");
    let theme = artifact.find("CORPORATE-THEME").expect("theme present");
    let show = artifact
        .find("#show: note.with")
        .expect("template application present");
    assert!(
        prelude < theme && theme < show,
        "theme is not between the prelude and the template application"
    );
}

#[test]
fn a_theme_defining_only_note_still_gets_callouts_and_tasks_from_the_prelude() {
    // The inheritance half of the contract: a minimal theme does not have to
    // reimplement the constructs the body uses.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "mixed",
        "---\ntitle: Mixed\n---\n> [!NOTE]\n> A callout.\n\n- [x] A task\n",
    );
    write_theme(dir.path(), "minimal", &marker_theme("MINIMAL-THEME"));
    configure_theme(dir.path(), "minimal");

    let artifact = render_to_typst(dir.path(), ULID_A, &[]);
    assert!(artifact.contains("MINIMAL-THEME"), "theme missing");
    assert!(
        artifact.contains("#callout(kind: \"note\")"),
        "callout not emitted: {artifact}"
    );
    assert!(
        artifact.contains("#task(done: true)"),
        "task not emitted: {artifact}"
    );
}

#[test]
fn the_theme_flag_overrides_the_configured_theme() {
    let dir = themed_vault();
    write_theme(dir.path(), "customer", &marker_theme("CUSTOMER-THEME"));

    let artifact = render_to_typst(dir.path(), ULID_A, &["--theme", "customer"]);
    assert!(
        artifact.contains("CUSTOMER-THEME") && !artifact.contains("CORPORATE-THEME"),
        "the flag did not override the configured theme: {artifact}"
    );
}

#[test]
fn the_reserved_default_name_overrides_a_configured_theme_back_to_the_built_in_look() {
    let dir = themed_vault();
    let artifact = render_to_typst(dir.path(), ULID_A, &["--theme", "default"]);
    assert!(
        !artifact.contains("CORPORATE-THEME"),
        "the configured theme survived `--theme default`: {artifact}"
    );
    assert!(
        artifact.contains("#let note("),
        "the built-in prelude is missing: {artifact}"
    );
}

#[test]
fn an_unthemed_vault_renders_exactly_as_before() {
    // The feature is inert until configured: no theme marker, no seam comment.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "plain",
        "---\ntitle: Plain\n---\nBody.\n",
    );
    let artifact = render_to_typst(dir.path(), ULID_A, &[]);
    assert!(
        !artifact.contains("vault theme"),
        "an unconfigured vault emitted a theme seam: {artifact}"
    );
}

#[test]
fn a_configured_theme_with_no_file_errors_naming_the_path() {
    // Hard error rather than a silent fall back: a document in the wrong
    // livery is worse than one that was not produced.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "report",
        "---\ntitle: Report\n---\nBody.\n",
    );
    configure_theme(dir.path(), "no-such-theme");

    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "--to", "typst", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    let output = cmd.output().expect("run render");
    assert!(!output.status.success(), "a missing theme must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no-such-theme") && stderr.contains(".ntropy/themes/no-such-theme.typ"),
        "the error does not name the theme and the path: {stderr}"
    );
    assert!(
        !dir.path().join("report.typ").exists(),
        "an artifact was produced despite the failure"
    );
}

#[test]
fn a_theme_name_carrying_a_path_is_rejected() {
    // The name is joined onto the themes directory, so a traversing name must
    // not reach a file outside it.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "report",
        "---\ntitle: Report\n---\nBody.\n",
    );

    let mut cmd = ntropy(dir.path());
    cmd.args([
        "render",
        ULID_A,
        "--to",
        "typst",
        "-n",
        "--theme",
        "../outside",
    ]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    let output = cmd.output().expect("run render");
    assert!(!output.status.success(), "a traversing name must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid theme name"),
        "the error does not name the problem: {stderr}"
    );
}

#[test]
fn the_vault_root_is_granted_to_the_compiler() {
    // The asset half of the feature: without `--root` at the vault, a theme
    // referencing `/assets/...` cannot resolve it. The stub records its
    // arguments so the contract is checked without the real compiler.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "report",
        "---\ntitle: Report\n---\nBody.\n",
    );
    write_arg_recording_typst(dir.path());

    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "-o", "args.pdf", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", STUB_BIN);
    assert!(cmd.status().expect("run render").success());

    let args = fs::read_to_string(dir.path().join("args.pdf")).expect("recorded args");
    let recorded: Vec<&str> = args.lines().collect();
    let root_at = recorded
        .iter()
        .position(|arg| *arg == "--root")
        .unwrap_or_else(|| panic!("no --root in the invocation: {recorded:?}"));
    let granted = Path::new(recorded[root_at + 1])
        .canonicalize()
        .expect("the granted root exists");
    assert_eq!(
        granted,
        dir.path().canonicalize().expect("canonical vault path"),
        "the compiler was granted a directory other than the vault root"
    );
}

/// The kitchen-sink fixture: one note exercising every supported construct
/// (frontmatter value shapes, all callout kinds, footnote orders, task lists,
/// explicit ordered-list numbers, fence collisions, table alignments, note
/// links resolved and dangling, autolinks, images, raw HTML).
const KITCHEN_SINK: &str = include_str!("fixtures/kitchen-sink.md");

/// The target the kitchen-sink fixture's resolved note link points at.
fn write_kitchen_sink_vault(vault: &Path) {
    write_note(vault, ULID_A, "kitchen-sink", KITCHEN_SINK);
    write_note(
        vault,
        ULID_B,
        "linked",
        "---\ntitle: Current Linked Title\n---\nTarget body.\n",
    );
}

#[test]
fn render_kitchen_sink_pins_the_full_typst_document() {
    // The whole pipeline over the kitchen-sink fixture — prepare, emit,
    // assemble — pinned as one reviewable snapshot: any emitter or prelude
    // change surfaces here as a single kitchen-sink diff. The document also
    // parses error-free through typst-syntax at the unit level (see
    // `src/render/typst/`); this contract test pins the exact bytes.
    let dir = setup_vault();
    write_kitchen_sink_vault(dir.path());

    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "--to", "typst", "-p", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    let output = cmd.output().expect("run render");
    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let artifact =
        fs::read_to_string(dir.path().join("kitchen-sink.typ")).expect("typ artifact exists");
    redacted(dir.path()).bind(|| {
        insta::assert_snapshot!("kitchen_sink_document", artifact);
    });
}

#[test]
#[ignore = "runs the real typst binary; execute via `just verify-render`"]
fn render_kitchen_sink_compiles_with_real_typst() {
    // The full roundtrip's final leg: the kitchen-sink note rendered to pdf by
    // the real `typst` binary, plus a png of the same document for optical
    // inspection. Deliberately opt-in (ADR 0021 keeps external tools out of
    // the standard suite); the artifacts land under `target/verify-render/`.
    let dir = setup_vault();
    write_kitchen_sink_vault(dir.path());

    // The fixture references `diagram.png` next to the note; a minimal valid
    // 1x1 PNG satisfies both the pdf compile and the png render.
    const TINY_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    fs::write(dir.path().join("all-notes/diagram.png"), TINY_PNG).expect("write diagram");

    // The pdf leg: ntropy drives the real compiler end to end. Warnings on
    // stderr (raw HTML, remote image) are the fixture working as designed.
    let mut pdf = ntropy(dir.path());
    pdf.args(["render", ULID_A, "-o", "kitchen-sink.pdf", "-n"]);
    pdf.current_dir(dir.path());
    let output = pdf.output().expect("run render to pdf");
    assert!(
        output.status.success(),
        "pdf compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The typst artifact compiles to a png from beside the note (where its
    // relative asset paths resolve), giving the inspectable image.
    let mut typ = ntropy(dir.path());
    typ.args([
        "render",
        ULID_A,
        "--to",
        "typst",
        "-o",
        "all-notes/kitchen-sink.typ",
        "-n",
    ]);
    typ.current_dir(dir.path());
    assert!(typ.status().expect("run render to typst").success());

    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/verify-render");
    fs::create_dir_all(&out_dir).expect("create verify-render dir");
    // `--root` at the vault, as ntropy itself compiles: the emitted document
    // addresses its assets from the vault root (ADR 0045), so a hand-compile
    // needs the same root to resolve them.
    let png_status = Command::new("typst")
        .args(["compile", "--format", "png", "--root"])
        .arg(dir.path())
        .arg("kitchen-sink.typ")
        .arg(out_dir.join("kitchen-sink-{p}.png"))
        .current_dir(dir.path().join("all-notes"))
        .status()
        .expect("run typst compile to png");
    assert!(png_status.success(), "png compile failed");

    fs::copy(
        dir.path().join("kitchen-sink.pdf"),
        out_dir.join("kitchen-sink.pdf"),
    )
    .expect("copy pdf");
    fs::copy(
        dir.path().join("all-notes/kitchen-sink.typ"),
        out_dir.join("kitchen-sink.typ"),
    )
    .expect("copy typ");

    println!("verify-render artifacts:");
    println!("  {}", out_dir.join("kitchen-sink.pdf").display());
    println!("  {}", out_dir.join("kitchen-sink.typ").display());
    println!("  {}", out_dir.join("kitchen-sink-1.png").display());
}

#[test]
#[ignore = "runs the real typst binary; execute via `just verify-render`"]
fn render_note_link_reaches_the_real_pdf_as_a_link_to_the_sibling_file() {
    // What the stub cannot show: that the emitted `#link` survives the real
    // compiler as a PDF link annotation, and that its target names a file
    // actually sitting next to it (ADR 0044). Both notes render with no `-o`
    // into the same directory, which is the arrangement the naming contract
    // is built for. Opt-in like the kitchen-sink compile above, since it needs
    // typst installed (ADR 0021).
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "source",
        &format!("---\ntitle: Source\n---\nSee [the target]({ULID_B}-target-note.md).\n"),
    );
    write_note(
        dir.path(),
        ULID_B,
        "target-note",
        "---\ntitle: Target Note\n---\nTarget body.\n",
    );

    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("create output dir");
    for id in [ULID_A, ULID_B] {
        let mut cmd = ntropy(dir.path());
        cmd.args(["render", id, "-n"]);
        cmd.current_dir(&out_dir);
        let output = cmd.output().expect("run render to pdf");
        assert!(
            output.status.success(),
            "pdf compile failed for {id}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // The link target is the name the target note rendered itself under, so
    // the annotation resolves against a file that is really there.
    assert!(
        out_dir.join("target-note.pdf").exists(),
        "the target did not render under the name the link points at"
    );

    // typst writes the annotation's action uncompressed, so the URI is
    // findable in the raw bytes: `/A<</Type/Action/S/URI/URI(target-note.pdf)>>`.
    let pdf = fs::read(out_dir.join("source.pdf")).expect("read the source pdf");
    let needle = b"/URI(target-note.pdf)";
    assert!(
        pdf.windows(needle.len()).any(|window| window == needle),
        "no link annotation targeting the sibling artifact in the rendered pdf"
    );
}

#[test]
#[ignore = "runs the real typst binary; execute via `just verify-render`"]
fn a_theme_reaches_an_asset_outside_all_notes_with_real_typst() {
    // The crux of the theming feature, and the one part only the real
    // compiler can prove: typst sandboxes file access to its root, so a theme
    // referencing `/assets/logo.svg` compiles only because ntropy grants the
    // vault root. The asset stays outside `all-notes/`, which holds notes and
    // nothing else (ADR 0045).
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "report",
        "---\ntitle: Report\ntags: [confidential]\nstatus: draft\n---\nBody text.\n",
    );

    let assets = dir.path().join("assets");
    fs::create_dir_all(&assets).expect("assets dir");
    fs::write(
        assets.join("logo.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 16">
             <rect width="64" height="16" fill="#2dd4bf"/>
           </svg>"##,
    )
    .expect("write logo");

    // A logo in the page header, and no frontmatter strip: the two
    // requirements the feature exists for.
    write_theme(
        dir.path(),
        "corporate",
        r#"#let note(title: none, frontmatter: (:), paper: "a4", body) = {
  set page(
    paper: paper,
    margin: (x: 2.2cm, top: 3.4cm, bottom: 2.4cm),
    header: align(right, image("/assets/logo.svg", width: 3.2cm)),
  )
  if title != none { text(size: 1.6em, weight: "bold", title); v(0.8em) }
  body
}
"#,
    );
    configure_theme(dir.path(), "corporate");

    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "-o", "themed.pdf", "-n"]);
    cmd.current_dir(dir.path());
    let output = cmd.output().expect("run render to pdf");
    assert!(
        output.status.success(),
        "themed compile failed \u{2014} the asset outside all-notes did not resolve:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let pdf = fs::metadata(dir.path().join("themed.pdf")).expect("the artifact exists");
    assert!(pdf.len() > 0, "the themed artifact is empty");
}

#[test]
fn render_survives_a_tool_that_exits_without_reading_stdin() {
    // ntropy restores `SIGPIPE`'s default disposition for its own stdout (see
    // the sigpipe test at the end of this file), so writing the document to a
    // child that has already exited must not let the resulting broken pipe
    // kill the whole process. The stub exits without draining stdin, and the
    // note is made large enough that the document cannot fit into the kernel
    // pipe buffer, so the writer is still writing when the child is gone.
    let dir = setup_vault();
    let big_body = "A line of filler text to inflate the document.\n".repeat(4096);
    write_note(
        dir.path(),
        ULID_A,
        "big",
        &format!("---\ntitle: Big\n---\n{big_body}"),
    );

    // A stub that writes its artifact and exits immediately, stdin untouched.
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.path().join(STUB_BIN);
    fs::create_dir_all(&bin).expect("stub-bin dir");
    let script = r#"#!/bin/sh
out=""
for arg in "$@"; do
  out="$arg"
done
printf 'stub pdf without reading\n' > "$out"
exit 0
"#;
    let path = bin.join("typst");
    fs::write(&path, script).expect("write stub typst");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod stub typst");

    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "-o", "big.pdf", "-p", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", STUB_BIN);
    let output = cmd.output().expect("run render");
    assert!(
        output.status.success(),
        "render must survive the unread pipe; status: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("big.pdf")).expect("artifact exists"),
        "stub pdf without reading\n"
    );
}

#[test]
fn render_paper_config_reaches_the_artifact() {
    // A `[render] paper` setting in the vault config shapes the emitted
    // document: the template application carries the configured paper.
    let dir = setup_vault();
    fs::write(
        dir.path().join(".ntropy/config.toml"),
        "[render]\npaper = \"us-letter\"\n",
    )
    .expect("write config");
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "--to", "typst", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    assert!(cmd.status().expect("run render").success());

    let artifact = fs::read_to_string(dir.path().join("wanted.typ")).expect("typ artifact exists");
    assert!(
        artifact.contains(r#"paper: "us-letter","#),
        "configured paper missing: {artifact}"
    );
}

#[test]
fn render_without_paper_config_defaults_to_a4() {
    // The default vault config has no `[render]` section; the artifact still
    // carries an explicit paper so it compiles identically anywhere.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "--to", "typst", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    assert!(cmd.status().expect("run render").success());

    let artifact = fs::read_to_string(dir.path().join("wanted.typ")).expect("typ artifact exists");
    assert!(
        artifact.contains(r#"paper: "a4","#),
        "default paper missing: {artifact}"
    );
}

#[test]
fn render_unknown_paper_config_errors_naming_the_value() {
    // An unknown paper name is a config parse error surfaced before any scan
    // or render, naming the offending value.
    let dir = setup_vault();
    fs::write(
        dir.path().join(".ntropy/config.toml"),
        "[render]\npaper = \"no-such-paper\"\n",
    )
    .expect("write config");
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nbody\n",
    );
    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "--to", "typst", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    let output = cmd.output().expect("run render");
    assert!(!output.status.success(), "a broken config must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no-such-paper") || stderr.contains("unknown variant"),
        "the error names the bad paper: {stderr}"
    );
    assert!(
        !dir.path().join("wanted.typ").exists(),
        "no artifact on a config error"
    );
}

#[test]
fn render_to_typ_alias_behaves_like_typst() {
    // `typ` is an unlisted alias of `typst`: it produces the identical artifact
    // even though it appears in no help text.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nThe note body text.\n",
    );
    let mut cmd = ntropy(dir.path());
    cmd.args(["render", ULID_A, "--to", "typ", "-n"]);
    cmd.current_dir(dir.path());
    cmd.env("PATH", "no-such-bin");
    let status = cmd.status().expect("run render");
    assert!(status.success());

    let artifact = fs::read_to_string(dir.path().join("wanted.typ")).expect("typ artifact exists");
    assert!(
        artifact.contains("#show: note.with"),
        "template application missing: {artifact}"
    );
    assert!(
        artifact.contains("The note body text"),
        "converted body missing: {artifact}"
    );
}

#[test]
fn render_to_typst_raw_html_warns_and_strict_fails() {
    // Raw HTML degrades: the emitter drops it and warns on stderr. The lenient
    // run still succeeds; `--strict` promotes the engine warning to a failure,
    // exactly as scan warnings behave.
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "wanted",
        "---\ntitle: Wanted\n---\nBefore <div>raw</div> after.\n",
    );
    redacted(dir.path()).bind(|| {
        let mut lenient = ntropy(dir.path());
        lenient.args(["render", ULID_A, "--to", "typst", "-p", "-n"]);
        lenient.current_dir(dir.path());
        lenient.env("PATH", "no-such-bin");
        assert_cmd_snapshot!("render_typst_html_lenient", lenient);

        let mut strict = ntropy(dir.path());
        strict.args(["render", ULID_A, "--to", "typst", "-p", "-n", "--strict"]);
        strict.current_dir(dir.path());
        strict.env("PATH", "no-such-bin");
        assert_cmd_snapshot!("render_typst_html_strict", strict);
    });
}

#[test]
fn info_reports_vault_and_stats() {
    let dir = setup_vault();
    write_note(
        dir.path(),
        ULID_A,
        "a",
        "---\ntitle: A\ntags: [area/work, daily]\n---\n",
    );
    write_note(
        dir.path(),
        ULID_B,
        "b",
        "---\ntitle: B\ntags: [area/work]\n---\n",
    );
    let templates = dir.path().join(".ntropy/templates");
    fs::create_dir_all(&templates).expect("templates dir");
    fs::write(templates.join("default.md"), "x").expect("default template");
    fs::write(templates.join("meeting.md"), "x").expect("meeting template");

    let mut settings = redacted(dir.path());
    // The global default vault is host-specific, so redact that whole line.
    settings.add_filter(r"(?m)^Default vault: .*$", "Default vault: [DEFAULT]");
    settings.bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.arg("info");
        assert_cmd_snapshot!(cmd);
    });
}

/// `--print` backs the `ncd` shell function (`contrib/shell/ntropy.sh`), which
/// substitutes this output straight into `cd`. Stdout therefore carries the
/// path and nothing else, with none of the report's statistics.
#[test]
fn info_print_emits_only_the_vault_path() {
    let dir = setup_vault();
    // A note and a template both appear in the full report, so their absence
    // from the snapshot shows `--print` skipped the vault scan.
    write_note(
        dir.path(),
        ULID_A,
        "a",
        "---\ntitle: A\ntags: [area/work]\n---\n",
    );
    let templates = dir.path().join(".ntropy/templates");
    fs::create_dir_all(&templates).expect("templates dir");
    fs::write(templates.join("default.md"), "x").expect("default template");

    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["info", "--print"]);
        assert_cmd_snapshot!(cmd);
    });
}

/// `-p` is the same flag as `--print`, spelled as on `new`, `today`, `search`
/// and `render` (ADR 0035).
#[test]
fn info_print_short_flag_matches_long() {
    let dir = setup_vault();

    let run = |flag: &str| {
        let mut cmd = ntropy(dir.path());
        cmd.args(["info", flag]);
        let output = cmd.output().expect("run ntropy");
        assert!(output.status.success(), "`info {flag}` failed");
        String::from_utf8(output.stdout).expect("utf-8 stdout")
    };

    let printed = run("--print");
    assert_eq!(printed, run("-p"));

    // Every resolution rule canonicalizes, so a shell can `cd` to the result
    // from any working directory.
    let path = Path::new(printed.trim());
    assert!(
        path.is_absolute(),
        "expected an absolute path, got {printed:?}"
    );
}

/// `ncd` forwards its arguments to `ntropy`, so the global flags must select
/// the printed vault just as they do for every other command.
#[test]
fn info_print_honors_the_vault_flag() {
    let vault = setup_vault();
    let elsewhere = tempfile::tempdir().expect("temp dir");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(elsewhere.path());
    cmd.env_remove("NTROPY_VAULT");
    cmd.arg("--vault").arg(vault.path());
    cmd.args(["info", "--print"]);
    let output = cmd.output().expect("run ntropy");

    assert!(output.status.success(), "`info --print` failed");
    let printed = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let expected = fs::canonicalize(vault.path()).expect("canonicalize vault");
    assert_eq!(Path::new(printed.trim()), expected);
}

/// With no vault to resolve, the command must fail and write nothing to
/// stdout. `ncd` keys off the exit status, and a successful empty run would
/// leave it substituting an empty string into `cd`.
#[test]
fn info_print_fails_without_a_vault() {
    let dir = tempfile::tempdir().expect("temp dir");
    // The global default vault comes from the user's config directory, which
    // `directories` derives from `HOME`, or from `XDG_CONFIG_HOME` where that
    // applies. Pointing both at an empty directory takes the host's own
    // default out of play so no resolution rule can match.
    let home = tempfile::tempdir().expect("home dir");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(dir.path());
    cmd.env_remove("NTROPY_VAULT");
    cmd.env("HOME", home.path());
    cmd.env("XDG_CONFIG_HOME", home.path().join("config"));
    cmd.args(["info", "--print"]);
    let output = cmd.output().expect("run ntropy");

    assert!(
        !output.status.success(),
        "expected a failing exit status without a vault"
    );
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
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
fn edit_alias_lists_multiple_matches() {
    // `edit` is a hidden alias of `search`; a multi-match selector lists the
    // notes non-interactively rather than erroring (ADR 0031).
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
fn reconcile_rewrites_stale_link_targets() {
    let dir = setup_vault();
    write_note(dir.path(), ULID_A, "target", "---\ntitle: Target\n---\n");
    // The source links to the target with a stale slug.
    write_note(
        dir.path(),
        ULID_B,
        "source",
        &format!("---\ntitle: Source\n---\nsee [Target]({ULID_A}-old.md)\n"),
    );
    redacted(dir.path()).bind(|| {
        let mut cmd = ntropy(dir.path());
        cmd.arg("reconcile");
        assert_cmd_snapshot!(cmd);
    });
    let source = std::fs::read_to_string(dir.path().join(format!("all-notes/{ULID_B}-source.md")))
        .expect("read source");
    assert!(source.contains(&format!("[Target]({ULID_A}-target.md)")));
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
fn edit_no_match_exits_nonzero() {
    // A selector matching nothing prints the no-match message and exits
    // non-zero, identical to `search` (ADR 0031).
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

/// A reader that closes the pipe before the first write (e.g. `| head -0`, or
/// any reader that exits early) must make ntropy die quietly with `SIGPIPE`,
/// the Unix CLI convention, rather than panic in `println!`'s write-error
/// path.
///
/// The read end is closed before the child is even spawned, so the child's
/// first stdout write always lands on an already-closed pipe: the outcome
/// does not depend on the pipe buffer size or on winning a race against a
/// reader process.
#[cfg(unix)]
#[test]
fn broken_stdout_pipe_exits_via_sigpipe_not_panic() {
    use std::os::fd::FromRawFd;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;

    let dir = setup_vault();

    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid pointer to two `i32`s, as `pipe(2)` requires.
    let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
    assert_eq!(rc, 0, "pipe() failed");
    let [read_fd, write_fd] = fds;
    // SAFETY: `read_fd` was just returned by `pipe(2)` above and has not been
    // closed yet, so it is a valid, open file descriptor.
    let close_rc = unsafe { libc::close(read_fd) };
    assert_eq!(close_rc, 0, "close(read_fd) failed");

    let mut cmd = ntropy(dir.path());
    cmd.arg("info");
    // SAFETY: `write_fd` was just returned by `pipe(2)` above, is still open
    // (only `read_fd` was closed), and is not owned by any other `Stdio`/`File`
    // in this process, so `Stdio` taking ownership of it is sound.
    cmd.stdout(unsafe { Stdio::from_raw_fd(write_fd) });
    cmd.stderr(Stdio::piped());

    let child = cmd.spawn().expect("spawn ntropy");
    let output = child.wait_with_output().expect("wait for ntropy");

    assert_eq!(
        output.status.signal(),
        Some(libc::SIGPIPE),
        "expected the process to be killed by SIGPIPE, got status: {:?}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "stderr must not contain a panic message, got: {stderr}"
    );
}

/// Encrypted-vault behaviour, compiled only where encryption exists.
#[cfg(feature = "encryption")]
mod encrypted {
    use super::*;

    // =============================================================================
    // Encrypted vaults
    // =============================================================================
    //
    // The key material below is a Rust const rather than a committed key file, so
    // nothing in the repository ever looks like a leaked credential to a scanner.
    // Every encrypted test passes `--identity` (via `$NTROPY_IDENTITY`) and `-n`,
    // so no test can reach the OS credential store or block on a prompt.

    /// A throwaway age keypair used only by this test suite.
    const TEST_IDENTITY: &str =
        "AGE-SECRET-KEY-1MHMUUFG33KU5Y3EFYUS0FNWK98ZTAVCX5ENHD3Y63RGCHN2SZGKQ7KHJNM";
    const TEST_RECIPIENT: &str = "age1k4f8hw3lfpcnq2hxm9w323s8f44ta06y0k59pnmqmgkh8qxfpfpq4cqyfh";

    /// The passphrase used wherever a fixture needs one.
    const TEST_PASSPHRASE: &str = "correct horse battery staple";

    /// Where the fixtures put their key material, relative to the vault root.
    const TEST_IDENTITY_FILE: &str = "test-identity.txt";
    const TEST_PASSPHRASE_FILE: &str = "test-passphrase.txt";

    /// Build an encrypted vault directly on disk, plus the identity file that
    /// opens it.
    fn setup_encrypted_vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        fs::create_dir_all(root.join("all-notes")).expect("all-notes");
        fs::create_dir_all(root.join(".ntropy/templates")).expect(".ntropy");
        fs::write(
            root.join(".ntropy/identity.pub"),
            format!("{TEST_RECIPIENT}\n"),
        )
        .expect("recipient");
        fs::write(
            root.join(".ntropy/templates/default.md"),
            "---\ntitle: {{title}}\n---\n\n# {{title}}\n",
        )
        .expect("template");
        // Kept beside the vault rather than inside it, so it never looks like part
        // of the vault's own contents.
        fs::write(root.join(TEST_IDENTITY_FILE), format!("{TEST_IDENTITY}\n"))
            .expect("identity file");
        fs::write(
            root.join(TEST_PASSPHRASE_FILE),
            format!("{TEST_PASSPHRASE}\n"),
        )
        .expect("passphrase file");
        dir
    }

    /// A `ntropy` command against an encrypted vault, wired so it can neither
    /// prompt nor consult a keychain.
    fn ntropy_encrypted(vault: &Path) -> Command {
        let mut cmd = ntropy(vault);
        // Relative to the command's working directory, which is the vault. An
        // absolute temp path would land in the snapshot's metadata block and churn
        // on every run.
        cmd.env("NTROPY_IDENTITY", TEST_IDENTITY_FILE);
        cmd.arg("-n");
        cmd
    }

    #[test]
    fn new_in_an_encrypted_vault_writes_an_age_file() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_encrypted(vault).args(["new", "-p", "Quarterly Review"]));
        });
    }

    #[test]
    fn a_created_note_is_not_readable_as_plaintext() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        ntropy_encrypted(vault)
            .args(["new", "-p", "Secret Plans"])
            .output()
            .expect("run new");

        let notes: Vec<_> = fs::read_dir(vault.join("all-notes"))
            .expect("read dir")
            .map(|e| e.expect("entry").path())
            .collect();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].extension().and_then(|e| e.to_str()), Some("age"));

        let raw = fs::read(&notes[0]).expect("read note");
        assert!(
            !raw.windows(12).any(|w| w == b"Secret Plans"),
            "the title reached the file in the clear"
        );
    }

    #[test]
    fn search_in_an_encrypted_vault_lists_notes_by_title() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        for title in ["Alpha", "Beta"] {
            ntropy_encrypted(vault)
                .args(["new", "-p", title])
                .output()
                .expect("run new");
        }
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_encrypted(vault).args(["search", "-p"]));
        });
    }

    #[test]
    fn a_locked_encrypted_vault_errors_naming_unlock() {
        // No identity and no way to ask for one: the message has to say what to do.
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        let mut cmd = ntropy(vault);
        cmd.env_remove("NTROPY_IDENTITY");
        cmd.args(["search", "-n", "-p"]);
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(cmd);
        });
    }

    #[test]
    fn write_authors_a_note_in_an_encrypted_vault() {
        // The gap `write` exists to close: authoring content here previously
        // needed an editor, which a script cannot drive.
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        let created = ntropy_encrypted(vault)
            .args(["new", "--empty", "-p", "Written Note"])
            .output()
            .expect("run new");
        let path = String::from_utf8_lossy(&created.stdout)
            .trim_end()
            .to_string();

        let mut cmd = ntropy_encrypted(vault);
        cmd.arg("write").arg(&path);
        let out = super::feed(&mut cmd, super::WRITTEN);
        assert!(out.status.success(), "{}", super::rendered(&out));

        // Stored as ciphertext, and readable back as the note through ntropy.
        let raw = fs::read(&path).expect("read bytes");
        assert!(
            !raw.windows(12).any(|w| w == b"Written Note"),
            "the title reached the file in the clear"
        );
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(
                "write_encrypted_reads_back",
                ntropy_encrypted(vault).args(["search", "-P", "tag:written"])
            );
        });
    }

    #[test]
    fn write_works_on_a_locked_vault() {
        // Encrypting needs only the public recipient, and targeting by name
        // reads no note, so a machine without the key can still author.
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        let created = ntropy_encrypted(vault)
            .args(["new", "--empty", "-p", "Written Note"])
            .output()
            .expect("run new");
        let path = String::from_utf8_lossy(&created.stdout)
            .trim_end()
            .to_string();

        let mut cmd = ntropy(vault);
        cmd.env_remove("NTROPY_IDENTITY");
        cmd.args(["-n", "write"]).arg(&path);
        let out = super::feed(&mut cmd, super::WRITTEN);
        assert!(
            out.status.success(),
            "a locked vault must still accept a write: {}",
            super::rendered(&out)
        );

        // And it really is the note, once a key shows up again.
        let read_back = ntropy_encrypted(vault)
            .args(["search", "-P", "tag:written"])
            .output()
            .expect("run search");
        assert_eq!(String::from_utf8_lossy(&read_back.stdout), super::WRITTEN);
    }

    #[test]
    fn write_refuses_content_that_is_not_a_note_in_an_encrypted_vault() {
        // Validation happens before the cipher, so a rejected write cannot
        // leave unreadable bytes where a note used to be.
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        let created = ntropy_encrypted(vault)
            .args(["new", "-p", "Intact Note"])
            .output()
            .expect("run new");
        let path = String::from_utf8_lossy(&created.stdout)
            .trim_end()
            .to_string();
        let before = fs::read(&path).expect("read bytes");

        let mut cmd = ntropy_encrypted(vault);
        cmd.arg("write").arg(&path);
        let out = super::feed(&mut cmd, "no frontmatter at all\n");
        assert!(!out.status.success());
        assert_eq!(
            fs::read(&path).expect("read bytes"),
            before,
            "a refused write must leave the ciphertext alone"
        );
    }

    #[test]
    fn the_identity_flag_and_env_var_agree() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        ntropy_encrypted(vault)
            .args(["new", "-p", "Same Either Way"])
            .output()
            .expect("run new");

        let via_env = ntropy_encrypted(vault)
            .args(["search", "-p"])
            .output()
            .expect("run search");

        let mut cmd = ntropy(vault);
        cmd.env_remove("NTROPY_IDENTITY");
        let via_flag = cmd
            .args(["-n", "-i", TEST_IDENTITY_FILE, "search", "-p"])
            .output()
            .expect("run search");

        assert_eq!(via_env.stdout, via_flag.stdout);
        assert!(via_env.status.success());
    }

    #[test]
    fn an_identity_file_that_does_not_exist_errors() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        let mut cmd = ntropy(vault);
        cmd.env("NTROPY_IDENTITY", "no-such-identity.txt");
        cmd.args(["search", "-n", "-p"]);
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(cmd);
        });
    }

    #[test]
    fn print_content_reads_the_same_as_a_plaintext_vault() {
        // The whole point of the flag: `-p` names a ciphertext file here, but
        // `-P` gives the note, so a script reads a vault the same way either
        // way it stores its notes.
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        let note = "---\ntitle: Shared\ntags: [work]\n---\nbody line\n";

        // Write it through the CLI so it really is ciphertext on disk.
        fs::write(
            vault.join("all-notes").join(format!("{ULID_A}-shared.md")),
            note,
        )
        .expect("stray plaintext note");
        ntropy_encrypted(vault)
            .arg("reconcile")
            .output()
            .expect("adopt");
        assert!(
            vault
                .join("all-notes")
                .join(format!("{ULID_A}.age"))
                .is_file()
        );

        let output = ntropy_encrypted(vault)
            .args(["search", "-P", ULID_A])
            .output()
            .expect("run");
        assert!(output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), note);
    }

    #[test]
    fn view_add_in_an_encrypted_vault_is_rejected() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(
                ntropy_encrypted(vault).args(["view", "add", "by-tag", "--field", "tags"])
            );
        });
    }

    #[test]
    fn info_reports_encryption_state() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        let mut settings = redacted(vault);
        // The global default vault is host-specific, so redact that whole line.
        settings.add_filter(r"(?m)^Default vault: .*$", "Default vault: [DEFAULT]");
        settings.bind(|| {
            assert_cmd_snapshot!(ntropy_encrypted(vault).arg("info"));
        });
    }

    #[test]
    fn info_on_a_locked_vault_reports_without_asking_for_a_key() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        let mut cmd = ntropy(vault);
        cmd.env_remove("NTROPY_IDENTITY");
        cmd.args(["-n", "info"]);
        let mut settings = redacted(vault);
        settings.add_filter(r"(?m)^Default vault: .*$", "Default vault: [DEFAULT]");
        settings.bind(|| {
            assert_cmd_snapshot!(cmd);
        });
    }

    #[test]
    fn init_encrypted_reports_the_new_vault() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path();
        fs::write(vault.join("pw.txt"), "correct horse\n").expect("write passphrase");

        let mut cmd = ntropy(vault);
        cmd.args(["-n", "init", "--encrypted", "--passphrase-file", "pw.txt"]);

        let mut settings = redacted(vault);
        // The recipient is derived from a keypair generated at run time.
        settings.add_filter(r"age1[0-9a-z]+", "[RECIPIENT]");
        settings.bind(|| {
            assert_cmd_snapshot!(cmd);
        });
    }

    #[test]
    fn init_encrypted_without_a_passphrase_source_errors() {
        // `-n` means no prompt, so there is nowhere for a passphrase to come from.
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy(vault).args(["-n", "init", "--encrypted"]));
        });
    }

    #[test]
    fn an_encrypted_init_seeds_no_view_and_no_gitignore() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path();
        fs::write(vault.join("pw.txt"), "correct horse\n").expect("write passphrase");

        ntropy(vault)
            .args(["-n", "init", "--encrypted", "--passphrase-file", "pw.txt"])
            .output()
            .expect("run init");

        assert!(!vault.join("by-tag").exists());
        assert!(!vault.join(".gitignore").exists());
        assert!(vault.join(".ntropy/identity.pub").is_file());
        assert!(vault.join(".ntropy/identity.age").is_file());
    }

    #[test]
    fn unlock_on_a_plaintext_vault_errors() {
        // Needs no keychain to verify: the vault is refused before any store opens.
        let dir = setup_vault();
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy(vault).args(["-n", "unlock"]));
        });
    }

    #[test]
    fn lock_on_a_plaintext_vault_errors() {
        let dir = setup_vault();
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy(vault).args(["-n", "lock"]));
        });
    }

    // -------------------------------------------------------------------------
    // Whole-vault conversions
    // -------------------------------------------------------------------------

    /// A plaintext vault plus a passphrase file, ready to be converted.
    fn convertible_vault() -> tempfile::TempDir {
        let dir = setup_vault();
        fs::write(
            dir.path().join(TEST_PASSPHRASE_FILE),
            format!("{TEST_PASSPHRASE}\n"),
        )
        .expect("passphrase file");
        dir
    }

    /// `ntropy` against a vault being converted: non-interactive, with a
    /// passphrase file so nothing prompts.
    fn ntropy_convert(vault: &Path) -> Command {
        let mut cmd = ntropy(vault);
        cmd.args(["-n", "--passphrase-file", TEST_PASSPHRASE_FILE]);
        cmd
    }

    #[test]
    fn vault_encrypt_converts_the_whole_vault() {
        let dir = convertible_vault();
        let vault = dir.path();
        write_note(vault, ULID_A, "alpha", "---\ntitle: Alpha\n---\nbody\n");

        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args(["vault", "encrypt", "-y"]));
        });

        assert!(vault.join(".ntropy/identity.pub").is_file());
        assert!(
            !vault
                .join("all-notes")
                .join(format!("{ULID_A}-alpha.md"))
                .exists()
        );
        assert!(
            vault
                .join("all-notes")
                .join(format!("{ULID_A}.age"))
                .is_file()
        );
    }

    #[test]
    fn an_encrypted_vault_is_searchable_afterwards() {
        // The end-to-end proof: convert, then read it back through the CLI.
        let dir = convertible_vault();
        let vault = dir.path();
        write_note(vault, ULID_A, "alpha", "---\ntitle: Alpha\n---\nbody\n");
        ntropy_convert(vault)
            .args(["vault", "encrypt", "-y"])
            .output()
            .expect("encrypt");

        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args(["search", "-p"]));
        });
    }

    #[test]
    fn vault_encrypt_refuses_without_confirmation_when_headless() {
        let dir = convertible_vault();
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args(["vault", "encrypt"]));
        });
    }

    #[test]
    fn vault_encrypt_refuses_an_already_encrypted_vault() {
        let dir = convertible_vault();
        let vault = dir.path();
        ntropy_convert(vault)
            .args(["vault", "encrypt", "-y"])
            .output()
            .expect("encrypt");

        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args(["vault", "encrypt", "-y"]));
        });
    }

    #[test]
    fn an_interrupted_conversion_blocks_the_next_command() {
        let dir = convertible_vault();
        let vault = dir.path();
        write_note(vault, ULID_A, "alpha", "---\ntitle: Alpha\n---\nbody\n");

        let mut cmd = ntropy_convert(vault);
        cmd.env("NTROPY_MIGRATION_FAIL_AFTER", "produce");
        cmd.args(["vault", "encrypt", "-y"]);
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(cmd);
        });

        // Both forms survive, and the marker is there.
        assert!(
            vault
                .join("all-notes")
                .join(format!("{ULID_A}-alpha.md"))
                .is_file()
        );
        assert!(
            vault
                .join("all-notes")
                .join(format!("{ULID_A}.age"))
                .is_file()
        );
        assert!(vault.join(".ntropy/migration.toml").is_file());

        // And every ordinary command now refuses rather than scanning it.
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args(["search", "-p"]));
        });
    }

    #[test]
    fn resume_finishes_an_interrupted_conversion() {
        let dir = convertible_vault();
        let vault = dir.path();
        write_note(vault, ULID_A, "alpha", "---\ntitle: Alpha\n---\nbody\n");

        let mut cmd = ntropy_convert(vault);
        cmd.env("NTROPY_MIGRATION_FAIL_AFTER", "produce");
        cmd.args(["vault", "encrypt", "-y"])
            .output()
            .expect("interrupt");

        redacted(vault).bind(|| {
            assert_cmd_snapshot!(
                ntropy_convert(vault).args(["vault", "encrypt", "--resume", "-y"])
            );
        });

        assert!(!vault.join(".ntropy/migration.toml").exists());
        assert!(
            !vault
                .join("all-notes")
                .join(format!("{ULID_A}-alpha.md"))
                .exists()
        );
        assert!(
            vault
                .join("all-notes")
                .join(format!("{ULID_A}.age"))
                .is_file()
        );
    }

    #[test]
    fn vault_decrypt_restores_the_notes() {
        let dir = convertible_vault();
        let vault = dir.path();
        write_note(vault, ULID_A, "alpha", "---\ntitle: Alpha\n---\nbody\n");
        ntropy_convert(vault)
            .args(["vault", "encrypt", "-y"])
            .output()
            .expect("encrypt");

        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args(["vault", "decrypt", "-y"]));
        });

        assert!(
            vault
                .join("all-notes")
                .join(format!("{ULID_A}-alpha.md"))
                .is_file()
        );
        assert_eq!(
            fs::read_to_string(vault.join("all-notes").join(format!("{ULID_A}-alpha.md")))
                .expect("read"),
            "---\ntitle: Alpha\n---\nbody\n"
        );
        // The key files go with the ciphertext.
        assert!(!vault.join(".ntropy/identity.pub").exists());
        assert!(!vault.join(".ntropy/identity.age").exists());
    }

    #[test]
    fn vault_decrypt_refuses_a_plaintext_vault() {
        let dir = convertible_vault();
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args(["vault", "decrypt", "-y"]));
        });
    }

    #[test]
    fn vault_rekey_keeps_the_passphrase_by_default() {
        // Rekey is about replacing the key, not the passphrase, so the one
        // that opened the old key wraps the new one unless told otherwise.
        let dir = convertible_vault();
        let vault = dir.path();
        write_note(vault, ULID_A, "alpha", "---\ntitle: Alpha\n---\nbody\n");
        ntropy_convert(vault)
            .args(["vault", "encrypt", "-y"])
            .output()
            .expect("encrypt");

        redacted(vault).bind(|| {
            let mut settings = insta::Settings::clone_current();
            settings.add_filter(r"age1[0-9a-z]+", "[RECIPIENT]");
            settings.bind(|| {
                assert_cmd_snapshot!(ntropy_convert(vault).args(["vault", "rekey", "-y"]));
            });
        });

        // The original passphrase still opens the vault.
        let output = ntropy_convert(vault)
            .args(["search", "-p"])
            .output()
            .expect("search");
        assert!(output.status.success(), "the passphrase must be unchanged");
    }

    #[test]
    fn vault_rekey_can_set_a_new_passphrase() {
        let dir = convertible_vault();
        let vault = dir.path();
        write_note(vault, ULID_A, "alpha", "---\ntitle: Alpha\n---\nbody\n");
        ntropy_convert(vault)
            .args(["vault", "encrypt", "-y"])
            .output()
            .expect("encrypt");

        fs::write(vault.join("new-pw.txt"), "a different passphrase\n").expect("write");
        let rekey = ntropy_convert(vault)
            .args([
                "vault",
                "rekey",
                "-y",
                "--new-passphrase-file",
                "new-pw.txt",
            ])
            .output()
            .expect("rekey");
        assert!(rekey.status.success(), "{rekey:?}");

        // The old passphrase no longer opens it; the new one does.
        let mut old = ntropy(vault);
        old.args([
            "-n",
            "--passphrase-file",
            TEST_PASSPHRASE_FILE,
            "search",
            "-p",
        ]);
        assert!(!old.output().expect("search").status.success());

        let mut new = ntropy(vault);
        new.args(["-n", "--passphrase-file", "new-pw.txt", "search", "-p"]);
        assert!(new.output().expect("search").status.success());
    }

    #[test]
    fn vault_passphrase_changes_the_wrapper_and_leaves_notes_alone() {
        let dir = convertible_vault();
        let vault = dir.path();
        write_note(vault, ULID_A, "alpha", "---\ntitle: Alpha\n---\nbody\n");
        ntropy_convert(vault)
            .args(["vault", "encrypt", "-y"])
            .output()
            .expect("encrypt");

        let note = vault.join("all-notes").join(format!("{ULID_A}.age"));
        let before = fs::read(&note).expect("read note");
        fs::write(vault.join("new-pw.txt"), "a different passphrase\n").expect("write");

        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args([
                "vault",
                "passphrase",
                "--new-passphrase-file",
                "new-pw.txt",
            ]));
        });

        assert_eq!(
            fs::read(&note).expect("read note"),
            before,
            "notes must be untouched"
        );
    }

    #[test]
    fn vault_passphrase_refuses_a_plaintext_vault() {
        let dir = convertible_vault();
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_convert(vault).args(["vault", "passphrase"]));
        });
    }

    #[test]
    fn reconcile_adopts_a_hand_added_plaintext_note() {
        let dir = setup_encrypted_vault();
        let vault = dir.path();
        fs::write(
            vault
                .join("all-notes")
                .join(format!("{ULID_A}-dropped-in.md")),
            "---\ntitle: Dropped In\n---\nbody\n",
        )
        .expect("write stray note");

        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy_encrypted(vault).arg("reconcile"));
        });
        assert!(
            !vault
                .join("all-notes")
                .join(format!("{ULID_A}-dropped-in.md"))
                .exists()
        );
        assert!(
            vault
                .join("all-notes")
                .join(format!("{ULID_A}.age"))
                .is_file()
        );
    }
}

/// What a build compiled without encryption support says when asked.
#[cfg(not(feature = "encryption"))]
mod without_encryption {
    use super::*;

    #[test]
    fn unlock_reports_the_missing_feature() {
        let dir = setup_vault();
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy(vault).args(["-n", "unlock"]));
        });
    }

    #[test]
    fn init_encrypted_reports_the_missing_feature() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = dir.path();
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy(vault).args(["-n", "init", "--encrypted"]));
        });
    }

    #[test]
    fn an_encrypted_vault_is_recognized_and_refused() {
        // The point of keeping detection compiled in: the vault is named
        // as encrypted rather than reported as a directory full of
        // corrupt notes.
        let dir = setup_vault();
        let vault = dir.path();
        fs::write(vault.join(".ntropy/identity.pub"), "age1example\n").expect("write recipient");
        redacted(vault).bind(|| {
            assert_cmd_snapshot!(ntropy(vault).args(["-n", "search", "-p"]));
        });
    }
}
