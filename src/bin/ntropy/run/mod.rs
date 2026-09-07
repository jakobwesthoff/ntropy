// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Dispatch: parsed CLI to library use cases (`docs/design/cli.md`).
//!
//! This layer owns everything the headless library deliberately does not: vault
//! resolution from flags/env/config, the interactive-vs-plain choice, the
//! picker and editor, confirmation prompts, and translating outcomes into exit
//! codes. Each command resolves to one or more `ops::` calls plus presentation.

mod edit;
mod editor;
mod interact;
#[cfg(feature = "encryption")]
mod keys;
mod lsp;
mod output;
mod picker;
#[cfg(feature = "encryption")]
mod prompt;
mod render;
mod securetemp;
#[cfg(feature = "encryption")]
mod vault_cmd;

use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::CommandFactory;

use ntropy::config::global;
use ntropy::ops;
use ntropy::reconcile;
use ntropy::scan::ScanWarning;
use ntropy::session::VaultSession;
use ntropy::vault::{ResolveOptions, Vault, layout, resolve};

use crate::cli::{Cli, Command, GlobalArgs, VaultCommand, ViewCommand, join};

/// Run the parsed CLI to completion, returning the process exit code.
pub fn run(cli: Cli) -> Result<ExitCode> {
    let Some(command) = cli.command else {
        // Bare `ntropy` prints help (ADR 0018).
        Cli::command().print_help()?;
        println!();
        return Ok(ExitCode::SUCCESS);
    };

    // `init` is the one command that does not operate on an already-resolved
    // vault, so it is handled before resolution.
    if let Command::Init {
        path,
        set_default,
        encrypted,
    } = command
    {
        return cmd_init(&cli.global, path, set_default, encrypted);
    }

    // `info` resolves the vault itself so it can report which rule matched.
    if let Command::Info { print } = command {
        return cmd_info(&cli.global, print);
    }

    // The language server resolves a vault per document, so it starts without
    // one (ADR 0029).
    if let Command::Lsp = command {
        return lsp::run();
    }

    // The key and conversion commands resolve the vault themselves. A
    // conversion in particular must not go through the session build below,
    // whose marker guard exists to stop every *other* command from touching a
    // half-converted vault — including, if it ran here, the `--resume` that
    // finishes one.
    match command {
        Command::Unlock => return cmd_unlock(&cli.global),
        Command::Lock => return cmd_lock(&cli.global),
        Command::Vault { command } => return cmd_vault(&cli.global, command),
        _ => {}
    }

    let session = open_session(&cli.global)?;
    let interactive = interact::is_interactive(cli.global.non_interactive);

    match command {
        // Handled above, before vault resolution.
        Command::Init { .. } => unreachable!("init is dispatched before vault resolution"),
        Command::Search {
            query,
            print,
            print_content,
        } => cmd_search(
            &cli.global,
            &session,
            join(&query),
            print,
            print_content,
            interactive,
        ),
        Command::New {
            title,
            template,
            empty,
            print,
        } => cmd_new(&session, join(&title), template, empty, print, interactive),
        Command::Today { print } => cmd_today(&session, print, interactive),
        Command::Write { target } => cmd_write(&session, &target),
        Command::Reconcile => cmd_reconcile(&cli.global, &session),
        Command::Delete { selector, force } => {
            cmd_delete(&session, join(&selector), force, interactive)
        }
        Command::Render {
            selector,
            to,
            engine,
            output,
            theme,
            print,
        } => render::cmd_render(
            &cli.global,
            &session,
            join(&selector),
            to,
            engine,
            output,
            theme,
            print,
            interactive,
        ),
        Command::View(sub) => cmd_view(&session, sub),
        // Handled above, before vault resolution.
        Command::Unlock | Command::Lock | Command::Vault { .. } => {
            unreachable!("key and conversion commands dispatch before vault resolution")
        }
        Command::Tags => cmd_tags(&cli.global, &session),
        // Handled above, before vault resolution.
        Command::Info { .. } => unreachable!("info is dispatched before vault resolution"),
        Command::Lsp => unreachable!("lsp is dispatched before vault resolution"),
    }
}

/// How many of the most-used tags `info` reports.
const TOP_TAGS: usize = 5;

// =============================================================================
// Vault resolution
// =============================================================================

fn resolve_options(global: &GlobalArgs) -> Result<ResolveOptions> {
    let global_default = global::load()
        .context("while loading the global config")?
        .default_vault;
    Ok(ResolveOptions {
        explicit: global.vault.clone(),
        env: std::env::var_os("NTROPY_VAULT").map(PathBuf::from),
        start_dir: std::env::current_dir().ok(),
        global_default,
    })
}

pub(super) fn resolve_vault(global: &GlobalArgs) -> Result<Vault> {
    let opts = resolve_options(global)?;
    Vault::resolve(&opts).context("while resolving the vault")
}

/// Resolve the vault and open a session for reading and writing its notes.
fn open_session(global: &GlobalArgs) -> Result<VaultSession> {
    let vault = resolve_vault(global)?;
    session_for(global, vault)
}

/// Open a session over an already-resolved vault.
#[cfg(feature = "encryption")]
fn session_for(global: &GlobalArgs, vault: Vault) -> Result<VaultSession> {
    // A vault mid-conversion holds two copies of every note, so scanning or
    // syncing it would produce nonsense and editing it could lose work. Every
    // ordinary command reaches this, which is why the guard lives here rather
    // than in each of them; the conversions build their sessions directly and
    // deliberately bypass it.
    if let Some(marker) = ntropy::migrate::marker::read_at(&vault.layout().migration_file())
        .context("while checking for an interrupted conversion")?
    {
        bail!(
            "this vault has an interrupted `{}`; finish it with `{}`",
            marker.operation,
            marker.operation.resume_command()
        );
    }
    keys::KeyContext::from_args(global).open(vault)
}

/// Open a session with no way to read an encrypted vault.
///
/// The session still recognizes which files are notes, so an encrypted vault
/// reports the missing support once instead of warning about every file.
#[cfg(not(feature = "encryption"))]
fn session_for(_global: &GlobalArgs, vault: Vault) -> Result<VaultSession> {
    if layout::is_encrypted(vault.root()) {
        return Ok(VaultSession::unsupported(vault));
    }
    Ok(VaultSession::plaintext(vault))
}

/// The error a build compiled without encryption support reports.
#[cfg(not(feature = "encryption"))]
fn encryption_unsupported() -> anyhow::Error {
    anyhow!(
        "this build of ntropy was compiled without encryption support \
         (rebuild with `--features encryption`)"
    )
}

// =============================================================================
// Commands
// =============================================================================

fn cmd_init(
    global: &GlobalArgs,
    path: Option<PathBuf>,
    set_default: bool,
    encrypted: bool,
) -> Result<ExitCode> {
    // The positional path and the global `--vault` are two ways to name the same
    // target, so requiring exactly one keeps the destination unambiguous. With
    // neither, `init` scaffolds the current directory.
    let target = match (path, global.vault.clone()) {
        (Some(_), Some(_)) => {
            bail!("pass the target as either `--vault` or the positional path, not both")
        }
        (Some(p), None) => p,
        (None, Some(v)) => v,
        (None, None) => std::env::current_dir().context("while reading the current directory")?,
    };
    let options = init_options(global, encrypted, &target)?;
    let report = ops::init_vault_with(&target, &options).context("while initializing the vault")?;

    let touched_gitignore =
        !report.gitignore_added.is_empty() || !report.gitignore_removed.is_empty();
    if report.created.is_empty() && !touched_gitignore {
        println!("Vault already initialized at {}", report.root.display());
    } else {
        println!("Initialized vault at {}", report.root.display());
    }
    report_gitignore_changes(&report.gitignore_added, &report.gitignore_removed);
    if let Some(recipient) = &report.recipient {
        println!("Encryption:    on");
        println!("Recipient:     {recipient}");
    }

    if set_default {
        let canonical = std::fs::canonicalize(&report.root).unwrap_or_else(|_| report.root.clone());
        set_global_default(&canonical)?;
        println!("Set as default vault.");
    }
    Ok(ExitCode::SUCCESS)
}

// =============================================================================
// Encryption commands
// =============================================================================
//
// These are compiled into both feature configurations so `--help` is identical
// whichever way ntropy was built, and a stripped binary answers with a clear
// message rather than "unrecognized subcommand" (ADR 0041).

/// Build the init options, obtaining a passphrase when one is called for.
#[cfg(feature = "encryption")]
fn init_options(global: &GlobalArgs, encrypted: bool, target: &Path) -> Result<ops::InitOptions> {
    if !encrypted {
        return Ok(ops::InitOptions::default());
    }
    // Re-running `init --encrypted` on a vault that already has a keypair must
    // not ask for a passphrase it would then discard: the existing key is kept.
    if layout::is_encrypted(target) {
        return Ok(ops::InitOptions::default());
    }
    let label = target.display().to_string();
    Ok(ops::InitOptions {
        encrypt_with: Some(keys::KeyContext::from_args(global).new_passphrase(&label)?),
    })
}

#[cfg(not(feature = "encryption"))]
fn init_options(_global: &GlobalArgs, encrypted: bool, _target: &Path) -> Result<ops::InitOptions> {
    if encrypted {
        return Err(encryption_unsupported());
    }
    Ok(ops::InitOptions::default())
}

#[cfg(feature = "encryption")]
fn cmd_unlock(global: &GlobalArgs) -> Result<ExitCode> {
    let vault = resolve_vault(global)?;
    let recipient = keys::KeyContext::from_args(global).unlock(&vault)?;
    println!("Unlocked {} ({recipient}).", vault.root().display());
    Ok(ExitCode::SUCCESS)
}

#[cfg(feature = "encryption")]
fn cmd_lock(global: &GlobalArgs) -> Result<ExitCode> {
    let vault = resolve_vault(global)?;
    let (recipient, removed) = keys::KeyContext::from_args(global).lock(&vault)?;
    if removed {
        println!("Locked {} ({recipient}).", vault.root().display());
    } else {
        println!("{} was already locked.", vault.root().display());
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(feature = "encryption"))]
fn cmd_unlock(_global: &GlobalArgs) -> Result<ExitCode> {
    Err(encryption_unsupported())
}

#[cfg(not(feature = "encryption"))]
fn cmd_lock(_global: &GlobalArgs) -> Result<ExitCode> {
    Err(encryption_unsupported())
}

#[cfg(feature = "encryption")]
fn cmd_vault(global: &GlobalArgs, command: VaultCommand) -> Result<ExitCode> {
    let interactive = interact::is_interactive(global.non_interactive);
    vault_cmd::run(global, command, interactive)?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(feature = "encryption"))]
fn cmd_vault(_global: &GlobalArgs, command: VaultCommand) -> Result<ExitCode> {
    // Every variant is named so the flags count as read and the stripped build
    // stays warning-free.
    match command {
        VaultCommand::Encrypt { resume, yes } | VaultCommand::Decrypt { resume, yes } => {
            let _ = (resume, yes);
        }
        VaultCommand::Rekey {
            resume,
            yes,
            new_passphrase_file,
        } => {
            let _ = (resume, yes, new_passphrase_file);
        }
        VaultCommand::Passphrase {
            new_passphrase_file,
        } => {
            let _ = new_passphrase_file;
        }
    }
    Err(encryption_unsupported())
}

fn cmd_search(
    global: &GlobalArgs,
    session: &VaultSession,
    selector: String,
    print: bool,
    print_content: bool,
    interactive: bool,
) -> Result<ExitCode> {
    // A bare invocation browses the whole vault; a selector resolves a full ULID
    // directly or otherwise runs as a DSL query (the id-or-query rule of ADR
    // 0025). Both feed one presentation path, which `edit` shares verbatim as a
    // hidden alias (ADR 0031).
    let matches = match optional(&selector) {
        Some(selector) => {
            ops::resolve_selection(session, selector).context("while resolving the selector")?
        }
        None => ops::search(session, None).context("while listing notes")?,
    };
    output::print_warnings(&matches.warnings);

    match matches.notes.as_slice() {
        // A no-match, including an empty-vault listing, is a non-zero exit so
        // `search <x> && ...` branches correctly. The message goes to stderr to
        // keep stdout clean for pipelines.
        [] => {
            eprintln!("No notes matched your search criteria.");
            Ok(ExitCode::FAILURE)
        }
        // `--print-content` emits the note itself rather than a path, so it
        // needs exactly one note: several notes concatenated with nothing
        // between them is not something a caller can take apart again. That is
        // `render`'s rule (ADR 0025), for the same reason.
        [note] if print_content => {
            print!(
                "{}",
                session
                    .cipher()
                    .read(&note.path)
                    .context("while reading the note")?
            );
            Ok(ExitCode::SUCCESS)
        }
        notes if print_content && !interactive => {
            report_ambiguous(&selector, notes)?;
            Ok(ExitCode::FAILURE)
        }
        notes => {
            if interactive {
                // On a TTY a lone match is selected straight away; several open
                // the picker pre-filtered to them.
                let selected = if let [note] = notes {
                    Some(note.path.clone())
                } else {
                    let candidates = ops::to_candidates(notes)?;
                    picker::pick(candidates, picker::align_candidates)?.map(|s| s.path)
                };
                match selected {
                    // `--print-content` writes the note itself, decrypting
                    // where the vault requires it, so the output is the same
                    // whichever way the vault stores its notes.
                    Some(path) if print_content => {
                        print!(
                            "{}",
                            session
                                .cipher()
                                .read(&path)
                                .context("while reading the note")?
                        );
                    }
                    // `--print` writes the selection's path to stdout instead
                    // of opening the editor (ADR 0035). Nothing was edited, so
                    // no realign or view refresh is needed.
                    Some(path) if print => println!("{}", path.display()),
                    Some(path) => open_and_refresh(session, &path)?,
                    // A cancelled picker produced no path, so under `--print`
                    // the command fails and `p=$(ntropy search -p ...)`
                    // branches correctly; without `--print` a cancel stays a
                    // successful no-op.
                    None if print || print_content => return Ok(ExitCode::FAILURE),
                    None => {}
                }
            } else if print {
                // With no picker to choose one note, `--print` covers every
                // match: one path per line, in the table's newest-first order,
                // ready for `xargs` and friends.
                for note in notes {
                    println!("{}", note.path.display());
                }
            } else {
                // Without a TTY the editor never opens, mirroring `new`/`today`
                // (ADR 0015); the plain table is printed instead.
                output::print_notes(notes)?;
            }
            Ok(exit_for_warnings(global.strict, &matches.warnings, 0))
        }
    }
}

fn cmd_new(
    session: &VaultSession,
    title: String,
    template: Option<String>,
    empty: bool,
    print: bool,
    interactive: bool,
) -> Result<ExitCode> {
    // The two creation modes differ only in what lands in the file. Both know
    // their content without reading it back, which is what keeps the tail below
    // shared and an encrypted vault from having to decrypt a file written
    // moments earlier.
    let (path, initial) = if empty {
        let path =
            ops::create_empty_note(session, &title).context("while creating the empty note")?;
        (path, String::new())
    } else {
        let note = ops::create_note(session, &title, template.as_deref())
            .context("while creating the note")?;
        let initial = format!("{}{}", note.raw_header, note.body);
        (note.path, initial)
    };

    // Open the editor only when interactive and not explicitly suppressed;
    // otherwise create-and-print for scripting (ADR 0015).
    if !print && interactive {
        open_and_refresh_from(session, &path, Some(&initial))?;
    } else {
        reconcile::refresh_views(session).context("while refreshing views")?;
        println!("{}", path.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_write(session: &VaultSession, target: &str) -> Result<ExitCode> {
    // stdin is the payload, which is why this command has no picker and no
    // prompt: an invocation that carries a note on stdin is a scripted one, and
    // dropping a fullscreen picker into the middle of a pipeline would be worse
    // than failing on an ambiguous target (ADR 0043).
    let mut content = String::new();
    std::io::stdin()
        .read_to_string(&mut content)
        .context("while reading the note from stdin")?;

    let note = ops::write_note(session, target, &content).context("while writing the note")?;

    // The same post-processing the editor round trip does on exit, so a title
    // the caller changed cannot leave the filename and the views behind.
    let path = match reconcile::realign(session, &note.path)
        .context("while realigning the written note")?
    {
        Some(rename) => rename.to,
        None => note.path,
    };
    reconcile::refresh_views(session).context("while refreshing views")?;

    println!("{}", path.display());
    Ok(ExitCode::SUCCESS)
}

fn cmd_today(session: &VaultSession, print: bool, interactive: bool) -> Result<ExitCode> {
    let outcome = ops::today_note(session).context("while preparing today's note")?;

    // Mirror `new`: open interactively unless suppressed, otherwise refresh views
    // and print the path for scripting (ADR 0015).
    if !print && interactive {
        // A freshly created note is handed its own content; an existing one is
        // read through the vault's cipher like any other edit.
        let initial = outcome
            .created
            .then(|| format!("{}{}", outcome.note.raw_header, outcome.note.body));
        open_and_refresh_from(session, &outcome.note.path, initial.as_deref())?;
    } else {
        reconcile::refresh_views(session).context("while refreshing views")?;
        println!("{}", outcome.note.path.display());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_reconcile(global: &GlobalArgs, session: &VaultSession) -> Result<ExitCode> {
    println!("Reconciling vault at {}...", session.root().display());
    let report = reconcile::reconcile(session).context("while reconciling")?;
    output::print_warnings(&report.warnings);
    for rename in &report.renamed {
        println!(
            "renamed {} -> {}",
            file_name(&rename.from),
            file_name(&rename.to)
        );
    }
    for rewrite in &report.links_rewritten {
        println!(
            "relinked {} -> {} in {}",
            rewrite.from,
            rewrite.to,
            file_name(&rewrite.note)
        );
    }
    for adopted in &report.adopted {
        println!(
            "encrypted {} -> {}",
            file_name(&adopted.from),
            file_name(&adopted.to)
        );
    }
    report_gitignore_changes(&report.gitignore_added, &report.gitignore_removed);
    if session.is_encrypted() && !session.is_unlocked() {
        // Only adoption ran, so saying "scanned 0 notes" would misdescribe it.
        println!(
            "Encrypted {} into the vault. Link rewriting and view syncing need \
             `ntropy unlock`.",
            plural(report.adopted.len(), "note", "notes"),
        );
        return Ok(ExitCode::SUCCESS);
    }
    // A summary always prints, so even a no-op run confirms what happened.
    println!(
        "Scanned {}, renamed {}, relinked {}, synced {}, ignored {}, unignored {}, {}.",
        plural(report.notes_scanned, "note", "notes"),
        plural(report.renamed.len(), "file", "files"),
        plural(report.links_rewritten.len(), "link", "links"),
        plural(report.views_synced, "view", "views"),
        plural(report.gitignore_added.len(), "entry", "entries"),
        plural(report.gitignore_removed.len(), "entry", "entries"),
        plural(report.warnings.len(), "warning", "warnings"),
    );
    Ok(exit_for_warnings(global.strict, &report.warnings, 0))
}

fn cmd_delete(
    session: &VaultSession,
    selector: String,
    force: bool,
    interactive: bool,
) -> Result<ExitCode> {
    let matches =
        ops::resolve_selection(session, &selector).context("while resolving the selector")?;
    output::print_warnings(&matches.warnings);

    // Determine the single target note (path + human reference), honoring the
    // ambiguity rule shared with `edit` (ADR 0025).
    let target = match matches.notes.as_slice() {
        [] => {
            eprintln!("error: no note matches `{selector}`");
            return Ok(ExitCode::FAILURE);
        }
        [note] => (note.path.clone(), output::note_reference(note)?),
        notes => {
            if interactive {
                let candidates = ops::to_candidates(notes)?;
                match picker::pick(candidates, picker::align_candidates)? {
                    Some(selected) => {
                        let reference = output::reference(
                            selected.id,
                            &selected.date,
                            &selected.title,
                            &selected.tags,
                        );
                        (selected.path, reference)
                    }
                    None => return Ok(ExitCode::SUCCESS),
                }
            } else {
                report_ambiguous(&selector, notes)?;
                return Ok(ExitCode::FAILURE);
            }
        }
    };
    let (path, reference) = target;

    if !force {
        if !interactive {
            bail!("refusing to delete {reference} without --force in non-interactive mode");
        }
        if !confirm(&format!("Delete {reference}? [y/N] "))? {
            println!("Aborted.");
            return Ok(ExitCode::SUCCESS);
        }
    }

    // The resolution scan above already surfaced any warnings; the view sync
    // scans the same vault, so its warnings are discarded to avoid printing
    // each one twice.
    ops::delete_note(session, &path).context("while deleting the note")?;
    println!("Deleted {reference}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_view(session: &VaultSession, sub: ViewCommand) -> Result<ExitCode> {
    match sub {
        ViewCommand::List => {
            let views = ops::list_views(session).context("while listing views")?;
            output::print_views(&views)?;
        }
        ViewCommand::Add { name, field } => {
            ops::add_view(session, &name, &field).context("while adding the view")?;
            println!("Added view `{name}` (field `{field}`).");
        }
        ViewCommand::Remove { name } => {
            ops::remove_view(session, &name).context("while removing the view")?;
            println!("Removed view `{name}`.");
            // ntropy never deletes the directory; tell the user it remains so
            // they can clean up the now-stale (and no longer ignored) tree.
            if session.layout().view_dir(&name).exists() {
                println!(
                    "left directory `{name}/` in place, delete it manually if you no longer need it"
                );
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_info(global: &GlobalArgs, print: bool) -> Result<ExitCode> {
    let opts = resolve_options(global)?;
    let (root, source) =
        resolve::resolve_with_source(&opts).context("while resolving the vault")?;

    // `--print` exists for shells to consume, so it emits the path alone and
    // skips the vault scan the report would otherwise run. Resolution
    // canonicalizes the root, so the path is absolute whichever rule matched.
    if print {
        println!("{}", root.display());
        return Ok(ExitCode::SUCCESS);
    }

    // `info` resolves the vault itself (to report which rule matched), so it
    // opens its own session rather than taking the one dispatch built.
    let session = session_for(global, Vault::new(root))?;
    let stats = ops::vault_stats(&session, TOP_TAGS).context("while gathering vault info")?;
    output::print_info(&session, &source, opts.global_default.as_deref(), &stats);
    Ok(ExitCode::SUCCESS)
}

fn cmd_tags(global: &GlobalArgs, session: &VaultSession) -> Result<ExitCode> {
    let list = ops::list_tags(session).context("while listing tags")?;
    output::print_warnings(&list.warnings);
    output::print_tags(&list.tags)?;
    Ok(exit_for_warnings(global.strict, &list.warnings, 0))
}

// =============================================================================
// Shared helpers
// =============================================================================

/// Open a note in the editor, then realign its filename and sync views.
///
/// Only the touched note is realigned, so an out-of-band drift elsewhere is
/// never renamed silently (ADR 0004); the view sync then reflects any title
/// or tag change made during the edit.
fn open_and_refresh(session: &VaultSession, path: &Path) -> Result<()> {
    open_and_refresh_from(session, path, None)
}

/// Open a note in the editor, then realign its filename and sync views.
///
/// `initial` is the content the caller already holds, letting `new` and `today`
/// skip decrypting a note they just wrote.
///
/// A failed edit still runs the post-processing when the note was written: the
/// error is reported afterwards, so a `:cq` that saved first behaves as it does
/// in a plaintext vault (ADR 0041).
fn open_and_refresh_from(session: &VaultSession, path: &Path, initial: Option<&str>) -> Result<()> {
    let hint = session.is_encrypted().then(|| session.root().to_path_buf());
    let launcher = |target: &Path| editor::open(target, hint.as_deref());

    let outcome = edit::edit_note(session, path, initial, &launcher);
    // Realign and refresh regardless of how the edit ended, so a note that was
    // written still gets its filename and views brought into step.
    reconcile::realign(session, path).context("while realigning the edited note")?;
    reconcile::refresh_views(session).context("while refreshing views")?;
    outcome?;
    Ok(())
}

/// Write the default-vault entry to the global config.
fn set_global_default(root: &Path) -> Result<()> {
    let path = global::config_path()
        .ok_or_else(|| anyhow!("no global config directory is available on this system"))?;
    let mut config = global::load_at(&path).unwrap_or_default();
    config.default_vault = Some(root.to_path_buf());
    global::write_at(&path, &config).context("while writing the global config")?;
    Ok(())
}

/// Print an "ambiguous selector" error and the candidate notes to stderr,
/// each as the shared human reference (ADR 0025).
fn report_ambiguous(selector: &str, notes: &[ntropy::note::Note]) -> Result<()> {
    eprintln!(
        "error: `{selector}` is ambiguous ({} matches):",
        notes.len()
    );
    for note in notes {
        eprintln!("  {}", output::note_reference(note)?);
    }
    eprintln!("refine the query or pass a full ULID");
    Ok(())
}

/// Prompt on the controlling terminal for a yes/no confirmation.
///
/// Question and answer both bypass stdout/stdin, so a redirected stream can
/// neither swallow the prompt nor feed the answer (ADR 0036). Confirmation is
/// only ever requested in interactive mode, which guarantees the terminal
/// exists.
pub(super) fn confirm(prompt: &str) -> Result<bool> {
    let tty = interact::open_tty().context("while opening the controlling terminal")?;
    write!(&tty, "{prompt}").context("while writing the confirmation prompt")?;
    (&tty).flush()?;
    let mut line = String::new();
    std::io::BufReader::new(&tty).read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// `None` for a blank query string, otherwise the trimmed query.
fn optional(query: &str) -> Option<&str> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// The exit code for a command that ran but hit warnings under `--strict`.
///
/// `extra` folds in warnings that are not scan warnings (the render engine's
/// degradation reports), so those also fail a strict run; commands with none
/// pass `0`.
fn exit_for_warnings(strict: bool, warnings: &[ScanWarning], extra: usize) -> ExitCode {
    if strict && (!warnings.is_empty() || extra > 0) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Format a count with its unit, choosing the singular or plural form.
pub(super) fn plural(count: usize, singular: &str, plural: &str) -> String {
    let unit = if count == 1 { singular } else { plural };
    format!("{count} {unit}")
}

/// Report `.gitignore` changes, one line per entry.
///
/// A pruned entry also warns that the view's directory was *not* removed —
/// ntropy never deletes directories — so the user can clean it up themselves.
fn report_gitignore_changes(added: &[String], removed: &[String]) {
    for entry in added {
        println!("ignored {entry}");
    }
    for entry in removed {
        let dir = entry.trim_matches('/');
        println!(
            "stopped ignoring {entry}; left directory `{dir}/` in place, delete it manually if you no longer need it"
        );
    }
}

/// The file-name component of a path as a lossy string.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
