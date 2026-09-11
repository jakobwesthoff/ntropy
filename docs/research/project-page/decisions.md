# Decisions: the project page as an ntropy export

The resumption log. Each entry records the question, the answer the user
gave, the options not chosen, and the decision, in the order they were
taken. Rationale appears only where the user stated it.

## Facts established before the first question (2026-09-11)

Validated against the repository at the start of the effort:

- The current page is built by the workflow
  `.github/workflows/pages.yml` from `README.md` and `docs/pages/`
  (a `config.yaml` with hero, highlights, demo, quick-start, and footer
  sections, the README as the documentation section, an imprint with
  obfuscated email and phone, and the demo recording under
  `docs/pages/assets/`).
- `README.md` is 1207 lines with 23 top-level sections.
- A site theme is a stylesheet, fonts, and icons (ADR 0055). The three
  page templates (`base.html`, `page.html`, `note.html`) are compiled
  into the binary (ADR 0050); a theme cannot change markup or layout.
  The template override is the deferred item in
  `todos/01m26qwep48ad9qtdyxqnhc5rn-site-theme-template-override.md`.
- The front page of an export is a note rendered in the ordinary page
  layout with sidebar, breadcrumbs, and outline.
- The Markdown walk passes raw HTML blocks and inline HTML through to
  the page unchanged.

## Decided

- **Q1. Where the documentation vault lives (2026-09-11).**
  - *Answer:* `docs/website/`, a vault directory beside `adr/`,
    `design/`, and `research/`; `docs/pages/` and the starter config go
    once the switch lands. Not chosen: reuse `docs/pages/` in place; a
    `website/` directory at the repository root.
- **Q2. How the pages workflow obtains the exporter (2026-09-11).**
  - *Answer:* the latest GitHub release asset, the Linux binary of the
    newest tag. Not chosen: building ntropy from source in the workflow.
- **Q3. How fine to split the README (2026-09-11).**
  - *Answer (verbatim):* "lets talk and discuss each possible section
    once we have agreed on an overall structure, nothing is set in
    stone, as we are redoing the structure we are open to change things
    that are not ideal currently". Deferred.
- **Q4. The organizing principle of the documentation (2026-09-11).**
  Offered: A, by topic in seven sections; B, guides and reference; A
  with an added Guides section.
  - *Answer (verbatim):* "i think we might want to go by topic, but
    split it a little differently nevertheless, as we need an
    introduction page kind of the starter page with the hero the quick
    start and so on and then a link into the documentation maybe even
    reachable using crosslinks form the starter into the features, and
    there we need things then organised by topic." The exact split
    stays open (Q3).
- **Q5. Extra pages beyond the README's content (2026-09-11).**
  - *Answer:* the impressum as a note in the vault marked
    `site.hidden`, linked from the footer only. Not chosen: the
    changelog as a page; an index page of the decision records.
- **Q6. Sidebar depth (2026-09-11).**
  - *Answer (verbatim):* "unsure maybe lets reevaluate when we get
    there". Deferred.
- **The user, unprompted (2026-09-11):** the template override for site
  themes has to be implemented before the page work can start; the
  ideas and answers so far are documented first, then the template
  feature is discussed and implemented. Its questions continue the site
  export log in [../web-export/decisions.md](../web-export/decisions.md).

## Open

- The split of the README into pages and the exact sections (Q3, Q4).
- The depth of the sidebar (Q6).
- The structure of the introduction page and its cross-links into the
  documentation (Q4).
- The reduced README's content (stage 3 of the work order).
- The design of the custom template (stage 4 of the work order).
