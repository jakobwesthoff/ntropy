# 64. View names are validated slug segments

Date: 2026-10-30

## Status

Accepted

Amends [ADR 0007](0007-vault-directory-layout.md), whose reserved names now
apply to the first segment of a view name, and
[ADR 0019](0019-scan-robustness-and-resource-tolerance.md), whose `--strict`
now also fails on a skipped view.

## Context

A view's name, from `config.toml` or `view add`, is joined onto the vault root
as the view's directory. The view sync lists that directory recursively and
removes every file and symlink in it that is not one of the view's links.
Nothing constrained the name: an absolute path or a `..` segment reached
outside the vault, `.git` reached a hidden directory inside it, and
`all-notes/x` passed the reserved-name check, which compared the whole string.
The sync removed files in sorted path order and stopped at the first
permission error, without restoring what it had already removed.

The name reached other paths too. `vault encrypt` deleted each configured
view's directory recursively, and `site` wrote pages under `views/<name>/` in
its output directory. In `.gitignore`, a name with a newline injected lines,
and an absolute name made the sync panic.

## Decision

A view name is one or more `/`-separated segments. It is valid when:

- it is not empty and holds no control character;
- no segment is empty, so it has no leading or trailing `/` and no `//`;
- its first segment is not a reserved name (`all-notes`, `.ntropy`,
  `.gitignore`, `README.md`);
- no segment starts with `.`;
- every segment is already in slug form (ADR 0023): lowercase ASCII letters,
  digits and single dashes, at most 72 characters.

Multi-segment names such as `area/work` stay valid.

The library builds a view's directory only from a type that this validation
alone constructs, so every path derived from a view name has passed it.

- `view add` refuses an invalid name, and names the nearest valid one when
  there is one.
- A configured view with an invalid name is skipped with a warning wherever
  views are synced or a site is exported, and the other views carry on.
  Under `--strict`, the warning fails `reconcile` and `site`. The skipped
  view's managed `.gitignore` entry is kept, and none is added for it.
- `view list` and `view remove` work on any configured name. `view remove`
  only edits the configuration and builds no path from the name.
- `vault encrypt` refuses to start while a configured view has an invalid
  name. A tree it cannot remove would stay in the vault, its link names
  spelling out note titles in plaintext. `vault encrypt --resume` skips such
  views and reports them.

A valid name still names a directory that ntropy owns (ADR 0007). A view named
after an existing directory of the vault, or nested in another view's
directory, is synced like any other view.

## Consequences

- Names that `view add` accepted before, such as `By_Tag`, `my.view` or
  `area work`, are invalid. Such a view stays skipped until it is removed and
  added again under a valid name, and its old directory is left on disk.
