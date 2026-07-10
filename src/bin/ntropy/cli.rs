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
    Info,

    /// Run the language server over stdin/stdout.
    // ADR 0029 governs the language-server surface.
    Lsp,
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
