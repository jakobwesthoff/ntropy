// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The clap command surface (ADR 0018, `docs/design/cli.md`).
//!
//! Global flags (`--vault`, `-n/--non-interactive`, `--strict`) are available
//! on every subcommand. `new`, `search` and `delete` take their free text as
//! repeated positional arguments which the `run` layer joins into one string
//! (the title / query / selector). `edit` is a hidden alias of `search`
//! (ADR 0031). A bare `ntropy` prints help.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "ntropy",
    version,
    about = "An opinionated Markdown note-taking and management CLI.",
    subcommand_required = false,
    arg_required_else_help = false
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Flags shared by every subcommand.
#[derive(Args, Debug)]
pub struct GlobalArgs {
    /// Operate on the vault at this path (overrides all other resolution).
    #[arg(long, global = true, value_name = "PATH")]
    pub vault: Option<PathBuf>,

    /// Force non-interactive (plain) behavior even on a TTY.
    #[arg(short = 'n', long = "non-interactive", global = true)]
    pub non_interactive: bool,

    /// Treat malformed/badly-named notes as errors instead of warnings.
    #[arg(long, global = true)]
    pub strict: bool,

    /// Use this age identity file instead of the OS credential store.
    ///
    /// `$NTROPY_IDENTITY` is consulted when the flag is absent.
    #[arg(short = 'i', long, global = true, value_name = "PATH")]
    pub identity: Option<PathBuf>,

    /// Read the vault passphrase from the first line of this file.
    ///
    /// Supplies the passphrase wherever one would otherwise be typed: creating
    /// an encrypted vault, unlocking one, or converting one. On
    /// `vault passphrase` it is the *current* passphrase.
    #[arg(long, global = true, value_name = "PATH")]
    pub passphrase_file: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize a vault (idempotent).
    Init {
        /// Where to create the vault (defaults to the current directory).
        path: Option<PathBuf>,
        /// Also record this vault as the global default.
        #[arg(long)]
        set_default: bool,
        /// Store the vault's notes encrypted at rest (ADR 0041).
        #[arg(long)]
        encrypted: bool,
    },

    /// Create a note from a template and open it.
    New {
        /// The note title (joined from all trailing arguments).
        #[arg(required = true, value_name = "TITLE")]
        title: Vec<String>,
        /// Template to use: `<name>.md` in `.ntropy/templates/` (default:
        /// `default`).
        #[arg(short = 't', long, value_name = "NAME")]
        template: Option<String>,
        /// Create the file empty instead of stamping a template, for callers
        /// that write the note's frontmatter and body themselves.
        #[arg(long, conflicts_with = "template")]
        empty: bool,
        /// Create and print the path only; do not open the editor.
        // `--no-edit` is accepted as a hidden alias for
        // backward compatibility (ADR 0035).
        #[arg(short = 'p', long, alias = "no-edit")]
        print: bool,
    },

    /// Open today's note, creating it from the `today` template if absent.
    Today {
        /// Create and print the path only; do not open the editor.
        // `--no-edit` is accepted as a hidden alias for
        // backward compatibility (ADR 0035).
        #[arg(short = 'p', long, alias = "no-edit")]
        print: bool,
    },

    /// Browse, filter, full-text search, or open a note by id or query.
    ///
    /// A single match opens directly; several open the picker. `edit` is a
    /// hidden alias for this command (ADR 0031).
    #[command(visible_alias = "list", alias = "edit")]
    Search {
        /// A full ULID or a query DSL expression (joined from trailing
        /// arguments; omitted = all notes).
        #[arg(value_name = "ID|QUERY")]
        query: Vec<String>,
        /// Print the selected note's path instead of opening the editor.
        // `--no-edit` is accepted as a hidden alias for consistency with
        // `new`/`today` (ADR 0035).
        #[arg(short = 'p', long, alias = "no-edit")]
        print: bool,
        /// Print the note's content to stdout instead of opening the editor.
        ///
        /// Must resolve to exactly one note. Reads identically whether or not
        /// the vault is encrypted, which `--print` cannot: there the path
        /// names a ciphertext file.
        #[arg(short = 'P', long, conflicts_with = "print")]
        print_content: bool,
    },

    /// Realign drifted filenames, re-sync views, and sync `.gitignore`.
    ///
    /// Brings the root `.gitignore` in line with the configured views, adding
    /// missing entries and pruning those for views you have removed. ntropy
    /// never deletes a directory, so a removed view's directory is left in place
    /// and reported for you to delete.
    Reconcile,

    /// Delete a note by id or query.
    Delete {
        /// A full ULID or a query DSL expression (joined from trailing
        /// arguments).
        #[arg(required = true, value_name = "ID|QUERY")]
        selector: Vec<String>,
        /// Skip the confirmation prompt.
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Render a note to a document artifact.
    ///
    /// `--to` selects the output format: `pdf` (the default) or `typst`, the
    /// emitted Typst document. `pdf` is produced by ntropy's own typst engine,
    /// which compiles the note with the external `typst` binary, so only `typst`
    /// need be on `PATH`. The `typst` format needs no external tool.
    Render {
        /// A full ULID or a query DSL expression (joined from trailing
        /// arguments; omitted = choose from all notes).
        #[arg(value_name = "ID|QUERY")]
        selector: Vec<String>,
        /// The output format: `pdf` (default) or `typst`.
        #[arg(long, value_name = "FORMAT", default_value = ntropy::render::DEFAULT_FORMAT)]
        to: String,
        /// Override the format's default engine.
        #[arg(long, value_name = "NAME")]
        engine: Option<String>,
        /// Write the artifact here instead of `./<slug>.<ext>`.
        #[arg(short = 'o', long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Print the artifact's path to stdout on success.
        #[arg(short = 'p', long)]
        print: bool,
    },

    /// Manage materialized view definitions.
    #[command(subcommand)]
    View(ViewCommand),

    /// List all tags with note counts.
    Tags,

    /// Show the active vault, its resolution, and vault statistics.
    Info {
        /// Print the active vault's path alone, for shell use.
        #[arg(short = 'p', long)]
        print: bool,
    },

    /// Store the vault's identity so later commands need no passphrase.
    Unlock,

    /// Forget the stored identity, so reading requires the passphrase again.
    Lock,

    /// Whole-vault operations: converting storage and managing the key.
    // Grouped rather than top-level because these are rare and destructive,
    // unlike `lock`/`unlock`, which are part of daily use (ADR 0041).
    Vault {
        #[command(subcommand)]
        command: VaultCommand,
    },

    /// Run the language server over stdin/stdout.
    // ADR 0029 governs the language-server surface.
    Lsp,
}

#[derive(Subcommand, Debug)]
pub enum VaultCommand {
    /// Convert a plaintext vault to encrypted storage.
    Encrypt {
        /// Finish a conversion that was interrupted.
        #[arg(long)]
        resume: bool,
        /// Skip the confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Convert an encrypted vault back to plaintext storage.
    Decrypt {
        /// Finish a conversion that was interrupted.
        #[arg(long)]
        resume: bool,
        /// Skip the confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Generate a fresh keypair and re-encrypt every note to it.
    Rekey {
        /// Finish a rekey that was interrupted.
        #[arg(long)]
        resume: bool,
        /// Skip the confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
        /// Read the passphrase for the new key from this file's first line.
        ///
        /// Without it the global `--passphrase-file` is reused, so the vault
        /// keeps its current passphrase and only the key changes.
        #[arg(long, value_name = "PATH")]
        new_passphrase_file: Option<PathBuf>,
    },
    /// Change the passphrase protecting the vault's identity.
    ///
    /// Only the wrapped identity is rewritten; the notes are untouched,
    /// because the key inside the wrapper does not change.
    Passphrase {
        /// Read the new passphrase from the first line of this file.
        ///
        /// The current passphrase comes from the global `--passphrase-file`.
        #[arg(long, value_name = "PATH")]
        new_passphrase_file: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ViewCommand {
    /// List configured views.
    List,
    /// Define a new view grouping by a frontmatter field, and ignore its directory.
    Add {
        /// The view's output-directory name.
        name: String,
        /// The frontmatter field to group by.
        #[arg(long)]
        field: String,
    },
    /// Remove a view definition and prune its `.gitignore` entry.
    ///
    /// The view's directory is left on disk (ntropy never deletes a directory)
    /// and reported so you can remove it yourself.
    Remove {
        /// The view name to remove.
        name: String,
    },
}

/// Join repeated positional arguments into a single space-separated string.
pub fn join(parts: &[String]) -> String {
    parts.join(" ")
}
