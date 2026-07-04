# SEC-5: Untrusted note fields printed raw to the terminal (escape-sequence injection)

> **STATUS: TRIAGED (code review + Fable security triage, 2026-07-02).**
> Both the non-interactive output and the interactive picker are confirmed
> affected by code reading; a Fable security-engineer pass verified every
> surface, corrected the scope (tags are safe; the LSP and reconcile link-target
> are additional vectors), and produced the sanitization design below.

## Summary

Note titles, filenames, note **path** components, template names, view
name/field, and body **link targets** are attacker-influenced in a
shared/cloned vault (see the SEC-4 walk-up trust model) and are written to
stdout/stderr and into the raw-mode picker **byte-for-byte with no control-char
filtering**. A field carrying a raw `ESC` (0x1B) byte — or `\x1b` via
double-quoted YAML — can emit terminal control sequences: title-bar rewrites,
screen clears, cursor moves, and on permissive emulators OSC 52 clipboard writes
(place attacker text into the clipboard for a later paste). Embedded `\n`/`\t`
additionally corrupt the ADR 0033 table and the picker layout on every sink.
`ls`, `git`, and ripgrep all sanitize/quote control bytes in untrusted names.

## Severity: Medium (SEC-4-gated)

- **Unconditional, no attacker sophistication:** layout corruption. A `\n` in a
  title splits one logical table row into two physical lines (breaking the ADR
  0033 line-oriented `tail -n +2` contract); `\t` and other width-0 C0 desync
  the `unicode-width` column math (`output.rs:203-242`). Fires on *any* sink.
- **Emulator-conditional escalation:** OSC 52 clipboard hijack is real **today**
  on kitty (clipboard write allowed by default), Windows Terminal, and tmux with
  `set-clipboard on`; gated/prompted on xterm (`allowWindowOps`), iTerm2,
  Alacritty, recent VTE. Credible route to code execution via a pasted command,
  but not universal.
- **Spoofing:** ntropy resolves and **deletes** notes by fuzzy-matched title, so
  a bidi-override title that visually impersonates another note is a genuine
  integrity risk (Trojan Source / CVE-2021-42574 class), distinct from control
  injection.
- Gated behind SEC-4 (you must operate on a vault you did not author); once that
  holds, exploitation is dropping one `.md`. **Medium**, not High/Low.
- Picker vs. table: same vulnerability severity, but the picker runs in raw mode
  on the alternate screen, so injected cursor/scroll-region/alt-screen sequences
  can desync the live UI and corrupt the terminal on exit — higher UI-integrity
  blast radius. Fix both together.

## How untrusted bytes get in

- **Titles**: lifted verbatim from YAML — `frontmatter.rs:122-127` does
  `mapping.get("title").as_str().map(str::to_string)` with only a
  `trim().is_empty()` filter. Any control byte in a plain or double-quoted scalar
  survives into `Note.title` (`note/mod.rs:34,92-100`). **Unsafe.**
- **Filenames / paths**: from disk via `to_string_lossy` / `Path::display`. A
  valid-UTF-8 control char (ESC = 0x1B is a valid scalar) passes unchanged; only
  *invalid* bytes become U+FFFD. **Unsafe.**
- **Body link targets**: the link regex `\[([^\]]*)\]\(([^)\s]+)\)`
  (`src/link/mod.rs:52`) forbids only `)` and Unicode whitespace in the target.
  ESC (U+001B) is **not** `\p{White_Space}`, so
  `[x](01ARZ3NDEKTSV4RRFFQ69G5FAV-<ESC>evil.md)` passes both the regex and
  `parse_target` (`link/mod.rs:171-181`, which only constrains the 26-char ULID
  prefix and `.md` suffix). So `reconcile`'s relink `from` is a **true escape
  carrier**, not cosmetic. **Unsafe.**
- **Template names / view name+field**: file stems read from disk / per-vault
  config strings — both inside the (attacker-influenced) vault. **Unsafe.**

## Scope correction: tags are SAFE (do not need sanitizing)

`extract_tags` (`frontmatter.rs:143-162`) runs every tag through
`tag::normalize` → `slug::normalize_segment`, whose charset filter
(`src/text/slug.rs:65-68`) keeps only `[a-z0-9-]` (segments joined with `/`). So
`note.tags` is `[a-z0-9/-]` by construction — no C0/DEL/C1/bidi. `Candidate.tags`
preserves that (`ops/select.rs:103`). Tags carry no exposure. (A blanket
"sanitize every cell" policy is still fine — sanitizing them is an idempotent
no-op — but the exposure is title/filename/path/link-target/template/view-name.)

## Confirmed surfaces

### Non-interactive output (`src/bin/ntropy/run/output.rs`)
- `print_notes` (`output.rs:30-49`): `note.title` (`:38`) and
  `note.path.display()` (`:40`) raw into cells → `write_row` plain `write!`
  (`:221-229`). Tags (`:39`) safe.
- `reference` (`output.rs:57-64`): raw `title`; used in the delete confirm
  prompt (`run/mod.rs:300`), the `Deleted {reference}` line (`:310`), and the
  ambiguous-match list (`:390`).
- `print_warnings` (`output.rs:169-178`): skipped file `name` (`to_string_lossy`,
  `:176`) raw to stderr.
- `print_info` (`output.rs:104-152`): **template names** (`:149`, from
  `ops/info.rs:94`) unsafe; top-tag names (`:141`) safe.
- `print_tags` / `print_views` (`output.rs:77-99`): tag names safe; **view
  name/field** (`:93`) unsafe (display side of SEC-3's hostile view name).

### Reconcile report (`src/bin/ntropy/run/mod.rs`)
- `cmd_reconcile` (`run/mod.rs:230-244`): `renamed {from}` where `from =
  file_name(rename.from = note.path)` unsafe (`to` is canonical, safe); and
  `relinked {from} -> {to} in {note}` where `rewrite.from` (`:241`) is the raw
  body link target — **injection vector** per above (`to`/`note` canonical, safe).

### Interactive picker (`src/bin/ntropy/run/picker/`) — CONFIRMED
- `align_candidates` (`picker/layout.rs:33-66`): builds `Row.display` from
  `truncate`/`pad` of `candidate.title`; `unicode-width` reports control chars as
  width 0 (`layout.rs` truncate/pad), so ESC neither consumes truncation budget
  nor is removed — it flows verbatim into `Row.display`.
- `draw_row` (`picker/mod.rs:261-326`): prints each `row.display` char via
  `style::Print(c)` (`:288`), no filtering, into a raw-mode alternate screen.

### LSP (additional surface, lower severity)
- `completion/link.rs:169` `label: entry.title`; `:171` `detail: Some(target)`
  (filename); `:163` inserts `format!("{}]({})", entry.title, target)` as literal
  document text on non-snippet clients (the snippet path escapes `\ $ }` at
  `:183-192` but **not** control bytes — orthogonal escapers).
- `navigation.rs:88` `name: entry.title` into workspace-symbol results.
- Most LSP clients render in a GUI and/or sanitize labels, but terminal clients
  (Neovim completion menu, fzf-lua pickers) render raw — same primitive, lower
  severity. Include in the fix since the helper is reachable.

### Explicitly OUT of scope (do NOT sanitize — would corrupt legitimate echoes)
- `error: no note matches \`{selector}\`` (`run/mod.rs:269`), the ambiguous
  header (`:385`), `Added/Removed view \`{name}\`` (`:322-333`): `selector`/`name`
  are the **user's own argv**, already through their shell — self-inflicted, not
  attacker note data.
- The `Reconciling vault at {root}` / `info` vault-path banner is borderline (a
  hostile pointer file could aim the vault at a control-laden path) but
  low-impact; sanitizing the path *cells* covers the note-derived cases. Do not
  chase the resolved-root banner in this fix.

## Sanitization policy (decided)

**Visibly escape, unconditionally, everywhere.**

- **Escape (not strip, not `?`).** The title is data the user must read to
  identify/act on a note (open, **delete**). Stripping collapses two notes that
  differ only by control bytes into identical display strings (worst outcome for
  delete-by-title) and hides tampering; `?` is width-stable but lossy/ambiguous.
  Visible escape preserves that *something specific* was there, stays greppable,
  and signals tampering — matching `git`'s C-style quoting of untrusted paths.
  Use **caret notation for C0/DEL** (`ESC`→`^[`, `TAB`→`^I`, `NL`→`^J`,
  `DEL`→`^?`) and **`\u{hex}` for C1 and bidi**. Every replacement is ASCII, so
  the output is width-stable and the width math stays exact.
- **Unconditional, NOT isatty-gated.** ADR 0033 commits to identical output
  whether on a TTY, piped, or forced plain, precisely so `ntropy search | less`
  stays aligned. You cannot know the ultimate sink (`> f; cat f`, pagers, tmux,
  scrollback re-emit to a terminal later). `git` sanitizes untrusted paths
  regardless of isatty (`core.quotePath` default true) — follow git, not `ls`.
- Same character set, same helper, same policy for table and picker; only the
  *call site* differs.

## Exact character set (char-level — `&str` is valid UTF-8)

**Escape:**
- **C0** `U+0000..=U+001F` — *including `\t` and `\n`* (they break the table/
  picker layout; this is display sanitization, not slug/query normalization).
- **DEL** `U+007F`.
- **C1** `U+0080..=U+009F` (`U+009B` is a single-char CSI introducer; `U+0085`
  NEL is a line break on some terminals). Only ever appears as a real scalar in
  a `&str`, so match on `char`, never on raw bytes.
- **Bidi overrides/embeddings** `U+202A..=U+202E` and **isolates**
  `U+2066..=U+2069` (Trojan Source spoofing; ntropy deletes by matched title).
  **Recommended in scope** — same choke point, trivial cost. Flag as a policy
  choice (it is spoofing, not escape injection, so reasonable to scope out).

**Do NOT touch:** RTL/LTR marks `U+200E`/`U+200F`/`U+061C` (legit mixed-script
titles), ZWJ/ZWNJ `U+200D`/`U+200C` (emoji/Indic), combining marks, CJK, any
printable wide char.

## Single choke point, location, ordering

**Helper:** `pub(crate) fn sanitize_for_terminal(&str) -> Cow<'_, str>` in the
**binary tree** (new `src/bin/ntropy/run/sanitize.rs`), not the library — this
respects the headless-library boundary (`run/mod.rs:6-10`: the bin owns
presentation). All CLI consumers are in the bin. (Promoting to
`src/text/terminal.rs` later, if the LSP reuse warrants, is a one-line move.)

**Insertion points (full set):**
1. `write_table` (`output.rs:192`) — sanitize every cell in one place; then
   `column_widths` + `pad` inherently measure the sanitized string. Header cells
   are trusted literals (no-op, returns `Borrowed`).
2. `reference` (`output.rs:57`) — wrap `title`.
3. `print_warnings` (`output.rs:176`) — wrap `name`.
4. `print_info` (`output.rs:149`) — wrap template `name`.
5. `cmd_reconcile` (`run/mod.rs:230-244`) — wrap `file_name(&rename.from)` and
   `rewrite.from`.
6. `align_candidates` (`picker/layout.rs:36-39`) — sanitize `candidate.title`
   **before** `truncate`, and sanitize the fields feeding `search_text`
   (`layout.rs:73-84`) so both `Row.display` and `Row.search` are clean.
7. LSP (lower priority): `completion/link.rs:163/169/171`, `navigation.rs:88`.

**Ordering — sanitize FIRST, then measure/pad/truncate. MANDATORY.** Concrete
failures if reversed:
- `truncate(&title, TITLE_CAP)` with raw ESC (width 0) keeps it within budget;
  sanitizing the *output* then expands ESC to `^[` (2 cols) after the cap was
  computed, blowing `TITLE_CAP` and misaligning every following column.
- In the table, if `column_widths` measured raw cells (ESC width 0) but
  `write_row` wrote sanitized cells (`^[` width 2), padding is short by 2 cols
  per control char. Measuring and writing must see the same sanitized string —
  hence sanitizing once inside `write_table`, before both.
- **Picker highlight coupling (verified):** `PickerState` scores over
  `Row.search` and re-runs matching over `Row.display` to get highlight positions
  as *char indices into display* (`picker/state.rs:22-24,152-157`; consumed in
  `draw_row` at `mod.rs:284` via `highlights.binary_search(&(i as u32))`). If
  `display` is sanitized in `align_candidates` **before** `PickerState::new`,
  every highlight index is computed over sanitized text and stays valid.
  Sanitizing later (e.g. in `draw_row`) would make indices refer to the
  unsanitized string and the yellow highlight would land on wrong chars. So
  picker sanitization **must** happen at `Row` construction, upstream of the
  matcher — do NOT move it into `draw_row` (which also owns the SGR styling).

## Implementation shape (hand-rolled, no crate)

`strip-ansi-escapes` is the wrong tool — it removes only well-formed CSI/OSC
sequences, not a lone ESC, `\n`, a C1 introducer, or a bidi override. Scope is a
~15-line char classifier; a dep adds surface for less correctness. `Cow` gives a
zero-allocation fast path for the ~100% clean case, keeping the hot
table/picker render allocation-free.

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

**Existing tests stay green:** current width/alignment/snapshot tests use only
clean ASCII/CJK/combining input, which `is_unsafe` excludes → `Borrowed`
unchanged. Only new tests exercise the escape path.

## Tests to add (bug-fix-first)

- **Helper:** `ESC`→`^[`, `NUL`→`^@`, `\t`→`^I`, `\n`→`^J`, `\r`→`^M`,
  `DEL`→`^?`; `U+0085`→`\u{85}`; `U+202E`→`\u{202e}`; clean input returns
  `Cow::Borrowed` (assert `matches!`); pass-through `日本語`, `e\u{301}`, an
  emoji ZWJ sequence, plain space unchanged; OSC-52 payload
  `"\u{1b}]52;c;ZXZpbAo=\u{7}"` produces output with no `0x1B`.
- **Table:** title with `\n`+`\t` → physical line count == rows+header (no
  split), columns still align, no trailing whitespace; OSC-52 title → rendered
  String has no `0x1B`/`0x07`; path cell with C0 escaped.
- **`print_warnings`:** file name with C0 → factor a `format_warning` and assert
  no raw control byte.
- **Picker:** ESC-bearing title → `Row.display` has no `0x1B`, width ≤
  `TITLE_CAP`, two same-visible-length candidates still align (proves
  sanitize-before-truncate); bidi-override title escaped.
- **Reconcile:** body link `[x](01ARZ3NDEKTSV4RRFFQ69G5FAV-<ESC>evil.md)` →
  `relinked` line has no raw `0x1B` (factor the report line into a formatter).
- **Regression:** CJK + combining-mark cases render byte-identical after the
  helper lands.

## Anything else a thorough fix must not forget

- Sanitize the full `note.path` cell (`output.rs:40`), not just the filename —
  vault-directory segments can carry control bytes too.
- View name/field cells tie to **SEC-3** (this is its display side); the two
  fixes are independent (sanitizing display does nothing for traversal, and vice
  versa). Cross-reference so neither is assumed to cover the other.
- Do not move sanitization into `draw_row` (breaks highlight index alignment,
  §ordering).
- Secondary sweep: audit that no `bail!`/`context`/`?` error-propagation path
  interpolates an unsanitized note field into a message reaching stderr
  (`main.rs:27` prints the anyhow chain). The scan-warning path is covered by
  `print_warnings`; a raw error that names a note field would not be. Not the
  core fix.
- LSP snippet path already has `escape_snippet` (`completion/link.rs:183`) for
  `\ $ }` only; the control-byte escaper is orthogonal and both must apply on
  the snippet insertion path.

## Acceptance

- No untrusted note field (title, filename, path, link target, template name,
  view name/field) reaches stdout/stderr or the picker carrying raw C0/C1/DEL or
  bidi-override chars — proven by tests feeding ESC/OSC-52 and bidi payloads and
  asserting the produced strings.
- The aligned table and picker layout stay correct when a title contains
  `\t`/`\n`/other zero-width control chars (sanitize-before-measure verified).
- Sanitization is unconditional (not isatty-gated); ADR 0033's identical-output
  contract is preserved.
- Legitimate titles (wide CJK, combining marks, emoji ZWJ, mixed-script) render
  unchanged; existing output/picker snapshot tests stay green.
- Tags confirmed control-free by normalization (no tag sanitization required,
  though harmless if applied).
