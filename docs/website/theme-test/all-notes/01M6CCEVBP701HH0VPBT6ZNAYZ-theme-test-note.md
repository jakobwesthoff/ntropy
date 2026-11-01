---
title: Theme test note
tags: [theme-test, theme-test/reference]
author: Jane Doe
status: draft
version: 3
ratio: 1.5
reviewed: false
due: 2026-12-01
aliases: [Specimen, Every construct]
reviewer:
see-also: []
angle: !degrees 90
contacts:
  - name: Ana
    roles: [writer, editor]
owner:
  team: Docs
  contact: docs@example.org
---
This note holds what a note can contain that both a PDF and a web page
draw, so a theme shows how it draws each part. Render it with your theme
and read it from top to bottom. Raw HTML and images from the web appear
only on web pages; the note "Raw HTML and remote images" of the theme test
vault holds them.

# Heading level 1

A paragraph of running text with **strong**, *emphasis*, ~~struck~~ and
`inline code`, followed by ***strong emphasis*** and **strong with
`code` inside**. Markup right before a parenthesis: **strong**(one),
*emphasis*(two), `code`(three) and [a link](https://example.org)(four). A
line ends here with a hard break  
and the next line continues the paragraph.

## Heading level 2

### Heading level 3

#### Heading level 4

##### Heading level 5

###### Heading level 6

### A heading with `code` and *emphasis*

## Links

An [external link](https://example.org/docs), a bare URL
https://example.org/path, a bare host www.example.org and a mail address
<docs@example.org>. A link to another note shows its title:
[the companion](01M6CCEVCDRQ5504MDH503KQT2-theme-test-companion.md). A
link with a heading anchor shows its own text:
[the linked section](01M6CCEVCDRQ5504MDH503KQT2-theme-test-companion.md#linked-section).
A link to a heading of this note: [back to the lists](#lists).
A [**bold** label](https://example.org) and
[the wide image as a file](../assets/theme-test/landscape.svg).

## Lists

- An unordered item
- An item with a nested list
  - A second level
    - A third level
- A last item

1. An ordered item
2. An item with a nested unordered list
   - Nested inside an ordered list
3. A third item

A paragraph ends the first ordered list.

7. A list that starts at seven
8. keeps its numbers

A task list:

- [x] A task that is done
- [ ] A task that is open

Steps with blocks inside:

1. Run the export:

   ```bash
   ntropy site -o out
   ```

2. Read the note:

   > Quoted inside an item.

A loose list, with space between its items:

- A loose list item

  with a second paragraph.

- Another loose item

## Quotes and callouts

> A block quote with **strong** text.
>
> A second paragraph in the same quote.
>
> > A nested quote.
>
> ### A heading inside a quote
>
> > [!WARNING]
> > A callout inside a quote.
>
> | Key | Value |
> | --- | --- |
> | a | b |

> [!NOTE]
> A note callout, with `code` inside.

> [!TIP]
> A tip callout with a table:
>
> | Key | Value |
> | --- | --- |
> | a | b |

> [!IMPORTANT]
> An important callout.

> [!WARNING]
> A warning callout.

> [!CAUTION]
> A caution callout.

> [!NOTE]
> #### A heading inside a callout
>
> A callout with a heading and a list:
>
> - one
> - two

> [!SUMMARY]
> A quote whose marker is not one of the five kinds stays a plain quote
> in PDF and HTML.

## Tables

| Left | Center | Right |
| :--- | :----: | ----: |
| apples | *ripe* | 12 |
| pears | `code` | 3.50 |
| a longer cell that wraps in a narrow column | x | 100 |

| Term | Meaning |
| --- | --- |
| `render.theme` | The theme a render uses |
| Long word | Donaudampfschifffahrtsgesellschaftskapitänsmützenabzeichen |

| ID | Name | Region | Owner | Created | Updated | Status | Priority | Estimate | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1042 | Release pipeline | eu-central | Docs | 2026-10-01 | 2026-11-01 | open | high | 5 days | ten columns |
| 1043 | Theme review | us-east | Web | 2026-10-02 | 2026-10-30 | done | low | 1 day | more than a column holds |

## Code

```rust
fn main() {
    let greeting = "Hello, theme";
    println!("{greeting}");
}
```

```python
def greet(name: str) -> str:
    return f"Hello, {name}"
```

```toml
[render]
theme = "handout"
```

```bash
ntropy render --force -o /tmp/theme-test.pdf 'title:"Theme test note"'
```

```json
{"name": "theme-test", "constructs": 42, "complete": true}
```

```
A code block without a language stays plain.
```

```rust
// A long line that a theme has to wrap or scroll: let configuration = load_configuration_from_the_environment_and_the_vault_and_the_command_line(arguments);
```

    An indented code block.

## Images

![A wide image, scaled to the column](../assets/theme-test/landscape.svg)

A small image without alt text:

![](../assets/theme-test/mark.svg)

A linked image:

[![Linked mark](../assets/theme-test/mark.svg)](https://example.org)

## Footnotes

A sentence with a footnote.[^first] A second footnote holds more than one
paragraph.[^second] A third sentence refers to the first footnote again.[^first]

[^first]: The first footnote.

[^second]: The second footnote, with `code`.

    And a second paragraph.

## Wrapping

A long word: Donaudampfschifffahrtsgesellschaftskapitänsmützenabzeichen.
A long URL: https://example.org/a/very/long/path/that/does/not/break/at/any/space/at/all/index.html.
A long line of code: `configuration.load_from_environment().merge_with_vault_settings().apply()`.
A link with a long label:
[https://example.org/a/very/long/path/that/does/not/break/at/any/space/at/all/index.html](https://example.org/a).

- A list item with a long address
  https://example.org/a/very/long/path/that/does/not/break/at/any/space/at/all/index.html
  and `configuration.load_from_environment().merge_with_vault_settings().apply()`.

> [!NOTE]
> A callout with a long address
> https://example.org/a/very/long/path/that/does/not/break/at/any/space/at/all/index.html.

| Key | Value |
| --- | --- |
| A long address | https://example.org/a/very/long/path/that/does/not/break/at/any/space/at/all/index.html |

### Donaudampfschifffahrtsgesellschaftskapitänsmützenabzeichen as a heading

A footnote with a long address.[^wrapping]

[^wrapping]: See https://example.org/a/very/long/path/that/does/not/break/at/any/space/at/all/index.html.

---

The horizontal rule above separates the last paragraph.
