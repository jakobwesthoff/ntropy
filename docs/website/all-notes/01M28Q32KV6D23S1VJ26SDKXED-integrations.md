---
title: Integrations
tags: [docs/integrate]
site:
  index: true
  label: Integrations
  order: 5
---
ntropy is a command-line program. Nothing runs inside it; editors, scripts, and coding agents all drive the same CLI, each through the door built for it. This section covers the three doors.

Editors talk to the [language server](01M28Q32MG1WQ0BTSBGCER8A2R-language-server.md). `ntropy lsp` gives any LSP-capable editor link completion, tag completion, go-to-definition, and a jump to any note by title. The page ends with a Neovim setup.

Scripts and the shell use the plain output described in [Scripting and the shell](01M28Q32N56MS70ETPES4794HJ-scripting-and-the-shell.md). With `-n` every command prints plain text on stdout, a `search` that finds nothing exits non-zero, and `info --print` is the hook behind the `ncd` shell function that jumps into the active vault.

LLM coding agents get the [agent skill](01M28Q32NS466VK7NWS6BFB2YP-agent-skill.md), a `SKILL.md` with reference docs that teaches an agent the house rules for the CLI: which flags keep it from blocking, and which shortcuts corrupt a vault.
