# 84. The ansi drawing follows patine 2.2

Date: 2026-11-01

## Status

Accepted

Amends [ADR 0082](0082-ansi-render-format-drawn-by-patine.md).

## Context

ADR 0082 has ntropy write the escape sequences from patine's styled lines
with a copy of patine's writer. patine 2.2.0 writes styled lines itself
(`patine::ansi`), clears the colours of a style (`TextStyle::without_colors`)
and draws GitHub alerts as callouts, which patine 2.1.0 drew as quotes with
the marker as text.

## Decision

ntropy writes the lines with `patine::ansi` and keeps no writer of its own.
Before that, each span's text is escaped as ADR 0082 describes, and each
span's style is mapped for the form: kept with every style, passed through
`TextStyle::without_colors` without the colours, replaced by the default
style for text alone. A style attribute patine adds to `TextStyle` reaches
the output without a change in ntropy.

A quote whose first line is a marker (`[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`,
`[!WARNING]`, `[!CAUTION]`, or another type) is drawn as patine draws a
callout: a heavy bar `┃` on every line and a bold title after a symbol, in
the kind's colour. A type beyond the five alerts is a neutral callout with
`○` and no colour. The look is patine's. Per form:

- with every style: patine's default colours;
- with every style but the colours: dim bars and a bold title, patine's
  `CalloutStyle::DimBar`, set beside `QuoteStyle::DimBar` because patine does
  not derive the callout style from the quote style;
- as text alone: the bar, the symbol and the title as text.
