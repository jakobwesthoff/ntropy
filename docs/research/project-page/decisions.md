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
- **Q7. The top-level sections (2026-09-11).** Offered: seven topic
  sections (Getting started; Writing notes; The vault; Finding notes;
  Publishing; Integrations; About), each a nav section with a landing
  note; fewer, broader sections; the seven plus a Reference section.
  - *Answer:* fewer, broader sections: Getting started; Notes and the
    vault; Finding notes; Publishing; Integrations and About. The pages
    of each section are discussed one section at a time (Q3).
- **Q8. The starter page's parts (2026-09-11).** Facts given first: the
  current page has a hero with the tagline and two buttons, three
  highlight cards, the demo recording, an install block with three
  tabs, and the basic commands; the README adds "Why I built this" and
  a four-command quick start linking into the topics.
  - *Answer:* all four offered parts: the hero with the tagline and two
    buttons (into the docs, and GitHub); highlight cards each linking
    to its topic page; the demo recording; install and the first
    commands.
- **Q9. The Getting started section (2026-09-11).**
  - *Answer:* a landing note holding installation and the quick start,
    and "A day with ntropy" as the one page under it. Not chosen: a
    short landing note with Installation, Quick start, and A day with
    ntropy as three pages; one page only.
- **Q10. The "Why I built this" essay (2026-09-11).**
  - *Answer:* on the starter page, a short section below the
    highlights. Not chosen: a page in Getting started; the About part;
    dropped.
- **Q11. The pages of Notes and the vault (2026-09-11).** Facts given
  first: the section takes over eight README sections, Linking (fifteen
  lines) and Finding the vault (one list) among them.
  - *Answer:* seven pages under a landing note that holds the vault
    layout and the "notes are the database" idea: Note format (with
    linking folded in), Markdown flavor, Templates and daily notes,
    Finding the vault, Materialized views, Encrypted vaults,
    Configuration. Not chosen: eight pages mirroring the README; five
    pages with one "Writing notes" page.
- **Q12. Nesting inside the section (2026-09-11).**
  - *Answer:* flat, the pages directly under the section. Not chosen:
    two sub-groups, Notes and The vault. This settles Q6 for the
    largest section.
- **Q13. The command reference table (2026-09-11).**
  - *Answer:* in Getting started, after the tour: the landing note,
    A day with ntropy, then Commands as the lookup page; Finding notes
    holds its landing note, Query language, and The interactive picker.
    Not chosen: Commands in Finding notes; Commands beside Scripting.
- **Q14. The pages of Publishing (2026-09-11).**
  - *Answer:* four pages under a landing note naming the three outputs:
    Rendering to PDF and HTML (with cross-references and `--force`),
    Document themes (Typst), Exporting a website (command, navigation,
    config, search), Site themes (tokens, fonts, icons, markup,
    templates). Not chosen: five pages with Templates apart; three
    pages.
- **Q15. The pages after Publishing (2026-09-11).** Offered: six pages
  plus the hidden impressum; the README's nine notes; five pages plus
  the impressum.
  - *Answer (verbatim):* "six pages plus hidden impressum, but i think
    license should be hidden and linked from the footer as well. the
    quesion is why are those pages part of a section at all? and maybe
    we should split into a Integration Section covering LSP, neovim,
    scripting, agents and then another section for the Limitations,
    development, design, adrs and so on?" Taken as: License is a hidden
    note linked from the footer like the impressum; two sections
    instead of one, Integrations (Language server with the Neovim
    setup, Scripting and the shell, Agent skill) and a last section for
    Limitations, Development, and Design (the decision records and
    design documents). This revises Q7 to six sections.
- **Q16. The label of the Integrations section (2026-09-11).**
  - *Answer:* Integrations. Not chosen: Integrations and about; More.
- **Q17. The last section (2026-09-11).**
  - *Answer:* "Development", two pages: a landing note with the build
    and contributing instructions, then Limitations and Design (the
    decision records and design documents). Not chosen: "About" with
    three pages; "Project" with three pages.
- **The structure as decided (Q7 to Q17), for reference:**
  - the starter page as the front page (Q8, Q10);
  - Getting started: landing note (installation, quick start), A day
    with ntropy, Commands;
  - Notes and the vault: landing note (vault layout, notes are the
    database), Note format (with linking), Markdown flavor, Templates
    and daily notes, Finding the vault, Materialized views, Encrypted
    vaults, Configuration;
  - Finding notes: landing note, Query language, The interactive
    picker;
  - Publishing: landing note, Rendering to PDF and HTML, Document
    themes, Exporting a website, Site themes;
  - Integrations: landing note, Language server, Scripting and the
    shell, Agent skill;
  - Development: landing note (build, contributing), Limitations,
    Design;
  - hidden, linked from the footer: License, Impressum.
- **Q18. The visual direction (2026-09-11).**
  - *Answer:* a distinct project-page identity: its own palette and
    type on the front page, the documentation pages restyled to match.
    Not chosen: extending the built-in theme unchanged; porting the old
    starter look.
- **Q19. Where the front page's structured content lives
  (2026-09-11).**
  - *Answer:* in the frontmatter of the index note, read by the
    template through `note.frontmatter`; the body keeps the prose. Not
    chosen: `[site.vars]`; raw HTML in the body.
- **Q20. Header and footer (2026-09-11).**
  - *Answer:* header links to the docs and to GitHub; the footer holds
    the license and the impressum with the copyright line, and links to
    GitHub and crates.io. Not chosen: a crates.io link in the header.
- **Q21. The demo (2026-09-11).**
  - *Answer:* in a terminal window frame, autoplaying muted, as the old
    page had it. Not chosen: a plain video.
- **Q22 to Q25. The first rendered export (2026-09-11).** The user
  looked at the export of the terminal teal theme and raised: the
  sidebar shows everything under a "docs" wrapper; the footer is not
  nice and should be neater and nerdier than the old one; the video is
  far too large; installation and download drown among the features and
  want a section under the why or under the video; the page is oddly
  overwide; the site name with the teal square in the header looks
  broken; and whether the impressum and license should have a sidebar
  at all. Discussed before anything changed.
  - *Q22 sidebar:* change the exporter: a rooted sidebar shows the root
    group's child groups as top-level sections whose titles link to
    their landing pages, the root's loose notes first (ADR 0056
    amendment). Not chosen: keep the wrapper and rename it.
  - *Q23 footer:* a status line, the way tmux or vim draw one: one mono
    bar with the name and version in teal, the license, the impressum,
    the old tagline as the status message, GitHub and crates.io on the
    right. Not chosen: the old footer restyled; prompt lines.
  - *Q24 site name:* `$ ntropy`, the name in mono after a muted teal
    dollar sign. Not chosen: a small wordmark with a static cursor; the
    plain name.
  - *Q25 layout:* install as its own section under the video (cargo,
    binaries, and source as three columns, the first commands below),
    the features after it; the front page and its header capped at
    56rem, the documentation pages keeping 84rem. Not chosen: install
    under the why; trimming the documentation layout to 78rem.
  - Proposed and not objected to: the impressum and the license render
    through a `plain` template of the theme, header, narrow column, and
    footer, without sidebar, outline, breadcrumbs, or pager.
- **Q26 to Q29. The second rendered export (2026-09-11).** The user
  found the status-line footer not nice, the three-column install
  section unclear and too like the section below, and the highlight
  strips too much content and not enough advertisement, the old page
  having been clearer; an artifact with variants for each followed.
  - *Q26 install:* the one-liner (`cargo install ntropy` at hero size
    with a copy button), then the binaries as small badge-like buttons
    with the Apple and Linux marks pointing at the latest release, then
    a plain "build from source" link; no Windows button, since ntropy
    has no Windows build. Not chosen: a terminal with tabs; install as
    a note; three choice buttons; the first five minutes as a sequence.
  - *Q27 pitch:* the old page's three highlights as they were, centered,
    no boxes. Not chosen: boxes; manifesto lines; a field of commands;
    folders versus metadata; an icon grid.
  - *Q28 footer:* the old page's two lines, the tagline and "Made with
    ♥ by", with the links, centered. Not chosen: the prompt-line
    variant.
  - *Q29 where the front page lives (the user, verbatim): "maybe this
    page should be build as html in the document, because using this
    template and frontmatter mixture makes it really hard to control
    and understand".* Answer: everything in the template: `front.html`
    holds the page as literal HTML, the index note only selects it.
    Not chosen: HTML in the template with the essay in the note; raw
    HTML in the note body; the frontmatter approach.
  - Asked for along the way and done: the hero's buttons with the
    download and GitHub marks as on the old page; the sections in the
    order hero, highlights, recording, install, why; the section
    dividers and the header's divider running the full viewport while
    the content stays capped; every other section on the surface tone,
    as the hero band is; larger section headings; the why centered like
    the rest; the header keeping its side gutter on a phone.
- **The user, unprompted (2026-09-11):** a GitHub icon for the theme.
- **The user, unprompted (2026-09-11):** the `just` recipe that exports
  the vault is `website`; the old page source under `docs/pages/` stays
  as a reference until the redesign is finished and is removed then,
  with everything related to it.
- **The user, unprompted (2026-09-11):** when the README is split, the
  pages' contents are audited and validated, and rewritten where they
  do not follow the writing rules (verbatim: "audit and validate their
  contents and make sure they adhere to unslop skill rules and your
  best writing practices. if they dont rewrite them").
- **The user, unprompted (2026-09-11):** the template override for site
  themes has to be implemented before the page work can start; the
  ideas and answers so far are documented first, then the template
  feature is discussed and implemented. Its questions continue the site
  export log in [../web-export/decisions.md](../web-export/decisions.md).

## Open

- The reduced README's content (stage 3 of the work order).
- The design of the custom template (stage 4 of the work order).
