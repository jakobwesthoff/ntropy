---
title: "Let vaults set default values for fields that notes lack"
kind: feature
component: vault
---
# Let vaults set default values for fields that notes lack

A field that most notes share one value of is cheapest to record only on the
exceptions. A vault could declare a default per field, so that a note without
the field behaves as if it had the default value, in queries and views. The
case that prompted this: a vault marks its German notes with `lang: de` and
leaves English notes unmarked. English is the default writing language, so
stamping `lang: en` on every note adds nothing but noise.

## Goal

ntropy has no notion of a default today, so the implied value is invisible:

- `field:value` never matches a note without that field. `field_matches`
  returns `false` for a missing key (`src/query/eval.rs`, the `None => false`
  arm), pinned by the `missing_field_does_not_match` test. `lang:en`
  therefore finds only the notes that spell it out.
- Views skip a note with no value for the grouped field
  (`src/view/materialize.rs`, module doc and `missing_field_yields_no_groups`).
  A `by-lang` view would have a `de/` group and no group for the unmarked
  majority.

The workaround is negation: `not lang:de` selects the unmarked notes, since
`not` over a missing field is true. It works for one exception value, grows
into `not lang:de and not lang:nl and ...` with more, and still gives views
nothing to group.

## Proposal

A per-vault table in `.ntropy/config.toml` names a default value per field:

```toml
[defaults]
lang = "en"
```

A note without `lang` then behaves as if it had `lang: en`:

- `lang:en` matches it.
- A view grouping by `lang` puts it in the `en` group.
- A note that sets the field keeps its own value. A default never overrides
  an explicit one.

`PerVaultConfig` (`src/config/per_vault.rs`) holds the `[[view]]`, `[render]`
and `[site]` tables today. A `defaults` map would sit beside them, omitted on
write when empty, like the other optional sections.

## Open questions

- Where the default applies. Resolving it once when a note is parsed is the
  smallest change, since every consumer of `Note::frontmatter`
  (`src/note/mod.rs`) would see it. It must then never be written back to the
  file, or a default turns into a stamped value on the next rewrite. Applying
  it only in query evaluation and view grouping keeps files and rendered
  output untouched, but every new consumer has to remember it.
- Rendered output. Should a defaulted value appear in the frontmatter that
  `render` and `site` show for a note, or only real values?
- Site search. ADR 0052 has the TypeScript evaluator in `site/src/search/`
  mirror the CLI's `field:` semantics, checked by the shared corpus
  `tests/fixtures/query-corpus.json`. The defaults have to reach the embedded
  note data or the evaluator, and the corpus needs cases for them.
- Value shapes. Scalars only, or lists too? A list default would act as
  membership, like an explicit list field does.
- `tags`. Is a default tag for untagged notes in scope, or is `tags` excluded,
  given its own normalization and hierarchy rules (ADRs 0006, 0023)?
- Reaching the unmarked notes deliberately. With a default in place, `lang:en`
  no longer tells explicit and implied values apart. The missing-field query
  predicate answers that question, and it should see the raw frontmatter, not
  the defaulted view.
