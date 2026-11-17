# Research: asset handling in ntropy

Research on how ntropy should store, name, check, render and encrypt the files
that belong to a note (images, PDFs, transcripts, archives). Started
2026-11-17. The todo that carries the matter is
[Give each note an asset folder that ntropy manages](../../../todos/all-notes/01M6E0CWM5KS1FRX3DTB3VREB7-give-each-note-an-asset-folder-that-ntropy-manages.md).

The facts about the code are checked against `main` at commit `ed914a3`
(release v2.11.0).

## Files

| File | Purpose |
| :--- | :--- |
| [work-order.md](work-order.md) | The request, structured: goal, what is open, what was asked, the method. |
| [current-state.md](current-state.md) | What ntropy does with assets today: the user's assumptions checked, each output, each subsystem, each placement of a file, the conventions in use and the limits found. |
| [external.md](external.md) | How other note tools handle attachments, with what was and was not re-fetched. |
| [options.md](options.md) | Every option considered, how each fares, why each was kept or rejected, the recommendation both researchers converged on, the technically settled points and the withdrawn ideas. |
| [open-questions.md](open-questions.md) | The questions only the user can decide, and the measurements. |
| [decisions.md](decisions.md) | The user's answers to the open questions, round by round. |

## State on 2026-11-17

The research is done. The decisions are taken with the user, working
through [open-questions.md](open-questions.md), and recorded in
[decisions.md](decisions.md). Rounds 1 to 4 (2026-11-17) decided the
design except asset encryption, which waits until the layout has been
used for a while. Where a decision differs from a recommendation or a
"settled" point in [options.md](options.md), decisions.md wins: the asset
folder is `_assets`, named like the note file, and there is no conversion
command.

Both researchers recommend one family of layout: a folder per note named by
the note's bare ULID, `all-notes/assets/<ULID>/`, linked with plain relative
Markdown, ownership read from the path, built in phases (bug fixes first,
then layout and commands, then encryption). The technical agreement holds
for option B. The readable variants open technical points listed in
options.md. The Typst staging root for encrypted renders is unprototyped by
both. What is left besides those is taste and risk acceptance. The
measurement of shared assets in the `primary` vault is done and found none,
which removes the case for first-class asset objects there.

The user then stated a principle that conflicts with the bare-ULID folder:
ntropy's structure should be readable and navigable without ntropy. The
naming question therefore leads [open-questions.md](open-questions.md), and
[options.md](options.md#readable-folder-names) treats the readable variants
in full.

Three bugs found on the way are filed as todos and are independent of the
decision: [parse_name](../../../todos/all-notes/01M7MNSS8YXMWC461EWYT9HFFD-a-link-into-a-folder-named-like-a-note-is-read-as-a-link-to-that-note.md),
[a missing file gets a dead link in HTML](../../../todos/all-notes/01M7MNV5C1Y9AMQXP0TJ0T7E7V-html-and-site-exports-point-a-missing-file-at-a-dead-path-in-the-files.md)
and [percent-encoded paths](../../../todos/all-notes/01M7MP0N1Y3Z331VHFKSVRJMXQ-a-percent-encoded-local-file-path-is-not-decoded-in-html-and-pdf-output.md).

## How to resume

1. Read [work-order.md](work-order.md) for the goal.
2. Read [current-state.md](current-state.md), starting with its table of the
   user's assumptions.
3. Read the recommendation and the settled points at the end of
   [options.md](options.md).
4. Take the questions in [open-questions.md](open-questions.md) with the
   user. Record each answer in the todo above, then write the ADR.

## Evidence

Code facts carry a `file:line` at the commit above, an ADR or a design doc.
Lab facts carry the command and its output as the researchers recorded them.
The lab directories (`opus-lab`, `fable-lab` and the document review's
`review-F-lab` and `review-O-lab`) live outside the
repository, in the working directory of the session that produced this
research, and are not kept here. A fact that rests only on a lab run says so.
Where a lab fact is cheap to repeat, the command is given.

Something marked "from memory" was written by a researcher without fetching
the source and was not re-fetched when this record was written.
