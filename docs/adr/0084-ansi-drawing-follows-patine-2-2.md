# 84. The ansi drawing follows patine 2.2

Date: 2026-11-01

## Status

Accepted

Amends [ADR 0082](0082-ansi-render-format-drawn-by-patine.md).

## Context

patine 2.2.0 draws GitHub alerts as callouts. patine 2.1.0, which ADR 0082
builds on, drew them as quotes with the marker as text.

## Decision

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
