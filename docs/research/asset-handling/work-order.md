# Work order

Requested 2026-11-17.

## Goal

Find a clean way to integrate assets into ntropy. Assets are the extra files
and images that belong to a note. Today they are put by hand into a directory
referenced by ULID and title, and nothing reconciles them. The user is not
sure this is the best way.

## Starting assumptions of the user

- Nothing reconciles the asset directory with its note.
- It is unclear whether HTML output, single or multi document, carries the
  assets at all.
- Images do render in PDFs.
- Assets are not encrypted.

[current-state.md](current-state.md) checks each of these.

## Principle stated by the user

Stated 2026-11-17 after the research was written, as an opinion to discuss
and not yet a decision: the idea of all ntropy structure is that it is human
readable and navigable without ntropy. A folder named only by a ULID makes
that much harder, because no correlation between folder and note can be
established any more. The user does not like the ULID-only folder approach
the researchers converged on. [options.md](options.md#readable-folder-names)
treats the readable variants in full and
[open-questions.md](open-questions.md) leads with the naming question.

## What the user is open to

Either the current way, integrated into ntropy, or something completely
different. No option is excluded in advance.

## What was asked

1. Work out the options for integrating assets cleanly.
2. Write a research document (this directory).
3. Then discuss with the user how to tackle it. That discussion has not
   happened yet; it starts from [open-questions.md](open-questions.md).

## Method

- Two researchers on different models, opus and fable, each investigated the
  code and ran a lab on the debug build of `ed914a3`. The labs used scratch
  vaults only, with a pinned `--vault` and configuration directory.
- Each wrote an independent proposal: facts, outside view, options,
  recommendation.
- Two critique rounds. In round 1 each attacked the other's proposal and
  tested its claims in the lab. In round 2 each answered the critique of
  itself and ended with a table of remaining differences, marked technical or
  user decision.
- Lab verification: claims that could be run were run, for example the
  reconcile incident, the PDF failure modes, the encrypted-vault behaviour,
  the link forms that the exporters accept, and the `parse_name` hazard.
- This record was then written from that material. Where the two disagreed on
  a fact, the code decides, and the outcome is stated in the text.

## Outside the scope

Implementation. The ADR that records the final layout is written after the
decision with the user.
