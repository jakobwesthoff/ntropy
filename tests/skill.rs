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
const PARTS: [&str; 8] = [
    "writing-notes",
    "querying",
    "vaults",
    "views",
    "rendering",
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

/// Cuts `cmd` off from the host: its vault selection, its global config, and
/// every place the skills of whoever runs the tests could be found, the home
/// directory and the variables that move a skills directory. `HOME` is
/// removed, not set, since insta-cmd records every variable a command sets in
/// the snapshot, and a test that needs a home sets its own after this.
fn isolate(cmd: &mut Command) {
    cmd.env_remove("NTROPY_VAULT");
    cmd.env("NTROPY_CONFIG_DIR", NO_CONFIG_DIR);
    for variable in [
        "HOME",
        "CLAUDE_CONFIG_DIR",
        "CODEX_HOME",
        "COPILOT_HOME",
        "GEMINI_CLI_HOME",
    ] {
        cmd.env_remove(variable);
    }
}

/// `ntropy` run in `cwd`, isolated from the host (see [`isolate`]).
fn ntropy(cwd: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ntropy"));
    cmd.current_dir(cwd);
    isolate(&mut cmd);
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
// The command reference of the core
// =========================================================

/// The lines of the core's "Command reference" section, up to the next
/// heading.
fn command_reference() -> Vec<String> {
    let core = repository_file("SKILL.md");
    core.lines()
        .skip_while(|line| *line != "## Command reference")
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .map(str::to_owned)
        .collect()
}

/// The commands `ntropy --help` lists, without `help`, which is clap's.
fn listed_commands() -> Vec<String> {
    let dir = empty_dir();
    let output = ntropy(dir.path()).arg("--help").output().expect("a run");
    let help = stdout_of(&output);
    help.lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(str::to_owned)
        .collect()
}

/// Every command of the CLI has a row in the core's command reference, so
/// an agent that reads only the core knows each exists. A row starts with the
/// command in backticks.
#[test]
fn the_command_reference_has_a_row_for_every_command() {
    let rows = command_reference();
    let commands = listed_commands();
    assert!(commands.len() >= 15, "parsed {commands:?}");

    for command in commands {
        let mention = format!("`ntropy {command}");
        assert!(
            rows.iter().any(|row| {
                // The first cell of the table row names the command.
                let first_cell = row.split('|').nth(1).unwrap_or("");
                first_cell.match_indices(&mention).any(|(at, _)| {
                    first_cell[at + mention.len()..]
                        .chars()
                        .next()
                        .is_none_or(|next| matches!(next, ' ' | '`' | '\\'))
                })
            }),
            "no row for `ntropy {command}`"
        );
    }
}

/// The subcommands of the groups are in their group's row, where an agent
/// looks for them.
#[test]
fn the_command_reference_names_the_subcommands_of_the_groups() {
    let rows = command_reference().join("\n");
    for name in [
        "site theme init",
        "asset add",
        "asset list",
        "view list",
        "view add",
        "view remove",
        "vault encrypt",
        "vault decrypt",
        "vault rekey",
        "vault passphrase",
    ] {
        let words: Vec<&str> = name.split(' ').collect();
        let (group, sub) = (words[0], words[words.len() - 1]);
        assert!(
            rows.contains(group) && rows.contains(sub),
            "the command reference lacks {name}"
        );
    }
}

/// The global flags are named as flags, in backticks.
#[test]
fn the_core_names_the_global_flags() {
    let core = repository_file("SKILL.md");
    for flag in [
        "`--vault",
        "`-n`",
        "`--strict`",
        "`-i`",
        "`--identity",
        "`--passphrase-file",
    ] {
        assert!(core.contains(flag), "the core lacks {flag}");
    }
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

// =========================================================
// The warning about an older installed copy
// =========================================================

/// The version of every copy the tests install as an older one.
const OLD: &str = "1.0.0";

/// A project directory with a repository root, and a home directory apart
/// from it, that no skills of whoever runs the tests can reach: the
/// project's walk up ends at its own `.git`, and `HOME` and every variable
/// that moves a skills directory are set or removed.
struct Scene {
    root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Scene {
        let root = empty_dir();
        fs::create_dir_all(root.path().join("project/.git")).expect("a creatable directory");
        fs::create_dir_all(root.path().join("home")).expect("a creatable directory");
        Scene { root }
    }

    fn project(&self) -> PathBuf {
        self.root.path().join("project")
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    /// `ntropy` run in the project with this scene's home.
    fn ntropy(&self) -> Command {
        let mut cmd = ntropy(&self.project());
        cmd.env("HOME", self.home());
        cmd
    }

    /// Installs a `SKILL.md` under `<base>/<skills_dir>/ntropy`, stamped
    /// `version`, or without a stamp.
    fn put_copy(&self, base: &Path, skills_dir: &str, version: Option<&str>) {
        let dir = base.join(skills_dir).join("ntropy");
        fs::create_dir_all(&dir).expect("a creatable directory");
        let stamp = version
            .map(|version| format!("metadata:\n  ntropy-version: \"{version}\"\n"))
            .unwrap_or_default();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: ntropy\ndescription: Old.\n{stamp}---\n\nOld body\n"),
        )
        .expect("a writable file");
    }

    /// The stderr of a run as text for a snapshot: the temp directories and
    /// the running version are replaced, as they differ per run and per
    /// release. Snapshotted as plain text since insta-cmd would record the
    /// `HOME` of the run.
    fn stderr(&self, output: &Output) -> String {
        let root = self.root.path();
        let canonical = root.canonicalize().expect("an existing directory");
        String::from_utf8(output.stderr.clone())
            .expect("a UTF-8 warning")
            .replace(&canonical.display().to_string(), "[TMP]")
            .replace(&root.display().to_string(), "[TMP]")
            .replace(env!("CARGO_PKG_VERSION"), "[VERSION]")
    }
}

/// `ntropy --version` prints clap's version text and nothing else when no
/// copy is older, in both spellings of the flag.
#[test]
fn version_prints_the_version_of_this_ntropy() {
    let scene = Scene::new();

    for flag in ["--version", "-V"] {
        let output = scene.ntropy().arg(flag).output().expect("a run");

        assert!(output.status.success(), "{flag}");
        assert_eq!(
            stdout_of(&output),
            format!("ntropy {}\n", env!("CARGO_PKG_VERSION")),
            "{flag}"
        );
        assert_eq!(output.stderr, b"", "{flag}");
    }
}

#[test]
fn an_older_project_copy_warns_after_the_skill() {
    let scene = Scene::new();
    scene.put_copy(&scene.project(), ".claude/skills", Some(OLD));

    let output = scene.ntropy().arg("skill").output().expect("a run");

    assert!(output.status.success());
    assert_eq!(stdout_of(&output), repository_file("SKILL.md"));
    insta::assert_snapshot!(scene.stderr(&output));
}

#[test]
fn a_copy_without_a_stamp_warns() {
    let scene = Scene::new();
    scene.put_copy(&scene.project(), ".agents/skills", None);

    let output = scene.ntropy().arg("skill").output().expect("a run");

    insta::assert_snapshot!(scene.stderr(&output));
}

#[test]
fn a_global_copy_under_home_is_named_from_home() {
    let scene = Scene::new();
    scene.put_copy(&scene.home(), ".claude/skills", Some(OLD));

    let output = scene.ntropy().arg("skill").output().expect("a run");

    insta::assert_snapshot!(scene.stderr(&output));
}

#[test]
fn a_part_warns_too() {
    let scene = Scene::new();
    scene.put_copy(&scene.project(), ".claude/skills", Some(OLD));

    let output = scene
        .ntropy()
        .args(["skill", "querying"])
        .output()
        .expect("a run");

    assert_eq!(
        stdout_of(&output),
        repository_file("references/querying.md")
    );
    assert!(
        scene
            .stderr(&output)
            .starts_with("warning: the ntropy skill in "),
        "{}",
        scene.stderr(&output)
    );
}

#[test]
fn a_copy_of_this_version_gives_no_warning() {
    let scene = Scene::new();
    scene.put_copy(
        &scene.project(),
        ".claude/skills",
        Some(env!("CARGO_PKG_VERSION")),
    );

    let output = scene.ntropy().arg("skill").output().expect("a run");

    assert_eq!(output.stderr, b"");
}

#[test]
fn a_fresh_install_gives_no_warning() {
    // The stamp the binary ships must be one its own age check reads as
    // current, or every install would warn about itself.
    let scene = Scene::new();
    let install = scene
        .ntropy()
        .args(["skill", "--install", ".claude/skills"])
        .output()
        .expect("a run");
    assert!(install.status.success());

    for args in [vec!["skill"], vec!["--version"]] {
        let output = scene.ntropy().args(&args).output().expect("a run");

        assert!(output.status.success(), "{args:?}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("warning"),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn install_gives_no_warning() {
    let scene = Scene::new();
    scene.put_copy(&scene.project(), ".claude/skills", Some(OLD));

    let output = scene
        .ntropy()
        .args(["skill", "--install", ".claude/skills"])
        .output()
        .expect("a run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
}

#[test]
fn another_command_gives_no_warning() {
    let scene = Scene::new();
    scene.put_copy(&scene.project(), ".claude/skills", Some(OLD));
    let vault = scene.project().join("vault");
    fs::create_dir_all(vault.join("all-notes")).expect("a creatable directory");
    fs::create_dir_all(vault.join(".ntropy")).expect("a creatable directory");

    let output = scene
        .ntropy()
        .arg("--vault")
        .arg(&vault)
        .args(["-n", "tags"])
        .output()
        .expect("a run");

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
}

#[test]
fn version_warns_after_the_version() {
    let scene = Scene::new();
    scene.put_copy(&scene.project(), ".claude/skills", Some(OLD));

    let output = scene.ntropy().arg("--version").output().expect("a run");

    assert!(output.status.success());
    assert_eq!(
        stdout_of(&output),
        format!("ntropy {}\n", env!("CARGO_PKG_VERSION"))
    );
    insta::assert_snapshot!(scene.stderr(&output));
}

#[test]
fn the_short_version_flag_warns_too() {
    let scene = Scene::new();
    scene.put_copy(&scene.project(), ".claude/skills", Some(OLD));

    let long = scene.ntropy().arg("--version").output().expect("a run");
    let short = scene.ntropy().arg("-V").output().expect("a run");

    assert!(short.status.success());
    assert_eq!(short.stderr, long.stderr);
    assert!(!short.stderr.is_empty());
}

#[test]
fn version_in_a_removed_directory_prints_the_version_and_no_warning() {
    let scene = Scene::new();
    scene.put_copy(&scene.home(), ".claude/skills", Some(OLD));
    let gone = scene.root.path().join("no-such-dir-after-the-start");
    fs::create_dir(&gone).expect("a creatable directory");

    // The directory the process starts in is removed before ntropy runs, so
    // the current directory cannot be read.
    let mut shell = Command::new("sh");
    isolate(&mut shell);
    shell
        .arg("-c")
        .arg(r#"cd "$1" && rmdir "$1" && exec "$2" --version"#)
        .arg("sh")
        .arg(&gone)
        .arg(env!("CARGO_BIN_EXE_ntropy"))
        .env("HOME", scene.home());
    let output = shell.output().expect("a run");

    assert!(output.status.success());
    assert_eq!(
        stdout_of(&output),
        format!("ntropy {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(output.stderr, b"");
}

#[test]
fn a_copy_below_claude_config_dir_is_found() {
    let scene = Scene::new();
    let config = scene.root.path().join("claude-config");
    scene.put_copy(&config, "skills", Some(OLD));

    let output = scene
        .ntropy()
        .env("CLAUDE_CONFIG_DIR", &config)
        .arg("skill")
        .output()
        .expect("a run");

    insta::assert_snapshot!(scene.stderr(&output));
}

#[test]
fn a_relative_claude_config_dir_names_no_directory() {
    let scene = Scene::new();
    scene.put_copy(&scene.project(), "no-such-config-dir", Some(OLD));

    let output = scene
        .ntropy()
        .env("CLAUDE_CONFIG_DIR", "no-such-config-dir")
        .arg("skill")
        .output()
        .expect("a run");

    assert_eq!(output.stderr, b"");
}
