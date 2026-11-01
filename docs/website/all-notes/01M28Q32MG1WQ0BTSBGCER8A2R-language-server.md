---
title: Language server
tags: [docs/integrate]
site:
  order: 1
---
`ntropy lsp` runs a
[Language Server](https://microsoft.github.io/language-server-protocol/)
over stdin and stdout, the machinery that gives an editor autocomplete
and go-to-definition. With it, any editor that speaks LSP completes links
between notes and tags, follows links, and jumps to a note by title.

## Set it up in Neovim

For a recent Neovim (0.11 or later), start the server for Markdown
buffers that live in a vault. Put this in your config:

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

-- Optional keys: follow a link, and find a note by title.
vim.keymap.set("n", "gd", vim.lsp.buf.definition)
vim.keymap.set("n", "<leader>fn", vim.lsp.buf.workspace_symbol)
```

`ntropy` must be on your `PATH`. Open a note under `all-notes/`, type
`[`, and the completion menu lists your notes. `gd` follows a link, and
the workspace-symbol picker jumps to any note by title.

The server resolves the vault for each open document with the same rules
as the CLI, so there is nothing to configure beyond pointing the editor
at the binary.

## What the server does

**Link completion.** Type `[` and pick a note; the candidates are
fuzzy-matched on title, tags, and file name. ntropy inserts the whole
`[Title](<ulid>-<slug>.md)`, so a link between notes never means
hand-copying a ULID. Typing inside a hand-written `](` completes just the
target. Editors that support snippets get the cursor placed right after
the inserted link.

**Tag completion.** Inside a note's `tags:` frontmatter, completion
offers the tags already in the vault, hierarchy-aware: typing
`programming/` narrows to its children. Both written forms work, the
single-line `[a, b]` list and the `- a` block list.

**Go to definition and document links.** Jump to, or click straight
through, a link to the note it points at.

**Workspace symbols.** Jump to any note in the vault by title.

A control character or bidi override in a title shows escaped, such as
`\u{1b}`, in the completion menu, in the inserted link and in the symbol
list. Candidates come from a scan of the saved files: a title or tag you
changed in an open buffer shows up in completion after you save.

In an encrypted vault the server decrypts in memory and works the same,
as long as the vault is unlocked. Following a link there opens a
read-only decrypted copy of the target, good for reading; editing still
goes through `ntropy search`.
