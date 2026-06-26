# 30. Replace the ripgrep stack with the regex crate for full-text search

Date: 2026-06-26

## Status

Accepted

Supersedes [ADR 0011](0011-embed-ripgrep-libraries-for-full-text-search.md)
and the `grep-searcher`/`grep-regex` entry of
[ADR 0024](0024-v1-dependency-selection.md).

## Context

[ADR 0011](0011-embed-ripgrep-libraries-for-full-text-search.md) embedded
`grep-searcher` and `grep-regex` to make full-text "ripgrep-fast". The
implementation realizes none of ripgrep's performance wins:

- Text predicates run against each note's already-parsed in-memory `body`
  (`note::Note::body`), via `Searcher::search_slice` over a `&str`. The body is
  resident because the scan parses every note for its frontmatter regardless, so
  the searcher's file-oriented machinery (memory-mapped and streamed reads)
  never engages.
- At a note body's kilobyte scale the searcher's literal prefilters save
  nothing, and the `regex` crate already ships the same prefilter machinery for
  an in-memory `is_match`.

`regex` is already a direct dependency (link extraction). Against an in-memory
`is_match` it is on par with the grep stack while dropping two crates whose value
is unrealized here.

## Decision

Drop `grep-searcher` and `grep-regex`. Compile each `text:` predicate (and the
bare-term shorthand) to a `regex::Regex` and evaluate it with `Regex::is_match`
against the note body.

Preserve the two observable behaviors of the prior matcher:

- **Smart case.** Case-insensitive unless the pattern contains a *literal*
  uppercase character. `\W`, `\pL`, and bracket-class shorthands carry no
  literal, so they do not force case sensitivity. This is computed from the
  pattern's AST via `regex-syntax`, mirroring the grep matcher's rule.
- **Line anchors.** `multi_line` is enabled so `^` and `$` match at line
  boundaries, and `.` does not cross a newline, matching the prior
  line-oriented behavior for the predicates v1 supports.

`ignore`, the directory walker ([ADR 0024](0024-v1-dependency-selection.md)), is
unaffected: it is not part of text matching.

## Consequences

- Two fewer direct dependencies; `regex-syntax` is added but already compiled
  transitively under `regex`.
- One residual difference from line-oriented searching: a pattern that
  explicitly spans a newline (a literal `\n`, or `(?s).`) can now match across
  lines. No v1 predicate constructs such a pattern.
- Smart-case detection is now owned code and must be tested (ADR 0021),
  including the literal-versus-class distinction.
