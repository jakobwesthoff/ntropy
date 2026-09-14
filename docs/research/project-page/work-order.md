# Work order: the project page as an ntropy export

Source: the user's request that opened this research effort, 2026-09-11.
This document restates that request in structured form. Every requirement
below traces to that request; nothing has been added to it. Answers to
open questions are recorded in [decisions.md](decisions.md), not here.

## Goal

Replace the current project page, which the project-page-starter
generator builds from `README.md` and `docs/pages/`, with a page that
ntropy exports itself. In the user's words: the starter-based page "has
kind of outgrown the project and size of the documentation by now", and
ntropy "now would be capable of rendering itself a well designed project
page given we give it a well designed template for that usecase and split
up the readme into ntropy documents".

## Stages, in the order the user gave them

1. Discuss the structure of the pages.
2. Create an ntropy vault within the project and transfer the
   information from the README into pages of that vault.
3. Reduce `README.md` to "a real entrypoint with quickstart and maybe
   even mostly dev related information", referencing the documentation
   from there.
4. Discuss a custom template for the page generation and iterate on its
   design.

## Requirements

- A proper entry point "with nice looking layout as before".
- A documentation part "which is well structured".
- Everything "fitting properly the design language".
- A footer.
- An impressum, "for legal reasons".

## Constraints

- The work happens on a branch called `new-project-page`, as an
  experiment.
