---
title: Kitchen Sink
tags:
  - area/work
  - demo
priority: 2
ratio: 1.5
draft: false
reviewer: null
review:
  due: 2026-08-01
  by: jakob
aliases: []
angle: !degrees 90
---

Intro with *emphasis*, **strong**, ~~struck~~, `let x = "a\\b";`, and a
bare URL https://example.com/path plus www.example.org and mail to
<jaw@ekko.io> too. Literal math $x^2$ stays text. Unicode: Über Größe
日本語.

A resolved note link to [old title](01BRZ3NDEKTSV4RRFFQ69G5FAV-linked.md)
and a dangling one to [**bold** ghost](01CRZ3NDEKTSV4RRFFQ69G5FAV-gone.md).

An [external link](https://example.org/docs), a [relative file](notes/other.md),
an [anchor](#table), and a [mail link](mailto:jaw@ekko.io).

> [!NOTE]
> A note callout with a `code chip` inside.

> [!TIP]
> Tips look encouraging.

> [!IMPORTANT]
> Important stands out.

> [!WARNING]
> Careful[^fn] with content.

> [!CAUTION]
> Caution is loud.

> A plain block quote with a second paragraph.
>
> The second paragraph.

[^fn]: A footnote with `code` inside and a list:
    - one
    - two

Another reference to the same footnote[^fn], one used before its
definition[^early], and an undefined one[^ghost].

[^early]: Defined after use.

## Table

| Left | Center | Right |
| :--- | :----: | ----: |
| a    | *b*    | c     |
| short row |

## Lists

- bullet
- nested parent
  - child level two
    - child level three

6. sixth
7. seventh

- [x] done task
- [ ] open task

A loose list:

- first item

  with a continuation paragraph

- second item

## Code

````rust ignore
fn demo() {
    println!("a ``` fence inside");
}
````

    indented code block

## Media and HTML

![A local diagram](diagram.png)

![A remote image](https://example.com/remote.png)

<div class="raw">block HTML is dropped with a warning</div>

Inline <br> HTML too.

---

The end.
