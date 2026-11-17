Part of the ntropy skill. Read SKILL.md first, which holds the rules for every task; `ntropy skill` prints it.

# Querying and scripting

`ntropy search` (alias `list`) is the single browse / filter / full-text /
open entry point. Filtering and full-text search are one mechanism: a query
expression evaluated in a single pass over `all-notes/`. `delete` takes the
same selector.

The selector is optional and read from all trailing arguments, each on its
own:

- **omitted** — all notes
- **a full 26-character ULID** — exactly that note
- **a note's filename or path** — exactly that note: one argument ending in
  `.md` or `.age` that names an existing file. A bare filename is looked up in
  `all-notes/`, any other path is taken as given, and a symlink to a note, such
  as an entry of a view directory, selects the note it points to. The path of
  a copy that shares another note's ULID selects the copy alone. A file that is
  no note of the vault is an error, and so is the filename of a note whose file
  no longer exists (as a query it would match the notes linking to it), so a
  stale name fails. Any other word ending in `.md` is a query.
- **anything else** — a query expression

## The query language

Combine predicates with `and`, `or`, `not`; precedence is `not` > `and` > `or`;
parentheses override. Two predicates side by side are an `and` that binds like
the written one (`a b or c` is `(a and b) or c`). A word that is not an
operator is a search word, so a mistyped operator (`tag:work nd status:done`)
or a doubled one (`a and and b`) searches bodies for that word instead of
failing. Values are bare words (Unicode letters, digits, `/`, `_`,
`-`, `.`) or double-quoted strings for anything with spaces, other punctuation
(`tag:"c++"`) or regex metacharacters. `tag:milestone/0.13` needs no quotes.
A bare search word matches literally, its dot included (`node.js` does not
match `nodexjs`); a quoted string is a regex (`"node.js"` does).

```bash
ntropy search -n tag:work and not status:done
ntropy search -n 'status:"in progress" or tag:urgent'
ntropy search -n '(tag:work or tag:side-project) and not status:done'
ntropy search -n borrow checker            # both words in the body, any order
ntropy search -n "borrow checker"          # the phrase, words in that order
```

Quote the whole query for the shell whenever it contains `(`, `)`, or double
quotes. An argument with several words and no query syntax (no `:`,
parenthesis or `"`, and no `and`, `or`, `not` in any case) is a phrase: its
words in order with any whitespace between them, a line break included, every
character literal. `'tag:work and not status:done'` in one argument stays a
query, and next to other arguments a whole-query argument is grouped, so
`search 'a or b' c` is `(a or b) and c`. A phrase that contains query syntax is
written with double quotes inside single quotes: `'"page not found"'`. Blank
arguments are ignored.

### Predicate semantics

- **`tag:Q`** — segment sub-path match, case-insensitive. `Q` and the note's
  tags are split on `/`; `Q` matches when its segments appear as a contiguous
  run inside a tag's segments. `tag:programming` matches `programming`,
  `programming/rust`, and `area/programming`; `tag:programming/rust` matches
  any tag containing that consecutive chain.
- **`field:value`** — frontmatter equality for scalars, membership for list
  fields. The value is exact and case-sensitive (`status:Done` does not match
  `status: done`); the field name ignores letter case (`Status:done` finds a
  `status` key). A number compares as ntropy reads it (`x: 2.0` is `x:2.0`,
  `x: 1.50` is `x:1.5`). A note with keys that differ only in case (`Status` and
  `status`) matches when any of them does. `tag` and `text` are reserved in
  any letter case, so a frontmatter field with such a name cannot be queried.
  Quote multi-word values: `status:"in progress"`.
- **`text:pattern`** (and bare words / bare quoted phrases) — a regex over the
  note body, smart-case: all-lowercase patterns match case-insensitively, one
  literal uppercase character makes the match case-sensitive. An invalid regex
  is a query error.

There are no comparison operators (`>`, `<`, date ranges) in the language.

## Non-interactive output

With `-n` (the core's first rule) the plain table is one note per line, newest
first, space-aligned columns padded to the widest cell, with an uppercase
header row:

```
ID                          DATE        TITLE                TAGS                   PATH
01KWVBW61WHJY7K27WNETSF641  2026-07-06  Refactor the parser  work,programming/rust  /…/all-notes/01KWVBW61WHJY7K27WNETSF641-refactor-the-parser.md
```

`tail -n +2` drops the header. Columns are separated by runs of two or more
spaces (the last column is unpadded), so fields that themselves contain single
spaces stay parseable:

```bash
# All matching IDs (first column is a fixed-width 26-char ULID):
ntropy search -n tag:work | tail -n +2 | awk '{print $1}'
```

Without parsing, `--id` prints one ULID per line, newest first, and nothing
else. A note and its copies share one ULID, which prints once:

```bash
# All matching IDs:
ntropy search -n --id tag:work
```

For file paths, `--print`/`-p` prints one path per line and nothing else, so
no parsing is needed either:

```bash
# The file path of one note by ULID:
note=$(ntropy search -n -p 01KWVBW61WHJY7K27WNETSF641)

# Every matching path, newest first:
ntropy search -n -p tag:work
```

`tags` and `view list` print the same style of headed, space-aligned table.

## Exit codes and stderr

`search` exits 1 in three situations that differ only on stderr:

| Situation | stderr | stdout |
|-----------|--------|--------|
| nothing matched, an empty vault included | `No notes matched your search criteria.` | nothing |
| an error: an unresolvable vault, an invalid query or regex, an ambiguous `-P` | a line that starts `error:` | nothing |
| `--strict` and a warning, such as a malformed note | `warning: ...` lines | the notes that matched still print |

Matches exit 0 without `--strict`. Judge a search by its stderr as well as its
exit code, or an error reads as "nothing matched":

```bash
err=$(mktemp)
if ids=$(ntropy search -n --id tag:urgent 2>"$err"); then
  echo "found: $ids"
elif grep -q '^No notes matched' "$err"; then
  echo "nothing is urgent"
else
  cat "$err" >&2     # error: or a --strict warning
fi
```

## Deleting

```bash
ntropy delete -n -f 01KWVBW61WHJY7K27WNETSF641
```

`delete` must resolve to **exactly one** note: an ambiguous query fails with
the candidate list and a non-zero exit, and nothing is deleted. A ULID that a
copy of the note shares is ambiguous too: run `ntropy reconcile`, which gives
the copy its own ULID, or select the note by its filename or path. `delete` does
not see a malformed note (it answers `no note matches`); `write` it into shape
first. `delete` refuses an empty selector with `error: the selector is empty`,
with no lookup and no picker, so an unset shell variable
(`delete -n -f "$ID"`) cannot delete a note.

Deleting removes the canonical file and refreshes the views.

End of references/querying.md.
