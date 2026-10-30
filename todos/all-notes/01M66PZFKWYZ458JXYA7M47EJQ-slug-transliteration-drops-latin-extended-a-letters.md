---
title: "Slug transliteration drops Latin Extended-A letters"
kind: improvement
component: note
impact: low
origin: review
---
# Slug transliteration drops Latin Extended-A letters

The transliteration table behind every slug (`transliterate` in `src/text/slug.rs`, ADR 0023) maps German umlauts and the common Western European accents, but not letters such as ł, ń, ő, ş, ğ, č, š or ž. Those letters are dropped, so a note titled "Gdańsk" gets the slug `gdask` and "Wrocław" gets `wrocaw`. Found by the plan review of the tags-keep-punctuation work on 2026-10-30, read in the table (lines 84 to 111 at the time), not run.

## Problem

Unmapped non-ASCII characters fall to the `_ => String::new()` arm. Polish, Czech, Hungarian, Turkish and other Latin-script titles lose letters in their filenames, view names and site page paths.

## Decisions

- 2026-10-30, lead: while the tags work changes slugs to replace ASCII punctuation with `-`, unmapped non-ASCII stays dropped rather than becoming `-`, because a `-` would split words (`gda-sk`) and rename those notes a second time once the table grows.

## Suggested fix

Extend the table with the Latin Extended-A letters, or transliterate through Unicode decomposition (NFD, then drop combining marks) plus a short table for letters that do not decompose (ł, đ, ø, ß, æ, œ). Either way the slug of affected titles changes, so `reconcile` renames those notes once; say so in the changelog.

## Done when

- `Gdańsk`, `Wrocław`, `Łódź`, `Brno – Žabovřesky` and `İstanbul` slug to readable ASCII, pinned by tests in `src/text/slug.rs`.
- ADR 0023 (or a new ADR amending it) states the coverage.
