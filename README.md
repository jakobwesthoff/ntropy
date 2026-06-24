# ntropy

> An opinionated Markdown note-taking CLI where metadata, not folders, is the
> filing system.

ntropy manages a collection of notes that are plain Markdown files with YAML
frontmatter, authored in your own `$EDITOR`. Organization lives in the
frontmatter — tags, dates, and arbitrary fields — rather than in a folder
hierarchy you maintain by hand.

The tool is deliberately opinionated: notes live flat in a single vault, a
note's identity is stable and independent of its title, and any hierarchy you
want to browse is a derived projection of the metadata rather than the
canonical storage.

The design is documented as decision records under [`docs/adr/`](docs/adr/) and
narrative design documents under [`docs/design/`](docs/design/).

## License

ntropy is licensed under the Mozilla Public License 2.0. See [`LICENSE`](LICENSE).
