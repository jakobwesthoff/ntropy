---
title: "Untrusted note fields are printed raw to the terminal"
kind: bug
component: cli
origin: review
tags: [security]
---
# Untrusted note fields are printed raw to the terminal

Found in the 2026-07-02 security review of terminal output. Note titles,
filenames, path components, template names, view names and fields, and body
link targets come from a vault the user may not have written. They reach
stdout, stderr and the raw-mode picker byte for byte, with no control
character filtering. A raw ESC byte (0x1B), including `\x1b` written in a
double-quoted YAML string, can emit terminal control sequences: title bar
rewrites, screen clears, cursor moves and, on permissive emulators, OSC 52
clipboard writes. Embedded `\n` and `\t` also break the table layout and the
picker layout on every output. `ls`, `git` and ripgrep all sanitize or quote
control bytes in untrusted names, and ntropy should do the same.

## Problem

Confirmed by code reading during the 2026-07-02 security review, which checked
both the non-interactive output and the interactive picker. A follow-up pass
checked every output surface, corrected the scope (tags are safe, while the
LSP and reconcile link targets are extra vectors) and drafted the sanitization
design below.

### How untrusted bytes get in

- Titles are taken verbatim from YAML. `parse_block` in
  `src/note/frontmatter.rs` keeps the `title` string after a `trim().is_empty()`
  check and nothing else. Control bytes in plain or double-quoted scalars reach
  `Note.title` in `src/note/mod.rs`. Unsafe.
- Filenames and path components come from disk through `to_string_lossy` and
  `Path::display`. ESC is valid UTF-8, so it passes unchanged, and only invalid
  bytes become U+FFFD. Unsafe.
- Body link targets pass through the link regex in `src/link/mod.rs`. That
  regex forbids only `)` and Unicode whitespace in the target. ESC is not
  `\p{White_Space}`, so `[x](01ARZ3NDEKTSV4RRFFQ69G5FAV-<ESC>evil.md)` passes
  both the regex and `parse_target`, which checks only the 26-character ULID
  prefix and the `.md` suffix. The `from` value of a reconcile relink is
  therefore a real escape carrier. Unsafe.
- Template names, and the view name and field, come from file stems on disk or
  from strings in the per-vault config. Both live inside the vault, which may be
  attacker-influenced. Unsafe.

### Tags are safe

`extract_tags` in `src/note/frontmatter.rs` passes every tag through
`tag::normalize` and `slug::normalize_segment`. The charset filter in
`src/text/slug.rs` keeps only `[a-z0-9-]`, with `/` between segments. Tags
therefore carry no control or bidi characters by construction. A blanket
"sanitize every cell" policy is still harmless, because sanitizing a clean
value does nothing, but tags are not an exposure.

### Confirmed surfaces

Non-interactive output, in `src/bin/ntropy/run/output.rs`:

- `print_notes` writes `note.title` and `note.path.display()` into table cells,
  and `write_row` writes them with plain `write!`. Tags are safe.
- `reference` writes the raw title. It feeds the delete confirmation prompt, the
  `Deleted {reference}` line and the ambiguous-match list.
- `print_warnings` writes the skipped file's name to stderr unchanged.
- `print_info` writes template names unchanged. Top-tag names are safe.
- `print_views` writes the view name and field unchanged. This is the display
  side of the hostile view name in the view name fix (see Relations).

Reconcile report, in `src/bin/ntropy/run/mod.rs`, `cmd_reconcile`:

- `renamed {from}`, where `from` is the file name of the note's old path, is
  raw. The `to` value is canonical and safe.
- `relinked {from} -> {to} in {note}`, where `from` is the raw body link target.
  This is the injection vector described above. `to` and `note` are canonical
  and safe.

Interactive picker, in `src/bin/ntropy/run/picker/`. Confirmed:

- `align_candidates` in `picker/layout.rs` builds `Row.display` from `truncate`
  and `pad` of `candidate.title`. `unicode-width` gives control characters
  width 0, so ESC neither uses up the truncation budget nor gets removed. It
  flows into `Row.display` unchanged.
- `draw_row` in `picker/mod.rs` prints each character of `row.display` with
  `style::Print(c)`, with no filtering, onto a raw-mode alternate screen.

LSP, a lower-severity surface:

- `completion/link.rs` sets the label to `entry.title`, the detail to the target
  filename, and inserts `format!("{}]({})", entry.title, target)` as literal text
  on clients without snippet support. The snippet path escapes `\`, `$` and `}`
  but not control bytes, so the two escapers are independent.
- `navigation.rs` sets the workspace symbol name to `entry.title`.
- Most LSP clients render in a GUI or sanitize labels. Terminal clients, such as
  a Neovim completion menu or an fzf-lua picker, render them raw. The primitive
  is the same, with lower severity. Include the LSP in the fix, because the
  helper is reachable from it.

Deliberately out of scope, because sanitizing these would corrupt legitimate
echoes: the `no note matches` error, the ambiguous-match header, and the
`Added` and `Removed view` lines. Those values come from the user's own argv,
so they are self-inflicted and not attacker data. The `Reconciling vault at`
banner shows the resolved root. A hostile pointer file could aim the vault at a
path with control characters, but the impact is low, and sanitizing the path
cells covers the note-derived cases. Leave the banner alone in this fix.

## Impact

Medium severity, gated by the walk-up discovery of a vault the user did not
author. Once that holds, exploitation requires dropping one `.md` file.

- Layout corruption happens on any output. A `\n` in a title splits one table
  row over two lines, which breaks the line-oriented contract of the table
  output. `\t` and other zero-width C0 characters throw off the `unicode-width`
  column arithmetic in `output.rs`.
- Clipboard hijack by OSC 52 is possible today on kitty, Windows Terminal and
  tmux with `set-clipboard on`. It is gated or prompted on xterm, iTerm2,
  Alacritty and recent VTE. It is a credible route to code execution through a
  pasted command, but not on every terminal.
- Spoofing is a real integrity risk. ntropy finds and deletes notes by fuzzy
  matched title, so a bidi-override title that looks like another note's title
  can lead the user to delete the wrong one. This is the Trojan Source class
  (CVE-2021-42574), distinct from control injection.
- The picker runs in raw mode on the alternate screen. Injected cursor, scroll
  region or alternate screen sequences can desync the live UI and leave the
  terminal broken on exit. The picker therefore has a larger UI integrity blast
  radius than the table. Both are the same vulnerability and should be fixed
  together.

## Suggested fix

Escape visibly, unconditionally, everywhere. Replacing characters with a
visible token keeps the information the user needs to identify a note. Stripping
would merge two notes whose titles differ only in control bytes into identical
display strings, which is the worst outcome for delete by title. A `?` is
width-stable but lossy. Visible escapes show that something specific was there,
they remain greppable, and they mark tampering. This matches how `git` quotes
untrusted paths.

- Use caret notation for C0 and DEL: ESC becomes `^[`, TAB `^I`, LF `^J`, and
  DEL `^?`.
- Use `\u{hex}` for C1 characters and bidi controls.
- Every replacement is ASCII, so the output has exact width and the width
  arithmetic keeps working.
- Apply it unconditionally, not gated on whether stdout is a terminal. ADR 0033
  requires identical output whether the sink is a TTY, a pipe or forced plain
  text, so `ntropy search | less` stays aligned. The final sink cannot be known.
  Output may be redirected, piped, re-emitted into a terminal later or kept in
  scrollback. `git` sanitizes untrusted paths regardless of isatty
  (`core.quotePath` defaults to true), and this fix follows `git`, not `ls`.
- Table and picker use the same helper and the same policy. Only the call sites
  differ.

### Character set

Escape these, matching on `char`, since `&str` is valid UTF-8:

- C0, `U+0000` to `U+001F`, including `\t` and `\n`. They break the table and
  picker layout. This is display sanitization, not slug or query normalization.
- DEL, `U+007F`.
- C1, `U+0080` to `U+009F`. `U+009B` is a one-character CSI introducer, and
  `U+0085` (NEL) is a line break on some terminals.
- Bidi overrides and embeddings, `U+202A` to `U+202E`, and isolates,
  `U+2066` to `U+2069`. These are the Trojan Source characters. They are in
  scope because they sit at the same choke point at trivial cost. They are a
  spoofing defence rather than escape injection, so they could be scoped out
  instead. See the open questions.

Do not touch these: the left-to-right and right-to-left marks `U+200E`,
`U+200F` and `U+061C`, which are needed for mixed-script titles. The zero-width
joiner and non-joiner `U+200D` and `U+200C`, which are needed for emoji and
Indic scripts. Combining marks, CJK characters and any printable wide
character.

### Location and ordering

Add one helper, `pub(crate) fn sanitize_for_terminal(&str) -> Cow<'_, str>`, in
a new module `src/bin/ntropy/run/sanitize.rs`. It belongs in the binary, not the
library, because the presentation layer owns it. Every CLI consumer lives in
the binary. It can move to `src/text/terminal.rs` later if the LSP needs it.
That move is a one-line change.

Insertion points:

1. `write_table` in `output.rs`. Sanitize every cell in this one place, then
   `column_widths` and `pad` measure the sanitized string. Header cells are
   trusted literals, so the helper returns them borrowed.
2. `reference` in `output.rs`, on the title.
3. `print_warnings` in `output.rs`, on the file name.
4. `print_info` in `output.rs`, on the template name.
5. `cmd_reconcile` in `run/mod.rs`, on the file name of `rename.from` and on
   `rewrite.from`.
6. `picker/layout.rs`, on the title and the tags before `measure` takes their
   natural widths and before `truncate` and `render_tags` cut them to the widths
   `allocate_columns` gives, and on the fields that feed `search_text`, so that
   `Row.display` and `Row.search` are both clean. `measure` and the cutting in
   `align_candidates` must see the same sanitized strings.
7. The LSP, at lower priority: `completion/link.rs` and `navigation.rs`.

Sanitize first, then measure, pad and truncate. This ordering is mandatory.
The reversed order fails in three ways:

- `truncate` with a raw ESC, which has width 0, keeps the string within budget.
  If sanitizing happens afterwards, ESC expands to `^[`, which is two columns, and
  the title exceeds its column width and pushes every later column out of line.
- In the table, if `column_widths` measures raw cells but `write_row` writes
  sanitized cells, each control character leaves padding two columns short.
  Measuring and writing must see the same sanitized string, which is why the
  table sanitizes once inside `write_table`, before both steps.
- Picker highlights depend on it. `PickerState` scores over `Row.search` and
  matches again over `Row.display` to get highlight positions as character
  indices into the display (`picker/state.rs`, consumed in `draw_row` in
  `picker/mod.rs`). If the display is sanitized before `PickerState::new`, every
  index refers to the sanitized text. Sanitizing later, for example in
  `draw_row`, would put the highlight on the wrong characters. Sanitize when the
  `Row` is constructed, upstream of the matcher. Do not move it into `draw_row`,
  which also owns the SGR styling.

### Implementation

A hand-rolled classifier of about fifteen lines is enough. The
`strip-ansi-escapes` crate is the wrong tool. It removes only well-formed CSI
and OSC sequences, not a lone ESC, a `\n`, a C1 introducer or a bidi override.
A dependency would add surface for less correctness. `Cow` gives a zero
allocation fast path for clean input, so table and picker rendering stay
allocation free in the common case.

```rust
use std::borrow::Cow;

/// Replace terminal-unsafe characters with a visible, width-stable token so an
/// untrusted note field cannot emit control sequences or reorder the line.
/// C0/DEL -> caret notation (`ESC` -> `^[`); C1 and bidi override/isolate
/// chars -> `\u{hex}`. Replacements are ASCII, so display width is exact and
/// can be measured/padded/truncated normally. Clean fields borrow unchanged.
pub(crate) fn sanitize_for_terminal(s: &str) -> Cow<'_, str> {
    if !s.chars().any(is_unsafe) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{00}'..='\u{1f}' => { out.push('^'); out.push((b'@' + c as u8) as char); }
            '\u{7f}' => out.push_str("^?"),
            c if is_unsafe(c) => out.push_str(&format!("\\u{{{:02x}}}", c as u32)),
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

fn is_unsafe(c: char) -> bool {
    matches!(c,
        '\u{00}'..='\u{1f}' | '\u{7f}' | '\u{80}'..='\u{9f}'
        | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}
```

The existing width, alignment and snapshot tests use only clean ASCII, CJK and
combining input, which `is_unsafe` leaves alone, so they return `Borrowed` and
stay unchanged. Only the new tests exercise the escape path.

## Tests

- Helper: ESC becomes `^[`, NUL `^@`, TAB `^I`, LF `^J`, CR `^M` and DEL `^?`.
  U+0085 becomes `\u{85}` and U+202E becomes `\u{202e}`. Clean input returns
  `Cow::Borrowed`, checked with `matches!`. Pass-through input such as `日本語`,
  `e\u{301}`, an emoji ZWJ sequence and a plain space stays unchanged. The OSC 52
  payload `"\u{1b}]52;c;ZXZpbAo=\u{7}"` produces output with no `0x1B` byte.
- Table: a title with `\n` and `\t` gives the same number of physical lines as
  rows plus header, with no split row, aligned columns and no trailing
  whitespace. An OSC 52 title leaves no `0x1B` or `0x07` in the rendered
  string. A path cell with C0 characters is escaped.
- `print_warnings`: factor out a `format_warning` function and assert that a file
  name with C0 characters yields no raw control byte.
- Picker: an ESC-bearing title gives a `Row.display` with no `0x1B` and a title
  cell no wider than the title column width. Two candidates of the same visible
  length still align, which proves sanitize-before-truncate. A bidi-override
  title is escaped.
- Reconcile: a body link `[x](01ARZ3NDEKTSV4RRFFQ69G5FAV-<ESC>evil.md)` gives a
  `relinked` line with no raw `0x1B`. Factor the report line into a formatter to
  test it.
- Regression: CJK and combining mark cases render byte-identical after the
  helper lands.

## Open questions

- Whether the bidi overrides and isolates stay in scope. They are a spoofing
  defence rather than escape injection, and the recommendation is to include
  them because they cost nothing extra at the same choke point.

## Further notes

- Sanitize the full `note.path` cell in `output.rs`, not only the file name.
  Directory segments inside the vault can carry control bytes too.
- The view name and field cells are the display side of the view name fix (see
  Relations). The two fixes are independent. Sanitizing the display does
  nothing for traversal, and the path fix does nothing for display. Neither
  should be assumed to cover the other.
- Do not move sanitization into `draw_row`. It breaks the highlight index
  alignment described above.
- A secondary sweep is still needed. Check that no `bail!`, `context` or `?`
  error path interpolates an unsanitized note field into a message on stderr.
  `main.rs` prints the anyhow chain, so a raw error that names a note field would
  reach the terminal unescaped. Scan-warning messages are covered by
  `print_warnings`. This is not the core fix.
- The LSP snippet path already has `escape_snippet` in `completion/link.rs`, which
  escapes `\`, `$` and `}` only. The control byte escaper is separate, and both
  must run on the snippet insertion path.

## Done when

- No untrusted note field (title, filename, path, link target, template name,
  view name or field) reaches stdout, stderr or the picker with raw C0, C1, DEL
  or bidi override characters. Tests feed ESC, OSC 52 and bidi payloads and
  check the produced strings.
- The aligned table and picker layout stay correct when a title contains `\t`,
  `\n` or other zero-width control characters. A test verifies
  sanitize-before-measure.
- Sanitization is unconditional, not gated on isatty, and the identical output
  guarantee of ADR 0033 holds.
- Legitimate titles with wide CJK characters, combining marks, emoji ZWJ
  sequences or mixed scripts render unchanged. The existing output and picker
  snapshot tests stay green.
- Tags are confirmed to be control free by normalization. Tag sanitization is
  not needed, though it would be harmless.

## Relations

- Relates to: [View names are used as unvalidated paths, so a config can delete files outside the vault](01KWH7Y8ZFKD52TJ3AEGSSBQFR-view-names-are-used-as-unvalidated-paths-so-a-config-can-delete-files.md), the view name and field cells share the root cause. The fixes are independent.
- Relates to: [Decide whether walk-up vault discovery needs a trust boundary](01KWH7Y8ZFKD52TJ3AEGSSBQFS-decide-whether-walk-up-vault-discovery-needs-a-trust-boundary.md), walk-up discovery is what makes a hostile vault reachable without a deliberate open.
