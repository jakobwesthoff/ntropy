// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `skill` command: the agent skill compiled into the binary, printed
//! or installed into a skills directory (ADR 0092). It needs no vault.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};

use ntropy::skill;

use super::output;

pub fn cmd_skill(part: Option<&str>, install: Option<&Path>) -> Result<ExitCode> {
    match install {
        Some(root) => cmd_install(root),
        None => cmd_print(part),
    }
}

/// Prints the core, or the part `name`, as the Markdown it is: an agent reads
/// it as it stands, so nothing is wrapped, coloured or added, also on a
/// terminal.
fn cmd_print(part: Option<&str>) -> Result<ExitCode> {
    let text = match part {
        None => skill::SKILL,
        Some(name) => {
            skill::part(name).expect("the argument parser accepts only the names of parts")
        }
    };
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
        .context("while printing the skill")?;
    // An agent that reads the skill this way may have an older copy
    // installed, which its runner loads in later sessions in place of this
    // one.
    warn_of_outdated_skill();
    Ok(ExitCode::SUCCESS)
}

/// Warns on stderr about every installed copy of the skill that is older
/// than this ntropy (ADR 0093), searched from the current directory. `ntropy
/// skill` and `ntropy --version` are the two callers: the version is read
/// before the command line, so nothing but the current directory can say
/// where to start. A current directory that cannot be read leaves nowhere to
/// start, and the check is skipped.
pub fn warn_of_outdated_skill() {
    let Ok(start) = std::env::current_dir() else {
        return;
    };
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from);
    if let Some(outdated) = skill::age::outdated_copies(&start, home.as_deref()) {
        output::warn(&format!("{}. {}", outdated.message, outdated.fix));
    }
}

/// Writes the skill into `root/ntropy`, where `root` is a skills directory
/// such as `~/.claude/skills`: each skill lives in a directory named after
/// it. Each file is named once it is written, so a failure half way leaves a
/// pipe naming what was written.
fn cmd_install(root: &Path) -> Result<ExitCode> {
    let dir = root.join("ntropy");
    let mut stdout = std::io::stdout().lock();
    skill::install(&dir, |file| {
        writeln!(stdout, "installed {}", output::shown_path(file))
    })
    .context("while installing the skill")?;
    Ok(ExitCode::SUCCESS)
}
