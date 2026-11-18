// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The clap command surface (ADR 0018, `docs/design/cli.md`).
//!
//! Global flags (`--vault`, `-n/--non-interactive`, `--strict`) are available
//! on every subcommand. `new` takes its title as repeated positional arguments
//! which the `run` layer joins with spaces. `search`, `delete`, `render`,
//! `show` and `site` take a query or selector as repeated positional
//! arguments, each read on its own by `ntropy::query::from_arguments`
//! (ADR 0076); a lone argument naming a note's file is a path (ADR 0091).
//! `edit` is a hidden alias of `search` (ADR 0031). `search`,
//! `new` and `today` take `--id` beside `-p` to print a note's ULID instead of
//! its path (ADR 0090). `skill` takes an optional part name or `--install <DIR>`
//! (ADR 0092). A bare `ntropy` prints help.

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

    /// Treat malformed/badly-named notes and skipped views as errors instead of warnings.
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

/// The subcommands of `ntropy asset`.
#[derive(Subcommand, Debug)]
pub enum AssetCommand {
    /// Copy files into a note's asset folder and print their Markdown links.
    ///
    /// Each link goes to stdout, `![alt](..)` for an image and `[name](..)`
    /// for any other file, with the target in angle brackets when the name
    /// needs them; `added <path>` lines go to stderr. The note is never
    /// edited: place the links yourself. Every check runs before the first file
    /// is touched; an I/O failure during the transfer leaves the files already
    /// placed, which are named in the output. Two names that differ only in case, from each other or from a
    /// file of the folder, are refused, and so is a note of the vault. With
    /// `--move`, taking a file out of another note's asset folder warns.
    Add {
        /// Move the files instead of copying them.
        #[arg(long = "move")]
        move_files: bool,
        /// Store the one file under this name.
        #[arg(long = "as", value_name = "NAME")]
        rename: Option<String>,
        /// Replace a file of the same name in the folder.
        #[arg(long)]
        force: bool,
        /// The note: a full ULID, a note's filename or path, or a query. It
        /// is one argument, so quote a query that holds spaces.
        ///
        /// A lone argument that ends in `.md` or `.age` and names an existing file
        /// selects the note in it, as for `render`. Several matching notes open
        /// the picker on a terminal and are an error under `-n`.
        #[arg(value_name = "ID|PATH|QUERY")]
        selector: String,
        /// The files to add.
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
    },

    /// List a note's asset files with their size and whether the note links
    /// them.
    ///
    /// A file that raw HTML in the note names counts as linked. A file a
    /// Markdown link names in the note's folder and the folder lacks is listed as
    /// `broken`, and so is a link that names another folder of the note, shown as
    /// written. Hidden files are listed; only operating-system metadata such as
    /// `.DS_Store` is left out.
    ///
    /// `--path` prints the path of each asset file instead, `--folder` the path
    /// of the folder.
    List {
        /// Print the absolute path of every asset file, one per line.
        #[arg(long, conflicts_with = "folder")]
        path: bool,
        /// Print the asset folder's absolute path, also when it does not exist
        /// yet.
        #[arg(long)]
        folder: bool,
        /// A full ULID, a note's filename or path, or a query DSL expression,
        /// as trailing arguments that are each read on their own; words in one
        /// argument form a phrase (omitted = choose from all notes).
        #[arg(value_name = "ID|PATH|QUERY")]
        selector: Vec<String>,
    },
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
        /// Create and print the ULID only; do not open the editor.
        #[arg(long, conflicts_with = "print")]
        id: bool,
    },

    /// Open today's note, creating it from the `today` template if absent.
    Today {
        /// Create and print the path only; do not open the editor.
        // `--no-edit` is accepted as a hidden alias for
        // backward compatibility (ADR 0035).
        #[arg(short = 'p', long, alias = "no-edit")]
        print: bool,
        /// Find or create today's note and print its ULID only; do not open
        /// the editor.
        #[arg(long, conflicts_with = "print")]
        id: bool,
    },

    /// Browse, filter, full-text search, or open a note by id or query.
    ///
    /// A single match opens directly; several open the picker. `edit` is a
    /// hidden alias for this command (ADR 0031).
    #[command(visible_alias = "list", alias = "edit")]
    Search {
        /// A full ULID, a note's filename or path, or a query DSL expression,
        /// as trailing arguments that are each read on their own; words in one
        /// argument form a phrase
        /// (omitted = all notes).
        ///
        /// A lone argument that ends in `.md` or `.age` and names an existing file
        /// selects the note in it: a bare filename is looked up in `all-notes/`, any
        /// other path is taken as given, and a symlink to a note, such as a view entry,
        /// selects that note. A file that is no note of the vault is an error, and so is
        /// the name of a note file that does not exist. Any other argument ending in
        /// `.md` or `.age` is a query.
        #[arg(value_name = "ID|PATH|QUERY")]
        query: Vec<String>,
        /// Print the selected note's path instead of opening the editor.
        // `--no-edit` is accepted as a hidden alias for consistency with
        // `new`/`today` (ADR 0035).
        #[arg(short = 'p', long, alias = "no-edit")]
        print: bool,
        /// Print the selected note's ULID instead of opening the editor.
        #[arg(long, conflicts_with_all = ["print", "print_content"])]
        id: bool,
        /// Print the note's content to stdout instead of opening the editor.
        ///
        /// Must resolve to exactly one note. Reads identically whether or not
        /// the vault is encrypted, which `--print` cannot: there the path
        /// names a ciphertext file.
        #[arg(short = 'P', long, conflicts_with = "print")]
        print_content: bool,
    },

    /// Replace one note's content with text read from stdin.
    ///
    /// The note is named, never searched for: a full ULID, the note's filename,
    /// its path in `all-notes/`, or a symlink to it such as a view entry.
    /// Nothing is prompted for and no picker opens, so this composes into a
    /// pipeline; a target that names no note, or more than one, is an error. The text must be a well-formed note (frontmatter
    /// block with a `title`) or it is refused before anything is written.
    ///
    /// Works on an encrypted vault, including a locked one, which is what makes
    /// it the way to author notes there without an editor (ADR 0043).
    Write {
        /// The note to write to.
        #[arg(value_name = "ULID|FILENAME|PATH")]
        target: String,
    },

    /// Realign drifted filenames, re-sync views, and sync `.gitignore`.
    ///
    /// Brings the root `.gitignore` in line with the configured views, adding
    /// missing entries and pruning those for views you have removed. ntropy
    /// never deletes a view's directory, so a removed view's directory is left in place
    /// and reported for you to delete.
    Reconcile,

    /// Delete a note by id or query.
    ///
    /// An empty selector is refused instead of matching every note. The
    /// note's asset folder, `all-notes/_assets/<note file name>/`, is removed
    /// with it, and the confirmation says so. When another note links a file
    /// in the folder, the folder stays, the notes that link it are named, and
    /// `--remove-assets` removes it anyway.
    Delete {
        /// A full ULID, a note's filename or path, or a query DSL expression,
        /// as trailing arguments that are each read on their own; words in one
        /// argument form a phrase.
        ///
        /// A lone argument that ends in `.md` or `.age` and names an existing file
        /// selects the note in it: a bare filename is looked up in `all-notes/`, any
        /// other path is taken as given, and a symlink to a note, such as a view entry,
        /// selects that note. A file that is no note of the vault is an error, and so is
        /// the name of a note file that does not exist. Any other argument ending in
        /// `.md` or `.age` is a query.
        #[arg(required = true, value_name = "ID|PATH|QUERY")]
        selector: Vec<String>,
        /// Skip the confirmation prompt.
        #[arg(short = 'f', long)]
        force: bool,
        /// Remove the note's asset folder even when other notes link a file
        /// in it. Without the flag such a folder stays and the notes that
        /// link it are named.
        #[arg(long)]
        remove_assets: bool,
    },

    /// Render a note to a document artifact.
    ///
    /// `--to` selects the output format: `pdf` (the default), `typst`, the
    /// emitted Typst document, `html`, a web page with a `<stem>_files/`
    /// directory beside it, or `ansi`, the note drawn for a terminal with
    /// ANSI escape sequences at 80 columns. `pdf` is produced by ntropy's own typst engine, which compiles
    /// the note with the external `typst` binary, so only `typst` need be on
    /// `PATH`. The other formats need no external tool.
    ///
    /// The look comes from the vault's config in `.ntropy/config.toml`:
    /// `[render] theme`, a Typst file in `.ntropy/themes/typst/`, for `pdf`
    /// and `typst`; `[site] theme`, a directory in `.ntropy/themes/site/`,
    /// for `html`. `--theme` overrides it for one invocation. `ansi` takes no
    /// theme.
    Render {
        /// A full ULID, a note's filename or path, or a query DSL expression,
        /// as trailing arguments that are each read on their own; words in one
        /// argument form a phrase
        /// (omitted = choose from all notes).
        ///
        /// A lone argument that ends in `.md` or `.age` and names an existing file
        /// selects the note in it: a bare filename is looked up in `all-notes/`, any
        /// other path is taken as given, and a symlink to a note, such as a view entry,
        /// selects that note. A file that is no note of the vault is an error, and so is
        /// the name of a note file that does not exist. Any other argument ending in
        /// `.md` or `.age` is a query.
        #[arg(value_name = "ID|PATH|QUERY")]
        selector: Vec<String>,
        /// The output format: `pdf` (default), `typst`, `html`, or `ansi`.
        #[arg(long, value_name = "FORMAT", default_value = ntropy::render::DEFAULT_FORMAT)]
        to: String,
        /// Override the format's default engine.
        #[arg(long, value_name = "NAME")]
        engine: Option<String>,
        /// Write the artifact here instead of `./<slug>.<ext>`.
        #[arg(short = 'o', long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Render with this theme instead of the vault's configured one.
        ///
        /// Names `<vault>/.ntropy/themes/typst/<NAME>.typ` for `pdf` and
        /// `typst`, or the directory `<vault>/.ntropy/themes/site/<NAME>/`
        /// for `html`. `default` selects ntropy's built-in look, overriding a
        /// configured theme.
        #[arg(long, value_name = "NAME")]
        theme: Option<String>,
        /// Print the artifact's path to stdout on success.
        #[arg(short = 'p', long)]
        print: bool,
        /// Replace an existing artifact, and for `html` a non-empty files
        /// directory beside it; without it either is refused.
        #[arg(long)]
        force: bool,
    },

    /// Show a note drawn for the terminal.
    ///
    /// The note's frontmatter, then its Markdown with headings, emphasis,
    /// lists, tables, quotes, callouts and highlighted code styled, laid out
    /// at the terminal's width. A note taller than the screen goes through
    /// `$PAGER`, or `less -R` when it is unset; `-n` never pages. Without a
    /// terminal on stdout the text comes without escape sequences, and
    /// `NO_COLOR` leaves out the colours. Must resolve to exactly one note,
    /// like `render`.
    Show {
        /// A full ULID, a note's filename or path, or a query DSL expression,
        /// as trailing arguments that are each read on their own; words in one
        /// argument form a phrase
        /// (omitted = choose from all notes).
        ///
        /// A lone argument that ends in `.md` or `.age` and names an existing file
        /// selects the note in it: a bare filename is looked up in `all-notes/`, any
        /// other path is taken as given, and a symlink to a note, such as a view entry,
        /// selects that note. A file that is no note of the vault is an error, and so is
        /// the name of a note file that does not exist. Any other argument ending in
        /// `.md` or `.age` is a query.
        #[arg(value_name = "ID|PATH|QUERY")]
        selector: Vec<String>,
        /// Lay the note out at this many columns instead of the terminal's
        /// width (80 without a terminal).
        #[arg(long, value_name = "COLUMNS", value_parser = clap::value_parser!(u16).range(1..))]
        width: Option<u16>,
    },

    /// Export the vault as a static website.
    ///
    /// Every note becomes a page under `notes/`, beside a tag tree, one page
    /// per configured view and group, and a front page. Pages carry a
    /// sidebar, breadcrumbs, an outline, and previous/next links. The site
    /// works when opened straight from disk; no server is needed.
    ///
    /// The look comes from `[site] theme` in `.ntropy/config.toml`, a
    /// directory in `.ntropy/themes/site/`; `--theme` overrides it for one
    /// invocation. `site theme init` copies the built-in theme into the vault.
    #[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
    Site {
        #[command(subcommand)]
        command: Option<SiteCommand>,
        /// The output directory. A non-empty directory is refused unless
        /// `--force` empties it first.
        #[arg(short = 'o', long, value_name = "DIR", required = true)]
        output: Option<PathBuf>,
        /// A query DSL expression restricting the exported notes, as trailing
        /// arguments that are each read on their own; words in one argument
        /// form a phrase (omitted = every note).
        #[arg(value_name = "QUERY")]
        query: Vec<String>,
        /// Export with this site theme instead of the vault's configured one.
        ///
        /// Names the directory `<vault>/.ntropy/themes/site/<NAME>/`.
        /// `default` selects the built-in theme, overriding a configured one.
        #[arg(long, value_name = "NAME")]
        theme: Option<String>,
        /// Empty a non-empty output directory before writing.
        #[arg(long)]
        force: bool,
        /// Print the path of the site's `index.html` to stdout on success.
        #[arg(short = 'p', long)]
        print: bool,
    },

    /// Add files to a note's asset folder and list what is there.
    ///
    /// A note's assets live in `all-notes/_assets/<note file name>/`. Both
    /// commands work on exactly one note, selected as `render` and `delete`
    /// select theirs. They do not work in an encrypted vault.
    #[command(subcommand)]
    Asset(AssetCommand),

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

    /// Print the agent skill compiled into this ntropy, or install it.
    ///
    /// Prints the skill's core, or one part of it, so an agent reads the skill
    /// of the ntropy that runs. `--install <DIR>` writes it to `<DIR>/ntropy/`
    /// for a skills directory such as `~/.claude/skills`. Needs no vault
    /// (ADR 0092).
    Skill {
        /// Part of the skill to print in place of its core.
        #[arg(
            value_parser = clap::builder::PossibleValuesParser::new(ntropy::skill::part_names()),
            conflicts_with = "install"
        )]
        part: Option<String>,
        /// Write the skill into DIR/ntropy, replacing its references/ directory.
        #[arg(long, value_name = "DIR")]
        install: Option<PathBuf>,
    },
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
pub enum SiteCommand {
    /// Manage site themes.
    #[command(subcommand)]
    Theme(SiteThemeCommand),
}

#[derive(Subcommand, Debug)]
pub enum SiteThemeCommand {
    /// Copy the built-in site theme into the vault as the starting point for
    /// a custom one.
    ///
    /// Writes `<vault>/.ntropy/themes/site/<NAME>/` and refuses to overwrite
    /// an existing directory. Select it with `[site] theme = "<NAME>"`.
    Init {
        /// The new theme's name.
        name: String,
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
    /// The view's directory is left on disk (ntropy never deletes a view's directory)
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
