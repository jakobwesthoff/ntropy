---
title: Language server
tags: [docs/integrate]
site:
  order: 1
---
`ntropy lsp` runs a [Language Server](https://microsoft.github.io/language-server-protocol/) over stdin and stdout. An LSP server is the machinery that gives an editor autocomplete and go-to-definition for code. This one teaches any LSP-capable editor to understand an ntropy vault, so the fiddly parts of note-taking become ordinary editor features. This page lists what the server does and shows a Neovim setup.

## What the server does

Link completion. Type `[` and pick a note; the candidates are fuzzy-matched on title, tags, and filename. ntropy inserts the whole `[Title](<ulid>-<slug>.md)`, so a [link between notes](01M28Q32ACTNYR0FBYG9CAZMKG-note-format.md) never means hand-copying a ULID. Typing inside a hand-written `](` completes just the target. Editors that support snippets get the cursor placed right after the inserted link.

Tag completion. Inside a note's `tags:` frontmatter, completion offers the [tags](01M28Q32ACTNYR0FBYG9CAZMKG-note-format.md) already in the vault, hierarchy-aware: typing `programming/` narrows to its children. Both authored forms work, the single-line `[a, b]` list and the `- a` block list.

Go to definition and document links. Jump to, or click straight through, a link to the note it points at.

Workspace symbols. Jump to any note in the vault by title.

The server resolves the vault for each open document with the same rules as the CLI, so there is nothing to configure beyond pointing the editor at the binary. Candidates come from a scan of the saved files: a title or tag you changed in an open buffer shows up in completion after you save.

In an [encrypted vault](01M28Q32DHD3RH94HNF80RQNGT-encrypted-vaults.md) the server decrypts in memory and works the same, as long as the vault is unlocked. Following a link there opens a read-only decrypted copy of the target, good for reading; editing still goes through `ntropy search`.

## Neovim

For a recent Neovim (0.11 or later), start the server for Markdown buffers that live in a vault. Put this in your config:

```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "markdown",
  callback = function(args)
    local root = vim.fs.root(args.buf, { ".ntropy", ".ntropy-vault" })
    if not root then
      return -- not inside an ntropy vault
    end
    vim.lsp.start({
      name = "ntropy",
      cmd = { "ntropy", "lsp" },
      root_dir = root,
    })
  end,
})

-- Optional: snippet support makes `[` completion place the cursor after the link.
-- (Neovim's built-in client advertises it; nvim-cmp/blink users get it too.)
vim.keymap.set("n", "gd", vim.lsp.buf.definition)
vim.keymap.set("n", "<leader>fn", vim.lsp.buf.workspace_symbol) -- find note by title
```

`ntropy` must be on your `PATH`. Open a note under `all-notes/`, type `[`, and the completion menu lists your notes. `gd` follows a link, and the workspace-symbol picker jumps to any note by title.
