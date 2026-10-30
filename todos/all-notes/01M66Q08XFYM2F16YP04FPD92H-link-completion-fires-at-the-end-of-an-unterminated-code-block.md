---
title: "Link completion fires at the end of an unterminated code block"
kind: bug
component: lsp
impact: low
origin: review
---
# Link completion fires at the end of an unterminated code block

The language server suppresses link completion inside code, but not when the cursor sits at the very end of a note that ends inside an unterminated fenced or indented code block without a trailing newline. Typing `[` there offers note links. Found by the code-masking work on 2026-10-30 and confirmed by reading the code, not reproduced in an editor.

## Problem

`in_code` in `src/link/mod.rs` asks `code::is_masked` (`src/link/code.rs`), which tests `Range::contains`. A block that runs to the end of the body has the range `start..body.len()`, so the offset `body.len()`, where the cursor is while typing at the end, is outside it. Fenced blocks had this gap before the mask moved to the Markdown parser, and indented blocks share it since they are masked too.

## Suggested fix

In `in_code` (not in `is_masked`, which link extraction also uses), treat an offset equal to `body.len()` as inside code when a masked range ends there and that range is an unterminated block. Pin it with tests in `src/bin/ntropy/run/lsp/completion/link.rs` for a fenced and an indented block, and one showing that a closed inline span ending the body does not suppress completion after it.
