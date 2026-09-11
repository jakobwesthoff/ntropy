---
title: Getting started
tags: [docs/start]
site:
  index: true
  label: Getting started
  order: 1
---
This section gets ntropy onto your machine and takes you through the first
vault. Install it below, scaffold a vault, and write a note. Then read
[A day with ntropy](01M28Q328EEEJKWA58R2DEYKKQ-a-day-with-ntropy.md), a short
tour of how the commands fit together, and keep
[Commands](01M28Q3292T4MPPX6NZN69PN6H-commands.md), the reference table of
every command and flag, within reach.

## Installation

ntropy is published on crates.io:

```bash
cargo install ntropy
```

### Pre-built binaries

Each release ships binaries on the
[GitHub Releases](https://github.com/jakobwesthoff/ntropy/releases) page for
macOS (Apple Silicon and Intel) and Linux (x86_64 and aarch64, statically
linked).

### Building from source

```bash
git clone https://github.com/jakobwesthoff/ntropy.git
cd ntropy
cargo build --release
cargo install --path .   # install your working copy onto your PATH
```

> [!NOTE]
> ntropy supports macOS and Linux. Windows is not supported; the reason is in
> [Limitations](01M28Q32Q4M6KYH4YXJ3EPDHQT-limitations.md).

## Quick start

```bash
# Scaffold a vault (this also seeds a by-tag view)
ntropy init ~/notes
cd ~/notes

# Create a note from a template and open it in your editor
ntropy new My first note

# Open today's daily note (created on first use each day)
ntropy today

# Find and open notes: full query language, fuzzy picker when several match
ntropy search tag:work and not status:done
```

`new` stamps the note out of a
[template](01M28Q32BNT6XKW8DFW56GH06T-templates-and-daily-notes.md), and
`today` has a daily template of its own. `search` takes an expression in the
[query language](01M28Q32FC8RW01C5DSEEV77DW-query-language.md); one match
opens in your editor, several open the
[interactive picker](01M28Q32FZCNHV2PZ5FSG8E360-the-interactive-picker.md).

You never have to tell ntropy which vault you mean from inside one. It finds
the vault on its own; the rules are in
[Finding the vault](01M28Q32CAAZA2PWAP4RR44GMA-finding-the-vault.md).
