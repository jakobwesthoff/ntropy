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
permission error, without restoring what it had already removed. Even a
valid name led elsewhere when the view's directory, or a directory above it,
was a symlink, as a cloned or synced tree can carry: the sync listed and
emptied the link's target.

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

A valid name keeps the path inside the vault only while no symlink sits on
it. Before a view is synced or its tree removed, each segment of its path
below the vault root is inspected without following it:

- If a segment is a symlink, the sync skips the view with the same warning
  as an invalid name, which fails `reconcile` under `--strict`. The view
  keeps its `.gitignore` entry, since its name is valid and still
  configured. `site` never touches view directories and does not check.
- `view add` refuses such a name, and the message says to remove the
  symlink itself rather than what it points to.
- `vault encrypt` refuses to start, and a resumed conversion skips the view
  and leaves the link in place. A path that cannot be inspected at all, for
  example one longer than the platform allows, is handled the same way by
  `vault encrypt`, so nothing can fail between deleting the plaintext notes
  and removing the conversion marker.

## Consequences

- Names that `view add` accepted before, such as `By_Tag`, `my.view` or
  `area work`, are invalid. Such a view stays skipped until it is removed and
  added again under a valid name, and its old directory is left on disk.
- The symlink check and the sync's later calls are separate lookups by
  path, so a symlink swapped in between them is not caught. From the check
  to the end of the sync's listing of the view's tree, a swapped directory
  makes the listing read the link's target, and everything listed there is
  removed; the window grows with the size of the tree. After the listing, a
  swap only redirects the removal of paths already listed. Removals never
  follow the last component of a path, so only a swapped directory above a
  removed file matters. The writer can be a sync client or a `git pull`
  that changes the vault while ntropy runs. Closing the window would take
  descriptor-relative calls (`openat` with `O_NOFOLLOW` per segment, then
  `unlinkat`, `mkdirat` and `symlinkat`) for the whole sync, and macOS has
  no `openat2` with `RESOLVE_NO_SYMLINKS`. A second check of the root's
  device and inode after the listing would narrow the window without
  closing it and was not adopted.
