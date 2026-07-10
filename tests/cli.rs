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
    // location.
    let script = r#"#!/bin/sh
cat > /dev/null
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
    let png_status = Command::new("typst")
        .args(["compile", "--format", "png", "kitchen-sink.typ"])
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
