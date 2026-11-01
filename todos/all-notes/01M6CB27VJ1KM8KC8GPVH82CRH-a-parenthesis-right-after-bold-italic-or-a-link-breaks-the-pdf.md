---
title: "A parenthesis right after bold, italic or a link breaks the PDF"
kind: bug
component: render
impact: high
origin: review
---
# A parenthesis right after bold, italic or a link breaks the PDF

`ntropy render` to `pdf` fails when a `(` follows inline markup without a
space. For the body

```markdown
See **bold**(x) and *em*(y) and [link](https://example.org)(z).
```

the Typst output is

```
See #strong[bold](x) and #emph[em](y) and #link("https://example.org")[link](z)\.
```

and typst stops with `error: expected function, found content`, so no
PDF is written (exit 1). Typst reads a `(` right after `#func[...]` as
more arguments to the call. Text like `**Note**(s)` or a link followed by
a parenthesis is common. Found on 2026-11-01 in the review of the Typst
footnote label fix (which ends its own calls with `;` for the same reason)
and reproduced by the lead with v2.9.0 in a scratch vault.

## Problem

The Typst emitter (`src/render/typst/emitter.rs`) writes `#strong[...]`,
`#emph[...]`, `#strike[...]` and `#link(...)[...]` with nothing that ends
the call, so the text that follows can continue it.

## Suggested fix

End every inline function call the emitter writes with `;` when the next
character could continue it (or always), as the footnote fix does, and
test each construct followed by `(` and by `[`.
