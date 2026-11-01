---
title: "A link to a heading in another note is not resolved in site and html exports"
kind: bug
component: site
origin: implementation
---
# A link to a heading in another note is not resolved in site and html exports

A Markdown link to a heading in another note, such as
`[Inside tmux](01M68C1XVP8WV9Z6NWRJ1BEWT8-notifications-the-terminal-and-tmux.md#inside-tmux)`,
is not resolved by `ntropy site`: the `#inside-tmux` fragment is taken as
part of the file name, the export warns, and `--strict` fails. Found on
2026-11-01 in asqr's user guide (an ntropy vault at `docs/guide/`), which
had to fall back to links to whole pages.

## Problem

Reproduced on 2026-11-01 with ntropy 2.7.1, exporting asqr's guide at
asqr commit 4119818:

```
$ ntropy --vault docs/guide -n site -o out --force --strict
warning: referenced file not found in the vault: .../docs/guide/all-notes/01M68C1XVP8WV9Z6NWRJ1BEWT8-notifications-the-terminal-and-tmux.md#inside-tmux
...
```

The warning comes from `src/site/build.rs` (the "referenced file not
found in the vault" message); `src/render/html/engine.rs` checks the same
text, so `render --to html` likely shares the resolution path. A link to a
heading in the same note (`#inside-tmux` alone) is a different case and
not part of this report.

## Expected

A link `<note>.md#<anchor>` resolves to the page of that note with the
fragment kept (`.../notes/<slug>.html#<anchor>`), in `site` and in
`render --to html`. Whether the anchor must match a heading's id on the
target page (and warn when it does not) is to be decided while planning;
the id rule must be the one the heading ids on the exported pages use.

## After the fix

asqr's guide (branch `user-guide-prep`) links whole pages since commit
bbf98db; its links can point at the sections again once this ships.
