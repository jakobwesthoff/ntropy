// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The clap command surface (ADR 0018, `docs/design/cli.md`).
//!
//! Global flags (`--vault`, `-n/--non-interactive`, `--strict`) are available
//! on every subcommand. `new`, `search`, `edit` and `delete` take their free
//! text as repeated positional arguments which the `run` layer joins into one
//! string (the title / query / selector). A bare `ntropy` prints help.

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
        #[arg(long, visible_alias = "print")]
        no_edit: bool,
    },

    /// Open today's note, creating it from the `today` template if absent.
    Today {
        /// Create and print the path only; do not open the editor.
        #[arg(long, visible_alias = "print")]
        no_edit: bool,
    },

    /// Browse, filter or full-text search notes.
    #[command(visible_alias = "list")]
    Search {
        /// A query DSL expression (joined from trailing arguments; omitted =
        /// all notes).
        #[arg(value_name = "QUERY")]
        query: Vec<String>,
    },

    /// Open a specific note by id or query.
    Edit {
        /// A full ULID or a query DSL expression (joined from trailing
        /// arguments).
        #[arg(required = true, value_name = "ID|QUERY")]
        selector: Vec<String>,
    },

    /// Realign drifted filenames and rebuild views.
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

    /// Manage materialized view definitions.
    #[command(subcommand)]
    View(ViewCommand),

    /// List all tags with note counts.
    Tags,

    /// Show the active vault, its resolution, and vault statistics.
    Info,

    /// Run the language server over stdin/stdout (ADR 0029).
    Lsp,
}

#[derive(Subcommand, Debug)]
pub enum ViewCommand {
    /// List configured views.
    List,
    /// Define a new view grouping by a frontmatter field.
    Add {
        /// The view's output-directory name.
        name: String,
        /// The frontmatter field to group by.
        #[arg(long)]
        field: String,
    },
    /// Remove a view definition and its directory.
    Remove {
        /// The view name to remove.
        name: String,
    },
}

/// Join repeated positional arguments into a single space-separated string.
pub fn join(parts: &[String]) -> String {
    parts.join(" ")
}
