---
title: Getting started
tags: [docs/start]
site:
  index: true
  label: Getting started
  order: 1
---
Three steps make ntropy ready to use: install it, tell it which editor to
open, and create a vault for your notes.

## Install

ntropy is published on crates.io:

```bash
cargo install ntropy
```

On macOS and Linux it also installs from the
[Homebrew tap](https://github.com/jakobwesthoff/homebrew-tap):

```bash
brew install jakobwesthoff/tap/ntropy
```

The formula also installs typst, which rendering a PDF needs, and the
shell script that defines `ncd`, a command that changes into your vault.
It needs one `source` line in your shell's startup file, given in
[Scripting and the shell](01M28Q32N56MS70ETPES4794HJ-scripting-and-the-shell.md).

Each release also ships binaries on the
[GitHub Releases](https://github.com/jakobwesthoff/ntropy/releases) page,
for macOS (Apple Silicon and Intel) and Linux (x86_64 and aarch64,
statically linked).

> [!NOTE]
> ntropy supports macOS and Linux. Windows is not supported; the reason
> is in [Limitations](01M28Q32Q4M6KYH4YXJ3EPDHQT-limitations.md). To build
> ntropy from source, see
> [Development](01M28Q32PFZCYEGS4TCN4Z0GN9-development.md).

## Set your editor

ntropy opens a note in the editor that `$VISUAL` names, or else
`$EDITOR`. With neither set, `new`, `today` and `search` cannot open a
note and stop with an error. Set one in your shell's startup file, such
as `~/.zshrc` or `~/.bashrc`:

```bash
export EDITOR=nvim
```

> [!TIP]
> The value may carry arguments, as in `EDITOR="code -w"`.
> [Configuration](01M28Q32E6BHWB0H27MM57KY90-configuration.md) has the
> details.

## Create a vault

A vault is a directory that holds your notes. Create one and change into
it:

```bash
ntropy init ~/notes
cd ~/notes
```

`init` creates `all-notes/` for the notes, a `by-tag` view that files
them by tag, a `README.md`, and `.ntropy/` with the vault's settings and
templates. Inside the vault, or in any directory below it, every command
finds it on its own.

> [!TIP]
> `ntropy init ~/notes --set-default` also makes it the default vault,
> which commands use wherever no other vault is found.
> [Finding the vault](01M28Q32CAAZA2PWAP4RR44GMA-finding-the-vault.md)
> has the rules.
