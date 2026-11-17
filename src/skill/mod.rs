// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The agent skill compiled into the binary (ADR 0092).
//!
//! `skills/ntropy/` holds the skill in the Agent Skills format: a core,
//! `SKILL.md`, and parts under `references/`. Each file reaches the binary
//! through [`include_str!`], so an agent that runs `ntropy skill` reads the
//! skill of the ntropy it runs, and [`install`] writes the same files into a
//! skills directory.
//!
//! The skill files carry no MPL header, the second exception in this
//! repository after the vault seed: they are copied verbatim into agents'
//! skills directories.

use std::io;
use std::path::Path;

pub mod age;

/// The skill's core: every rule an agent needs for every task.
pub const SKILL: &str = include_str!("../../skills/ntropy/SKILL.md");

/// The skill's parts by the name `ntropy skill <part>` takes. Each is
/// `references/<name>.md` beside `SKILL.md`, which names it at the step that
/// needs it.
pub const PARTS: &[(&str, &str)] = &[
    (
        "writing-notes",
        include_str!("../../skills/ntropy/references/writing-notes.md"),
    ),
    (
        "querying",
        include_str!("../../skills/ntropy/references/querying.md"),
    ),
    (
        "vaults",
        include_str!("../../skills/ntropy/references/vaults.md"),
    ),
    (
        "views",
        include_str!("../../skills/ntropy/references/views.md"),
    ),
    (
        "site",
        include_str!("../../skills/ntropy/references/site.md"),
    ),
    (
        "site-themes",
        include_str!("../../skills/ntropy/references/site-themes.md"),
    ),
    (
        "document-themes",
        include_str!("../../skills/ntropy/references/document-themes.md"),
    ),
];

/// The names `ntropy skill <part>` accepts, for the argument parser.
pub fn part_names() -> impl Iterator<Item = &'static str> {
    PARTS.iter().map(|(name, _)| *name)
}

/// The text of the part called `name`, none for a name that is no part.
pub fn part(name: &str) -> Option<&'static str> {
    PARTS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, text)| *text)
}

/// Writes every part and then the core into `dir`, handing each file to
/// `announce` once it is written. `dir` is the skill's own directory, such
/// as `.claude/skills/ntropy`.
///
/// ntropy owns `references/` in `dir`: it is replaced whole, so a part an
/// older ntropy wrote and this one no longer ships disappears. Any other
/// file in `dir` stays. The core carries the stamp that says which ntropy
/// the parts belong to, so it is written last: an install that stops half
/// way leaves the stamp of the copy it found, and the warning about that
/// copy stays. A failure names the path it happened at.
pub fn install(dir: &Path, mut announce: impl FnMut(&Path) -> io::Result<()>) -> io::Result<()> {
    // The message of an I/O error names no path, and the caller prints
    // only this error, so each failure carries the path it happened at.
    let at = |path: &Path| {
        let path = path.display().to_string();
        move |error: io::Error| io::Error::new(error.kind(), format!("{path}: {error}"))
    };
    std::fs::create_dir_all(dir).map_err(at(dir))?;

    let references = dir.join("references");
    match std::fs::remove_dir_all(&references) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(at(&references)(error));
        }
        _ => {}
    }
    std::fs::create_dir(&references).map_err(at(&references))?;
    for (name, text) in PARTS {
        let file = references.join(format!("{name}.md"));
        std::fs::write(&file, text).map_err(at(&file))?;
        announce(&file)?;
    }

    let core = dir.join("SKILL.md");
    std::fs::write(&core, SKILL).map_err(at(&core))?;
    announce(&core)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;

    /// The skill's directory in the repository, for tests that compare the
    /// binary's text with the files and for the guards on the directory.
    fn skill_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/ntropy")
    }

    /// The frontmatter of the core as a YAML value.
    fn core_frontmatter() -> serde_yaml_ng::Value {
        let text = SKILL
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
            .expect("the core opens with frontmatter")
            .0;
        serde_yaml_ng::from_str(text).expect("the frontmatter is YAML")
    }

    #[test]
    fn the_core_carries_the_version_of_this_ntropy() {
        // Every copy of the skill, installed or saved from `ntropy skill`,
        // names the ntropy it belongs to. A release that raises the crate
        // version fails here until the stamp follows.
        let frontmatter = core_frontmatter();
        let stamp = frontmatter["metadata"]["ntropy-version"].as_str();

        assert_eq!(stamp, Some(env!("CARGO_PKG_VERSION")), "{frontmatter:?}");
    }

    #[test]
    fn the_core_ends_with_its_end_line() {
        // An agent that read only part of the file can tell from the
        // missing end line that it has to read on.
        assert!(SKILL.ends_with("\nEnd of the ntropy skill.\n"));
    }

    #[test]
    fn every_part_ends_with_its_end_line() {
        for (name, text) in PARTS {
            assert!(
                text.ends_with(&format!("\nEnd of references/{name}.md.\n")),
                "{name}"
            );
        }
    }

    #[test]
    fn every_part_opens_with_the_line_that_sends_the_reader_to_the_core() {
        let opening = "Part of the ntropy skill. Read SKILL.md first, which holds the rules \
                       for every task; `ntropy skill` prints it.\n";
        for (name, text) in PARTS {
            assert!(text.starts_with(opening), "{name}: {:?}", &text[..60]);
        }
    }

    #[test]
    fn the_core_names_every_part_by_file_and_by_command() {
        // A host that loads the skill directory follows the file; an agent
        // that only ran `ntropy skill` has no file and needs the command.
        for (name, _) in PARTS {
            assert!(
                SKILL.contains(&format!("references/{name}.md")),
                "{name} by file"
            );
            assert!(
                SKILL.contains(&format!("`ntropy skill {name}`")),
                "{name} by command"
            );
        }
    }

    #[test]
    fn the_core_links_exactly_the_parts_there_are() {
        // A part name is a run of lowercase letters, digits and hyphens
        // between `references/` and `.md`; `site-themes` must not be read
        // as `site`.
        let linked: BTreeSet<&str> = SKILL
            .split("references/")
            .skip(1)
            .filter_map(|rest| {
                let name_end = rest
                    .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
                    .unwrap_or(rest.len());
                rest[name_end..]
                    .starts_with(".md")
                    .then(|| &rest[..name_end])
            })
            .collect();
        let shipped: BTreeSet<&str> = part_names().collect();

        assert!(!linked.is_empty());
        assert_eq!(linked, shipped);
    }

    #[test]
    fn every_link_to_a_part_is_followed_by_its_command() {
        // A reader of `ntropy skill` has no file to follow, so each link to
        // a part, in the core (`references/<part>.md`) and in a part
        // (`<part>.md`), also names the command that prints it, on the same
        // line or the next.
        let mut checked = 0;
        for (file, text) in std::iter::once(("SKILL.md", SKILL)).chain(PARTS.iter().copied()) {
            let prefix = if file == "SKILL.md" {
                "references/"
            } else {
                ""
            };
            for (name, _) in PARTS {
                let link = format!("]({prefix}{name}.md)");
                for (at, _) in text.match_indices(&link) {
                    let rest = &text[at + link.len()..];
                    let two_lines: String = rest.lines().take(2).collect::<Vec<_>>().join("\n");
                    assert!(
                        two_lines.contains(&format!("`ntropy skill {name}`")),
                        "{file}: the link to {name} lacks its command: {two_lines:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked >= 15, "found only {checked} links to parts");
    }

    #[test]
    fn the_repository_holds_no_part_the_binary_does_not_ship() {
        let on_disk: BTreeSet<String> = fs::read_dir(skill_dir().join("references"))
            .expect("the skill has a references directory")
            .map(|entry| {
                entry
                    .expect("a readable entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        let shipped: BTreeSet<String> = part_names().map(|name| format!("{name}.md")).collect();

        assert_eq!(on_disk, shipped);
    }

    #[test]
    fn the_binary_ships_the_files_of_the_repository() {
        assert_eq!(
            SKILL,
            fs::read_to_string(skill_dir().join("SKILL.md")).expect("the core exists")
        );
        for (name, text) in PARTS {
            let file = skill_dir().join(format!("references/{name}.md"));
            assert_eq!(*text, fs::read_to_string(file).expect("the part exists"));
        }
    }

    /// Guards the header exemption: these files are copied verbatim into
    /// agents' skills directories, so a license comment added by reflex
    /// would ship with every installed skill.
    #[test]
    fn the_skill_files_carry_no_license_header() {
        for text in std::iter::once(SKILL).chain(PARTS.iter().map(|(_, text)| *text)) {
            assert!(!text.contains("Mozilla Public"), "got: {text}");
        }
    }

    #[test]
    fn a_part_is_found_by_its_name() {
        for (name, text) in PARTS {
            assert_eq!(part(name), Some(*text));
        }
    }

    #[test]
    fn a_name_that_is_no_part_finds_nothing() {
        assert_eq!(part("no-such-part"), None);
    }

    #[test]
    fn the_part_names_are_those_of_the_table() {
        let names: Vec<&str> = part_names().collect();

        assert_eq!(names.len(), PARTS.len());
        assert!(names.contains(&"site-themes"));
    }

    // =========================================================
    // Installing
    // =========================================================

    fn temp() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temp dir")
    }

    #[test]
    fn install_writes_the_core_and_every_part() {
        let root = temp();
        let dir = root.path().join("ntropy");

        install(&dir, |_| Ok(())).expect("an installable skill");

        assert_eq!(fs::read_to_string(dir.join("SKILL.md")).unwrap(), SKILL);
        for (name, text) in PARTS {
            let file = dir.join(format!("references/{name}.md"));
            assert_eq!(fs::read_to_string(file).unwrap(), *text, "{name}");
        }
    }

    #[test]
    fn install_announces_each_file_once_it_is_written() {
        let root = temp();
        let dir = root.path().join("ntropy");
        let mut announced = Vec::new();

        install(&dir, |file| {
            assert!(
                file.is_file(),
                "{} is announced before it exists",
                file.display()
            );
            announced.push(file.to_owned());
            Ok(())
        })
        .expect("an installable skill");

        let mut expected: Vec<_> = PARTS
            .iter()
            .map(|(name, _)| dir.join(format!("references/{name}.md")))
            .collect();
        expected.push(dir.join("SKILL.md"));
        assert_eq!(announced, expected);
    }

    #[test]
    fn install_replaces_the_references_and_keeps_other_files() {
        let root = temp();
        let dir = root.path().join("ntropy");
        fs::create_dir_all(dir.join("references")).unwrap();
        fs::write(dir.join("references/retired.md"), "old").unwrap();
        fs::write(dir.join("notes.txt"), "mine").unwrap();

        install(&dir, |_| Ok(())).expect("an installable skill");

        assert!(!dir.join("references/retired.md").exists());
        assert_eq!(fs::read_to_string(dir.join("notes.txt")).unwrap(), "mine");
    }

    #[test]
    fn install_into_a_file_fails_and_names_the_path() {
        let root = temp();
        let dir = root.path().join("ntropy");
        fs::write(&dir, "in the way").unwrap();

        let error = install(&dir, |_| Ok(())).expect_err("a file is no directory");

        assert!(
            error.to_string().contains(&dir.display().to_string()),
            "{error}"
        );
    }

    #[test]
    fn install_fails_and_names_references_when_it_is_a_file() {
        let root = temp();
        let dir = root.path().join("ntropy");
        fs::create_dir_all(&dir).unwrap();
        let references = dir.join("references");
        fs::write(&references, "in the way").unwrap();

        let error = install(&dir, |_| Ok(())).expect_err("a file is no directory to replace");

        assert!(
            error
                .to_string()
                .starts_with(&references.display().to_string()),
            "{error}"
        );
    }

    #[test]
    fn install_stops_when_the_announcement_fails() {
        let root = temp();
        let dir = root.path().join("ntropy");

        let error = install(&dir, |_| Err(io::Error::other("closed stdout")))
            .expect_err("the announcement failed");

        assert_eq!(error.to_string(), "closed stdout");
        assert!(!dir.join("SKILL.md").exists());
    }

    #[test]
    fn a_failed_install_keeps_the_stamp_of_the_copy_it_found() {
        // The stamp says which ntropy the parts beside it belong to, so it
        // is written last: an install that stops half way must not leave a
        // current stamp over parts of an older ntropy.
        let root = temp();
        let dir = root.path().join("ntropy");
        fs::create_dir_all(&dir).unwrap();
        let old = "---\nname: ntropy\nmetadata:\n  ntropy-version: \"1.0.0\"\n---\n";
        fs::write(dir.join("SKILL.md"), old).unwrap();
        fs::write(dir.join("references"), "in the way").unwrap();

        install(&dir, |_| Ok(())).expect_err("a file is no directory to replace");

        assert_eq!(fs::read_to_string(dir.join("SKILL.md")).unwrap(), old);
    }
}
