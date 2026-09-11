// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The `site` command: the whole vault as a static website
//! (`docs/design/site-export.md`, ADR 0046).
//!
//! The library builds every page in memory; this module decides where they
//! land, does the one destructive step (emptying a forced output directory),
//! writes the files, and reports.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

use ntropy::ops;
use ntropy::session::VaultSession;
use ntropy::site;
use ntropy::view::ViewDef;

use super::render::{absolutize, lands_inside_vault};
use super::{exit_for_warnings, output, plural};
use crate::cli::{GlobalArgs, SiteThemeCommand};

pub fn cmd_site(
    global: &GlobalArgs,
    session: &VaultSession,
    output_dir: PathBuf,
    query: String,
    theme: Option<String>,
    force: bool,
    print: bool,
) -> Result<ExitCode> {
    // The config and the theme load before any scan, so a bad theme name
    // costs nothing (the rule `render` follows, ADR 0045).
    let config = ntropy::config::PerVaultConfig::load(&session.layout().config_file())
        .context("while loading the vault config")?;
    let selected = ntropy::render::theme::select(theme.as_deref(), config.site.theme.as_deref());
    let loaded_theme = selected
        .map(|name| ntropy::site::theme::load(session.layout(), name))
        .transpose()
        .context("while loading the site theme")?;
    let theme_dir = selected.map(|name| session.layout().site_theme_dir(name));

    // The output directory is prepared before the scan too: refusing a
    // non-empty directory should not cost a scan of the whole vault.
    let cwd = std::env::current_dir().context("while resolving the current directory")?;
    let absolute_output = absolutize(&output_dir, &cwd);
    prepare_output_dir(&absolute_output, force)?;
    if lands_inside_vault(session.root(), &absolute_output) && session.is_encrypted() {
        eprintln!(
            "warning: `{}` is inside an encrypted vault; the site is not \
             encrypted and will sync as plaintext",
            output_dir.display()
        );
    }

    // One scan yields every note; the query selects the exported subset, and
    // the ids of the rest let the build tell a link to an excluded note from
    // a dangling one.
    let matches = ops::search(session, None).context("while scanning the vault")?;
    output::print_warnings(&matches.warnings);
    let vault_ids: HashSet<_> = matches.notes.iter().map(|note| note.id).collect();
    // The parsed query also reaches the build: a single `tag:` predicate
    // roots the sidebar at that tag (ADR 0056).
    let parsed = match super::optional(&query) {
        Some(query) => Some(ntropy::query::parse(query).context("while parsing the query")?),
        None => None,
    };
    let notes: Vec<_> = match super::optional(&query) {
        Some(query) => {
            let prepared = ntropy::query::compile(query).context("while compiling the query")?;
            matches
                .notes
                .into_iter()
                .filter(|note| prepared.matches(note))
                .collect()
        }
        None => matches.notes,
    };

    let views: Vec<ViewDef> = config
        .views
        .iter()
        .map(|view| ViewDef::new(&view.name, &view.field))
        .collect();
    let fallback_title = session
        .root()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ntropy".to_string());

    let built = site::build(&site::Input {
        notes: &notes,
        vault_ids: &vault_ids,
        views: &views,
        options: &config.site,
        query: parsed.as_ref(),
        fallback_title: &fallback_title,
        vault_root: session.root(),
        theme: loaded_theme.as_ref().zip(theme_dir.as_deref()),
    })
    .context("while building the site")?;
    site::write(&built, &absolute_output).context("while writing the site")?;

    for warning in &built.warnings {
        eprintln!("warning: {warning}");
    }

    // `--print` writes exactly the entry page's path to stdout for command
    // substitution (ADR 0036); otherwise a completion report names the
    // directory, the page count, and the warnings.
    if print {
        println!("{}", output_dir.join(site::build::INDEX_PAGE).display());
    } else {
        println!(
            "Exported {} to {} ({}, {})",
            plural(built.page_count(), "page", "pages"),
            output_dir.display(),
            plural(notes.len(), "note", "notes"),
            plural(built.warnings.len(), "warning", "warnings")
        );
    }

    Ok(exit_for_warnings(
        global.strict,
        &matches.warnings,
        built.warnings.len(),
    ))
}

/// Make `dir` an empty, existing directory. A non-empty one is refused
/// unless `force`, in which case its contents are removed first; a path
/// that is not a directory is refused outright.
fn prepare_output_dir(dir: &Path, force: bool) -> Result<()> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("while creating {}", dir.display()))?;
        return Ok(());
    }
    if !dir.is_dir() {
        bail!("`{}` exists and is not a directory", dir.display());
    }
    let entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("while reading {}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<_>>()
        .with_context(|| format!("while reading {}", dir.display()))?;
    if entries.is_empty() {
        return Ok(());
    }
    if !force {
        bail!(
            "`{}` is not empty; pass --force to empty it before writing",
            dir.display()
        );
    }
    for entry in entries {
        if entry.is_dir() {
            std::fs::remove_dir_all(&entry)
        } else {
            std::fs::remove_file(&entry)
        }
        .with_context(|| format!("while removing {}", entry.display()))?;
    }
    Ok(())
}

pub fn cmd_site_theme(session: &VaultSession, sub: SiteThemeCommand) -> Result<ExitCode> {
    match sub {
        SiteThemeCommand::Init { name } => {
            ntropy::render::theme::validate_name(&name).context("while checking the theme name")?;
            let dir = session.layout().site_theme_dir(&name);
            site::theme::write_builtin(&dir)
                .with_context(|| format!("while writing the theme to {}", dir.display()))?;
            println!(
                "Wrote the built-in site theme to {}; select it with `[site] theme = \"{name}\"`",
                dir.display()
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}
