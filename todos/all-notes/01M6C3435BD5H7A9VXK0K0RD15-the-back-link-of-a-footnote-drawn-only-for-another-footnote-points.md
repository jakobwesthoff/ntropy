---
title: "The back link of a footnote drawn only for another footnote points nowhere"
kind: bug
component: render
origin: review
---
# The back link of a footnote drawn only for another footnote points nowhere

In `site` and `render --to html`, each footnote ends with a "↩" link back
to its reference (`#fnref-N`). A footnote that is drawn only because
another footnote definition refers to it, while that definition itself is
referenced nowhere in the body, gets a "↩" to an `fnref-N` id that is not
on the page, so the link does nothing. Found on 2026-11-01 while making
heading ids unique (ntropy 03cac17); the behaviour is older than that and
sits in the HTML emitter's `finish()` (`src/render/html/emitter.rs`). Not
reproduced on its own yet.

## Expected

Every "↩" points at an id that exists on the page. A footnote whose
reference is not drawn either gets no back link, or links to the
reference that is drawn (the plan decides which, with the reason).
