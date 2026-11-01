---
title: "A link to a local file with a fragment is not resolved in site and html exports"
kind: bug
component: site
origin: review
---
# A link to a local file with a fragment is not resolved in site and html exports

A link to a file in the vault with a fragment, such as
`[the spec](files/spec.pdf#page=2)`, is not resolved by `ntropy site` and
`render --to html`: the fragment is taken as part of the file name, the
export warns that the referenced file is not in the vault, and
`--strict` fails. Found on 2026-11-01 in the review of the plan for
resolving links to headings in other notes, which had the same cause for
note links; not reproduced on its own yet.

## Expected

The file is copied and linked as without the fragment, and the link keeps
the fragment (`.../spec.pdf#page=2`).
