# Named templates / note types (post-v1)

Deferred from v1 per ADR 0017. v1 has a single default template with hand-rolled
`{{var}}` substitution.

## To decide later

- Multiple named templates in `.ntropy/templates/` (`meeting.md`, `journal.md`,
  …) selected via `new --template <name>`; bare `new` uses `default.md`.
- Whether a richer template engine (e.g. minijinja) is warranted then, or
  hand-rolled substitution still suffices.
