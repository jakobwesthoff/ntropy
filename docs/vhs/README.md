# Demo Video Recording

This folder contains the [VHS](https://github.com/charmbracelet/vhs) tape used
to record the ntropy demo video.

## Prerequisites

- [VHS](https://github.com/charmbracelet/vhs) installed (`brew install vhs`).
- `ntropy` and [`mkulid`](https://github.com/jakobwesthoff/mkulid) on your
  `PATH`. `mkulid` pins each seeded note to a fixed creation date so the
  picker's date column is stable between recordings.
- `$EDITOR` set to Neovim. The note-creation segment drives **your real, local
  Neovim configuration** — there is no isolated config. The scripted keystrokes
  (`3G`, `f[`, `o`, `:wq`, …) assume stock Neovim motions; a config that
  remaps those normal-mode keys, or that pops a completion menu which captures
  `<Enter>`, will throw the recording off. Record with a config you trust, or
  temporarily point `$EDITOR` at a vanilla Neovim.
- The `VictorMono NFM SemiBold` Nerd Font installed (matches the Ghostty
  SemiBold setup). Swap `Set FontFamily` in `demo.tape` for any installed font
  if you don't have it.

## Recording

```bash
./record.sh
```

This will:

1. Build a throwaway vault at `/tmp/ntropy-demo`.
2. Seed it with a small, coherent set of prepared notes (work / Rust /
   learning / cooking) carrying the tags the demo queries against.
3. Materialise the `by-tag` view with `ntropy reconcile`.
4. Record `demo.tape` against that vault.
5. Tear the vault down again.

## Output

- `demo.webm` — WebM (smaller, modern browsers)
- `demo.mp4` — MP4 (broader compatibility)

Both land in `docs/pages/assets/`, where the landing page generator picks them
up. They are committed alongside the page so the GitHub Pages deploy has the
video without rendering VHS in CI; re-run `./record.sh` to refresh them.

## What the demo shows

1. `ntropy new` — a note stamped from a template and edited in Neovim: two tags
   added inside the frontmatter, one sentence of body.
2. `ntropy search tag:rust and not tag:reading` — the query language with a
   negation term, opening the interactive picker; filtered live to the new note
   and opened to confirm it stuck.
3. `ntropy view add by-status --field status` + `tree by-status` — turning a
   frontmatter field into a materialized view and browsing it as a plain
   directory of symlinks.

## Customising

Edit `demo.tape` for the on-screen flow and `record.sh` for the seeded corpus.
The theme is the ntropy palette: the brand teal (`#2dd4bf`) sits in the cyan
slot, so the picker's current row, the cursor, and the view's symlinks all pick
it up. Swap `Set Theme` for your own. See the
[VHS docs](https://github.com/charmbracelet/vhs) for available commands.
