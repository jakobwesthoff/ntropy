---
title: "Formats disagree on a footnote defined twice"
kind: bug
component: render
status: needs-discussion
impact: low
origin: discussion
---
# Formats disagree on a footnote defined twice

A note that defines the same footnote label twice (`[^a]: first` and
later `[^a]: second`) renders differently per format: HTML and PDF use
the last definition, the terminal view (patine) shows the earlier one as
an extra loose paragraph. No format warns.

## Problem

Seen on 2026-11-01 while fixing the HTML footnote order (ntropy f9e1fa5);
the terminal behaviour comes from patine 2.1 and is documented there.

## Decision

Asked on 2026-11-01 (AskUserQuestion in the lead session of the todo
run) with the options "same definition everywhere plus a warning", "same
definition everywhere" and "later". The user chose later: keep it open.

## Open question

Which definition wins in every format, and whether ntropy warns about a
duplicate. Either change needs patine too (a patine change and releases
of patine, ntropy and asqr).
