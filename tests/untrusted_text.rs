// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! CLI contract tests for text from the vault that reaches the terminal
//! (ADR 0071).
//!
//! Each test plants an escape sequence where a vault someone else wrote can
//! hold one (a title, a file or directory name, a link target, a config
//! value) and checks that ntropy shows it escaped, never as the raw bytes a
//! terminal would act on, and that a path it cannot print as it is is not
//! printed at all. Every invocation pins its vault with `--vault`, so vault
//! discovery can never reach a real vault.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use insta_cmd::assert_cmd_snapshot;

const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ULID_B: &str = "01BRZ3NDEKTSV4RRFFQ69G5FAV";

/// An OSC sequence that sets the terminal's title, the sort of payload a
/// hostile name carries.
const RETITLE: &str = "\u{1b}]0;pwned\u{7}";

/// [`RETITLE`] as ntropy shows it.
const RETITLE_SHOWN: &str = "\\u{1b}]0;pwned\\u{7}";

/// A vault at `<base>/<name>` with a `by-tag` view, the directory name taken
/// as it is.
fn vault_at(base: &Path, name: &str) -> PathBuf {
    let root = base.join(name);
    fs::create_dir_all(root.join("all-notes")).expect("all-notes");
    fs::create_dir_all(root.join(".ntropy/templates")).expect(".ntropy");
    fs::write(
        root.join(".ntropy/config.toml"),
        "[[view]]\nname = \"by-tag\"\nfield = \"tags\"\n",
    )
    .expect("config");
    fs::write(
        root.join(".ntropy/templates/default.md"),
        "---\ntitle: {{title}}\n---\n",
    )
    .expect("default template");
    fs::write(
        root.join(".ntropy/templates/today.md"),
        "---\ntitle: {{date}}\n---\n",
    )
    .expect("today template");
    root
}

/// Write `content` to `all-notes/<file>`.
fn note(vault: &Path, file: &str, content: &str) {
    fs::write(vault.join("all-notes").join(file), content).expect("write note");
}

/// An `ntropy -n` command pinned to `vault` and run from inside it, with no
/// editor and nothing inherited that could pick another vault or identity.
fn ntropy(vault: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(vault);
    cmd.args(["--vault", ".", "-n"]);
    cmd.env("EDITOR", "true");
    cmd.env("NTROPY_CONFIG_DIR", "no-such-config-dir");
    cmd.env_remove("VISUAL");
    cmd.env_remove("NTROPY_VAULT");
    cmd.env_remove("NTROPY_IDENTITY");
    cmd
}

fn run(cmd: &mut Command) -> Output {
    cmd.output().expect("run ntropy")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Assert that neither stream carries a byte a terminal acts on.
fn assert_no_raw_controls(output: &Output) {
    for (stream, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
        assert!(
            !bytes.iter().any(|b| *b == 0x1b || *b == 0x07),
            "raw control bytes on {stream}: {:?}",
            String::from_utf8_lossy(bytes)
        );
    }
}

/// Snapshot settings that redact the temporary paths, ULIDs and dates.
fn redacted(base: &Path) -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    if let Ok(canon) = fs::canonicalize(base) {
        settings.add_filter(&regex::escape(&canon.to_string_lossy()), "[BASE]");
    }
    settings.add_filter(&regex::escape(&base.to_string_lossy()), "[BASE]");
    settings.add_filter(r"\d{4}-\d{2}-\d{2}", "[DATE....]");
    settings
}

// =============================================================================
// The plain output
// =============================================================================

#[test]
fn search_escapes_a_hostile_title() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-evil.md"),
        "---\ntitle: \"Evil\\e]0;pwned\\a\"\ntags: [work]\n---\n",
    );
    note(
        &vault,
        &format!("{ULID_B}-two-lines.md"),
        "---\ntitle: \"Two\\nLines\\tTabbed\"\n---\n",
    );

    let output = run(ntropy(&vault).arg("search"));
    assert_no_raw_controls(&output);
    redacted(dir.path()).bind(|| {
        assert_cmd_snapshot!(ntropy(&vault).arg("search"));
    });
}

#[test]
fn search_escapes_a_hostile_file_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-{RETITLE}.md"),
        "---\ntitle: Plain\n---\n",
    );

    let output = run(ntropy(&vault).arg("search"));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!("{ULID_A}-{RETITLE_SHOWN}.md")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_skipped_file_is_named_escaped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-{RETITLE}.md"),
        "no frontmatter\n",
    );

    let output = run(ntropy(&vault).arg("search"));
    assert_no_raw_controls(&output);
    assert!(
        stderr(&output).contains(&format!("warning: skipped `{ULID_A}-{RETITLE_SHOWN}.md`")),
        "{}",
        stderr(&output)
    );
}

#[test]
fn delete_names_a_hostile_note_escaped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-evil.md"),
        "---\ntitle: \"Evil\\e]0;pwned\\a\"\n---\n",
    );

    let output = run(ntropy(&vault).args(["delete", ULID_A]));
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    assert!(
        stderr(&output).contains(&format!("Evil{RETITLE_SHOWN}")),
        "{}",
        stderr(&output)
    );
}

#[test]
fn view_list_escapes_a_hostile_view() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    fs::write(
        vault.join(".ntropy/config.toml"),
        "[[view]]\nname = \"a\\u001bb\"\nfield = \"f\\u001bg\"\n",
    )
    .expect("config");

    let output = run(ntropy(&vault).args(["view", "list"]));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains("a\\u{1b}b  f\\u{1b}g"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn info_escapes_a_hostile_template_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    fs::write(
        vault.join(format!(".ntropy/templates/{RETITLE}.md")),
        "---\ntitle: {{title}}\n---\n",
    )
    .expect("template");

    let output = run(ntropy(&vault).arg("info"));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!("  {RETITLE_SHOWN}\n")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn info_escapes_a_hostile_vault_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), &format!("vault{RETITLE}"));

    let output = run(ntropy(&vault).arg("info"));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!("vault{RETITLE_SHOWN} (via --vault flag)")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn info_escapes_a_hostile_pointer_file_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    vault_at(dir.path(), "vault");
    let project = dir.path().join(format!("project{RETITLE}"));
    fs::create_dir_all(&project).expect("project dir");
    fs::write(project.join(".ntropy-vault"), "../vault\n").expect("pointer file");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(&project)
        .env("NTROPY_CONFIG_DIR", "no-such-config-dir")
        .env_remove("NTROPY_VAULT")
        .args(["-n", "info"]);
    let output = run(&mut cmd);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!("project{RETITLE_SHOWN}/.ntropy-vault)")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn init_escapes_a_hostile_vault_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let target = dir.path().join(format!("vault{RETITLE}"));

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(dir.path())
        .env("NTROPY_CONFIG_DIR", "no-such-config-dir")
        .env_remove("NTROPY_VAULT")
        .args(["-n", "init"])
        .arg(&target);
    let output = run(&mut cmd);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains("Initialized vault at "),
        "{}",
        stdout(&output)
    );
    assert!(
        stdout(&output).contains(&format!("vault{RETITLE_SHOWN}\n")),
        "{}",
        stdout(&output)
    );
}

// =============================================================================
// The reconcile report
// =============================================================================

#[test]
fn reconcile_escapes_a_hostile_vault_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), &format!("vault{RETITLE}"));

    let output = run(ntropy(&vault).arg("reconcile"));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!("vault{RETITLE_SHOWN}...")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn reconcile_reports_a_hostile_file_name_escaped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-{RETITLE}.md"),
        "---\ntitle: Evil\n---\n",
    );

    let output = run(ntropy(&vault).arg("reconcile"));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!(
            "renamed {ULID_A}-{RETITLE_SHOWN}.md -> {ULID_A}-evil.md"
        )),
        "{}",
        stdout(&output)
    );
}

#[test]
fn reconcile_reports_a_hostile_link_target_escaped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-evil.md"),
        "---\ntitle: Evil\n---\n",
    );
    note(
        &vault,
        &format!("{ULID_B}-linker.md"),
        &format!("---\ntitle: Linker\n---\n[x]({ULID_A}-{RETITLE}.md)\n"),
    );

    let output = run(ntropy(&vault).arg("reconcile"));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!(
            "relinked {ULID_A}-{RETITLE_SHOWN}.md -> {ULID_A}-evil.md in {ULID_B}-linker.md"
        )),
        "{}",
        stdout(&output)
    );
}

#[test]
fn reconcile_reports_a_hostile_copy_escaped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-plan.md"),
        "---\ntitle: Plan\n---\n",
    );
    note(
        &vault,
        &format!("{ULID_A}-plan-{RETITLE}.md"),
        "---\ntitle: Plan copy\n---\n",
    );

    let output = run(ntropy(&vault).arg("reconcile"));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!("reassigned {ULID_A}-plan-{RETITLE_SHOWN}.md -> ")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn reconcile_reports_a_hostile_gitignore_entry_escaped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    fs::write(
        vault.join(".gitignore"),
        format!("# ntropy: derived view directory, safe to ignore\n/x{RETITLE}/\n"),
    )
    .expect(".gitignore");

    let output = run(ntropy(&vault).arg("reconcile"));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!(
            "stopped ignoring /x{RETITLE_SHOWN}/; left directory `x{RETITLE_SHOWN}/` in place"
        )),
        "{}",
        stdout(&output)
    );
}

// =============================================================================
// Rendering and site export
// =============================================================================

#[test]
fn render_escapes_an_engine_warning() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-html.md"),
        &format!("---\ntitle: Html\n---\n<div>{RETITLE}</div>\n"),
    );

    let output = run(ntropy(&vault).args(["render", "--to", "typst", "-o", "out.typ", ULID_A]));
    assert!(output.status.success(), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    assert!(
        stderr(&output).contains(&format!("warning: dropped raw HTML: <div>{RETITLE_SHOWN}")),
        "{}",
        stderr(&output)
    );
}

#[test]
fn render_names_a_hostile_default_output_escaped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-x{RETITLE}.md"),
        "---\ntitle: Plain\n---\nbody\n",
    );

    let output = run(ntropy(&vault).args(["render", "--to", "typst", ULID_A]));
    assert!(output.status.success(), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    assert!(
        stdout(&output).contains(&format!("Rendered x{RETITLE_SHOWN}.typ (typst via typst, ")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn site_escapes_an_export_warning() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-picture.md"),
        &format!("---\ntitle: Picture\n---\n![x](<missing{RETITLE}.png>)\n"),
    );

    let output = run(ntropy(&vault).args(["site", "-o", "out"]));
    assert!(output.status.success(), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    assert!(
        stderr(&output).contains(&format!("missing{RETITLE_SHOWN}.png")),
        "{}",
        stderr(&output)
    );
}

// =============================================================================
// Paths printed for scripts
// =============================================================================

#[test]
fn search_print_skips_a_hostile_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-{RETITLE}.md"),
        "---\ntitle: Hostile\n---\n",
    );
    note(
        &vault,
        &format!("{ULID_B}-clean.md"),
        "---\ntitle: Clean\n---\n",
    );

    let output = run(ntropy(&vault).args(["search", "-p"]));
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    let printed = stdout(&output);
    assert_eq!(printed.lines().count(), 1, "{printed}");
    assert!(
        printed.ends_with(&format!("{ULID_B}-clean.md\n")),
        "{printed}"
    );
    assert!(
        stderr(&output).contains(&format!(
            "{ULID_A}-{RETITLE_SHOWN}.md` holds control or bidi characters, so it is not \
             printed; run `ntropy reconcile` to rename the note"
        )),
        "{}",
        stderr(&output)
    );
}

#[test]
fn render_print_refuses_a_hostile_default_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), "vault");
    note(
        &vault,
        &format!("{ULID_A}-x{RETITLE}.md"),
        "---\ntitle: Plain\n---\nbody\n",
    );

    let output = run(ntropy(&vault).args(["render", "--to", "typst", "-p", ULID_A]));
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    assert!(
        !vault.join(format!("x{RETITLE}.typ")).exists(),
        "the refused artifact was written"
    );
    assert!(
        stderr(&output).contains("run `ntropy reconcile` to rename the note"),
        "{}",
        stderr(&output)
    );
}

/// Each `-p` path that holds the vault's root, from a vault whose directory
/// name needs escaping.
#[test]
fn print_refuses_a_path_under_a_hostile_vault_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), &format!("vault{RETITLE}"));
    note(
        &vault,
        &format!("{ULID_A}-plan.md"),
        "---\ntitle: Plan\n---\n",
    );

    for (what, mut cmd) in [
        ("info", {
            let mut cmd = ntropy(&vault);
            cmd.args(["info", "-p"]);
            cmd
        }),
        ("new", {
            let mut cmd = ntropy(&vault);
            cmd.args(["new", "-p", "Fresh"]);
            cmd
        }),
        ("today", {
            let mut cmd = ntropy(&vault);
            cmd.args(["today", "-p"]);
            cmd
        }),
        ("site", {
            let mut cmd = ntropy(&vault);
            cmd.args(["site", "-p", "-o"])
                .arg(dir.path().join(format!("out{RETITLE}")));
            cmd
        }),
    ] {
        let output = run(&mut cmd);
        assert_eq!(output.status.code(), Some(1), "{what}: {}", stderr(&output));
        assert_no_raw_controls(&output);
        assert!(stdout(&output).is_empty(), "{what}: {}", stdout(&output));
        assert!(
            stderr(&output).contains("holds control or bidi characters, so it is not printed"),
            "{what}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn write_refuses_to_print_a_path_under_a_hostile_vault_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vault = vault_at(dir.path(), &format!("vault{RETITLE}"));
    note(
        &vault,
        &format!("{ULID_A}-plan.md"),
        "---\ntitle: Plan\n---\n",
    );

    let mut cmd = ntropy(&vault);
    cmd.args(["write", ULID_A])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("run ntropy");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin
            .write_all(b"---\ntitle: Plan\n---\nnew body\n")
            .expect("write the note to stdin");
    }
    let output = child.wait_with_output().expect("wait for ntropy");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_no_raw_controls(&output);
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("holds control or bidi characters, so it is not printed"),
        "{}",
        stderr(&output)
    );
}

// =============================================================================
// The interactive lone match, on a terminal of its own
// =============================================================================

#[cfg(unix)]
mod on_a_terminal {
    use super::*;

    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    /// Run `cmd` to completion with a fresh pseudo-terminal as its
    /// controlling terminal, so ntropy takes its interactive branch, and
    /// return its piped stdout and stderr.
    ///
    /// The same harness as the editor-launch tests in `tests/cli.rs`, minus
    /// the recording of what was written to the terminal.
    fn run_on_own_terminal(cmd: &mut Command) -> Output {
        let mut master = -1;
        let mut slave = -1;
        // SAFETY: `master` and `slave` are valid pointers to `c_int`s, and the
        // name, termios and winsize arguments may be null. `null_mut` is used
        // because macOS declares them `*mut` while Linux declares them `*const`.
        let rc = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, 0, "openpty failed: {}", io::Error::last_os_error());
        // SAFETY: both descriptors were just returned by `openpty`, are open,
        // and nothing else in this process owns them.
        let (master, slave) =
            unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
        // `openpty` has no close-on-exec variant; without it a child another
        // test thread forks could hold the terminal open.
        for fd in [&master, &slave] {
            // SAFETY: `fd` is an open descriptor owned by this function.
            let rc = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
            assert_ne!(rc, -1, "fcntl failed: {}", io::Error::last_os_error());
        }

        cmd.stdin(Stdio::from(slave))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: the closure runs in the forked child before `exec` and calls
        // only `setsid` and `ioctl`, which are async-signal-safe and allocate
        // nothing. std has already moved the slave onto fd 0 when pre-exec
        // closures run, so fd 0 is the terminal to claim.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn().expect("run ntropy on its own terminal");
        // Leave the child as the only holder of the slave, so the terminal's
        // output ends when it exits.
        cmd.stdin(Stdio::null());

        // A session leader's exit waits for its terminal's output to drain,
        // so the terminal is read until it closes.
        let reader = std::thread::spawn(move || {
            let mut terminal = fs::File::from(master);
            let mut seen = Vec::new();
            let _ = io::Read::read_to_end(&mut terminal, &mut seen);
        });
        let output = child.wait_with_output().expect("wait for ntropy");
        reader.join().expect("the terminal reader does not panic");
        output
    }

    #[test]
    fn search_print_refuses_a_lone_hostile_match() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = vault_at(dir.path(), "vault");
        note(
            &vault,
            &format!("{ULID_A}-{RETITLE}.md"),
            "---\ntitle: Hostile\n---\n",
        );

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
        cmd.current_dir(&vault)
            .args(["--vault", ".", "search", "-p", ULID_A])
            .env("EDITOR", "true")
            .env("NTROPY_CONFIG_DIR", "no-such-config-dir")
            .env_remove("VISUAL")
            .env_remove("NTROPY_VAULT");
        let output = run_on_own_terminal(&mut cmd);

        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        assert_no_raw_controls(&output);
        assert!(stdout(&output).is_empty(), "{}", stdout(&output));
        assert!(
            stderr(&output).contains("run `ntropy reconcile` to rename the note"),
            "{}",
            stderr(&output)
        );
    }
}

// =============================================================================
// Encrypted vaults
// =============================================================================

#[cfg(feature = "encryption")]
mod encrypted {
    use super::*;

    /// A throwaway age keypair used only by these tests.
    const TEST_IDENTITY: &str =
        "AGE-SECRET-KEY-1MHMUUFG33KU5Y3EFYUS0FNWK98ZTAVCX5ENHD3Y63RGCHN2SZGKQ7KHJNM";
    const TEST_RECIPIENT: &str = "age1k4f8hw3lfpcnq2hxm9w323s8f44ta06y0k59pnmqmgkh8qxfpfpq4cqyfh";

    /// Where the fixtures put their key material, relative to the vault root.
    const TEST_IDENTITY_FILE: &str = "test-identity.txt";
    const TEST_PASSPHRASE_FILE: &str = "test-passphrase.txt";

    #[test]
    fn reconcile_reports_an_adopted_hostile_file_escaped() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = vault_at(dir.path(), "vault");
        fs::remove_file(vault.join(".ntropy/config.toml")).expect("no views");
        fs::write(
            vault.join(".ntropy/identity.pub"),
            format!("{TEST_RECIPIENT}\n"),
        )
        .expect("recipient");
        fs::write(vault.join(TEST_IDENTITY_FILE), format!("{TEST_IDENTITY}\n"))
            .expect("identity file");
        note(
            &vault,
            &format!("{ULID_A}-{RETITLE}.md"),
            "---\ntitle: Dropped In\n---\nbody\n",
        );

        let output = run(ntropy(&vault)
            .env("NTROPY_IDENTITY", TEST_IDENTITY_FILE)
            .arg("reconcile"));
        assert!(output.status.success(), "{}", stderr(&output));
        assert_no_raw_controls(&output);
        assert!(
            stdout(&output).contains(&format!(
                "encrypted {ULID_A}-{RETITLE_SHOWN}.md -> {ULID_A}.age"
            )),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn encrypt_refuses_a_hostile_view_without_a_raw_name() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = vault_at(dir.path(), "vault");
        fs::write(
            vault.join(".ntropy/config.toml"),
            "[[view]]\nname = \"a\\u001bb\"\nfield = \"tags\"\n",
        )
        .expect("config");
        fs::write(
            vault.join(TEST_PASSPHRASE_FILE),
            "correct horse battery staple\n",
        )
        .expect("passphrase file");

        let output = run(ntropy(&vault).args([
            "--passphrase-file",
            TEST_PASSPHRASE_FILE,
            "vault",
            "encrypt",
            "-y",
        ]));
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        assert_no_raw_controls(&output);
        assert!(
            stderr(&output).contains(
                "view `a\\u{1b}b`: the name contains a control character; remove it from \
                 `.ntropy/config.toml`"
            ),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn encrypt_names_a_removed_view_directory_escaped() {
        let dir = tempfile::tempdir().expect("temp dir");
        let vault = vault_at(dir.path(), &format!("vault{RETITLE}"));
        note(
            &vault,
            &format!("{ULID_A}-plan.md"),
            "---\ntitle: Plan\ntags: [work]\n---\n",
        );
        fs::create_dir_all(vault.join("by-tag/work")).expect("view tree");
        fs::write(
            vault.join(TEST_PASSPHRASE_FILE),
            "correct horse battery staple\n",
        )
        .expect("passphrase file");

        let output = run(ntropy(&vault).args([
            "--passphrase-file",
            TEST_PASSPHRASE_FILE,
            "vault",
            "encrypt",
            "-y",
        ]));
        assert!(output.status.success(), "{}", stderr(&output));
        assert_no_raw_controls(&output);
        assert!(
            stdout(&output).contains("removed view directory "),
            "{}",
            stdout(&output)
        );
        assert!(
            stdout(&output).contains(&format!("vault{RETITLE_SHOWN}/by-tag\n")),
            "{}",
            stdout(&output)
        );
    }
}
