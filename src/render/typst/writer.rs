// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! The Typst output writer: one method per escaping context
//! (`docs/design/typst-engine.md`, "Escaping" and "Writer and context
//! model").
//!
//! Escaping is a property of the write call, not of tracked emitter state.
//! Every character the emitter writes belongs to exactly one of three
//! contexts — markup text, string literals, raw content — and each has a
//! method here with its own fixed rule. Choosing the context lexically at
//! the call site removes the mode flag whose forgetting was the recurring
//! defect class of the earlier converter (unescaped URLs and captions, a
//! markup-escaped string literal; see
//! `docs/research/markdown-to-typst-conversion.md`).
//!
//! The writer stays dumb about structure: block terminators, newline
//! discipline, and indentation are per-construct knowledge that belongs to
//! the emitter event loop, not to a parallel model of Markdown constructs
//! inside the writer.

/// Every character Typst markup assigns meaning to, per the `markup()`
/// dispatcher of Typst's own lexer (`crates/typst-syntax/src/lexer.rs`,
/// main branch as of 2026-07-10). Escaping each occurrence unconditionally
/// is the foolproof rule: correctness never depends on position, word
/// boundaries, or lookahead. The set is derived from the lexer directly,
/// not from other converters, whose sets are known-incomplete.
///
/// Why each member is in the set:
///
/// - `\` escape introducer; before whitespace it is a forced line break
/// - `#` enters code mode
/// - `[` `]` content-block delimiters (an unbalanced `]` ends a block)
/// - `$` opens math
/// - `` ` `` opens raw
/// - `*` `_` strong/emphasis toggles
/// - `<` `>` label delimiters (`<` active before identifier chars)
/// - `@` reference marker
/// - `~` non-breaking-space shorthand
/// - `'` `"` smart quotes (typographic substitution, not structural)
/// - `-` dash shorthands (`--`, `---`, `-?`), minus-before-digit, bullet
/// - `.` ellipsis shorthand (`...`) and `<digits>.` enum markers
/// - `/` line/block comments (`//`, `/*` — active anywhere) and term lists
/// - `:` completes `http://` autolinks and term-list separators
/// - `=` heading markers
/// - `+` enum markers
pub const MARKUP_ACTIVE: [char; 20] = [
    '\\', '#', '[', ']', '$', '`', '*', '_', '<', '>', //
    '@', '~', '\'', '"', '-', '.', '/', ':', '=', '+',
];

/// The Typst markup writer: one method per escaping context.
///
/// The type encodes the design's central claim — every interpolation point
/// is classified as exactly one of the three contexts — as API shape.
/// Callers pick the context at the call site, so there is no mode flag to
/// forget to switch.
pub struct TypstWriter {
    out: String,
}

impl TypstWriter {
    pub fn new() -> Self {
        TypstWriter { out: String::new() }
    }

    /// Context 1: markup text. Paragraph text, heading text, list items,
    /// table cells, link labels, quote bodies, captions.
    ///
    /// Backslash-escapes every occurrence of every [`MARKUP_ACTIVE`]
    /// character. Escaping `'`, `"`, `~`, and the dash/ellipsis members
    /// also disables Typst's typographic substitutions, so note content
    /// renders character-for-character as written; typography is a concern
    /// of the theming layer, not of silent rewriting of note text.
    pub fn markup_text(&mut self, text: &str) {
        for c in text.chars() {
            if MARKUP_ACTIVE.contains(&c) {
                self.out.push('\\');
            }
            self.out.push(c);
        }
    }

    /// Context 2: string literals — the quoted arguments of emitted
    /// function calls (`#raw("…")`, `#link("…")`, image paths).
    ///
    /// Exactly two escapes: `\` → `\\` and `"` → `\"`. The markup escape
    /// set is wrong here: a backslash-escaped `#` inside a Typst string
    /// stays a literal backslash followed by `#`.
    ///
    /// The caller owns the surrounding quotes (via [`Self::syntax`]),
    /// because the quotes are Typst syntax, not user text.
    pub fn string_literal(&mut self, text: &str) {
        for c in text.chars() {
            if c == '\\' || c == '"' {
                self.out.push('\\');
            }
            self.out.push(c);
        }
    }

    /// Context 3: raw content between code fences. No escaping at all; the
    /// fence must be sized with [`fence`] so the content cannot close it.
    pub fn raw(&mut self, text: &str) {
        self.out.push_str(text);
    }

    /// Emitter-owned Typst syntax: `#emph[`, `= `, fences, argument commas.
    /// This is the only unescaped channel, and its visibility is restricted
    /// to the `typst` module so user-derived text cannot reach it from
    /// outside the emitter. Keeping it module-private makes that reviewable:
    /// the emitter is the only caller, and any user data in a `syntax`
    /// argument is a visible defect in the diff.
    // Its production caller is the emitter event loop, which arrives in the
    // next phase and removes this allow; until then only tests call it.
    #[allow(dead_code)]
    pub(super) fn syntax(&mut self, s: &str) {
        self.out.push_str(s);
    }

    pub fn finish(self) -> String {
        self.out
    }
}

impl Default for TypstWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the backtick fence for a raw block: one backtick more than the
/// longest backtick run inside the content, minimum three.
///
/// A fixed-length fence is closable from inside: content documenting
/// Markdown or Typst itself can legally contain arbitrarily long backtick
/// runs, so the fence length must be derived from the content.
pub fn fence(content: &str) -> String {
    let longest_run = content.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat((longest_run + 1).max(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    use typst_syntax::{SyntaxKind, SyntaxNode};

    // =====================================================================
    // Round-trip verification against the real Typst parser
    // =====================================================================
    //
    // Escaping correctness is the round-trip property: Typst must read the
    // emitted markup back as exactly the original text. These tests check
    // it against `typst-syntax` — the same crate the escaping rules were
    // derived from — without executing any external tool. The property:
    // parsing the escaped output yields only plain-text node kinds (Text,
    // Escape, Space), no parse errors, and reassembling those nodes gives
    // back the original input byte for byte. Any markup construct the
    // parser recognizes (a heading, a comment, a smart quote, a shorthand)
    // is an escaping failure.

    /// Walk the parse tree; collect reassembled plain text and every node
    /// kind that is not plain text. `Escape` leaves reassemble to the
    /// character after the backslash. Container `Markup` nodes are
    /// transparent; everything else (Strong, Heading, LineComment,
    /// Shorthand, SmartQuote, Linebreak, ...) is foreign.
    fn collect(node: &SyntaxNode, text: &mut String, foreign: &mut Vec<SyntaxKind>) {
        let is_leaf = node.children().next().is_none();
        if is_leaf {
            match node.kind() {
                SyntaxKind::Text | SyntaxKind::Space => text.push_str(node.leaf_text()),
                SyntaxKind::Escape => {
                    // Escape node text is `\` + the escaped character.
                    text.extend(node.leaf_text().chars().skip(1));
                }
                other => foreign.push(other),
            }
        } else {
            if node.kind() != SyntaxKind::Markup {
                foreign.push(node.kind());
            }
            for child in node.children() {
                collect(child, text, foreign);
            }
        }
    }

    /// Assert the round-trip property for one input; returns the failure
    /// description instead of panicking so the corpus loop can report every
    /// failing case at once.
    fn assert_round_trip(input: &str) -> Result<(), String> {
        let mut writer = TypstWriter::new();
        writer.markup_text(input);
        let emitted = writer.finish();

        let root = typst_syntax::parse(&emitted);
        let (errors, _warnings) = root.errors_and_warnings();
        if !errors.is_empty() {
            return Err(format!("emitted {emitted:?}: parse errors: {errors:?}"));
        }

        let mut text = String::new();
        let mut foreign = Vec::new();
        collect(&root, &mut text, &mut foreign);

        if !foreign.is_empty() {
            return Err(format!(
                "emitted {emitted:?}: markup constructs recognized: {foreign:?}"
            ));
        }
        if text != input {
            return Err(format!(
                "emitted {emitted:?}: reassembled {text:?} != input"
            ));
        }
        Ok(())
    }

    /// The corpus is enumerated mechanically from the design doc's tables
    /// rather than curated by anecdote: every escape-set member in every
    /// position that Typst's lexer treats differently (line start, mid-word,
    /// after a space, end of input, start of a second line), plus every
    /// multi-character sequence, plus the backslash-before-whitespace
    /// linebreak case.
    fn corpus() -> Vec<String> {
        let mut cases = Vec::new();

        for c in MARKUP_ACTIVE {
            cases.push(format!("{c} leads the line"));
            cases.push(format!("mid{c}word"));
            cases.push(format!("after space {c} here"));
            cases.push(format!("at the end{c}"));
            cases.push(format!("first line\n{c} second line"));
            // Line-anchored markers require a following space; cover the
            // marker-like shape explicitly.
            cases.push(format!("{c} marker-shaped"));
            // Doubled occurrences catch sequence triggers (`--`, `//`).
            cases.push(format!("doubled {c}{c} run"));
        }

        // Multi-character sequences from the design doc, verbatim.
        let sequences = [
            "an ellipsis... here",
            "en--dash",
            "em---dash",
            "soft-?hyphen",
            "minus -5 degrees",
            "visit http://example.com now",
            "or https://example.com/path",
            "a // line comment shape",
            "a /* block comment shape */",
            "6. numbered marker shape",
            "= heading shape",
            "- bullet shape",
            "+ enum shape",
            "/ term: shape",
            "<label-shape>",
            "@reference-shape",
            "trailing backslash \\",
            "\\ leading backslash",
            "'single' and \"double\" quotes",
            "nbsp~shape",
            "math $x^2$ shape",
            "code `raw` shape",
            "#code-mode shape",
            "[content block] shape",
            "unbalanced ] bracket",
            "*strong* and _emph_ shapes",
            "Über Größe 日本語 mixed with * unicode",
        ];
        cases.extend(sequences.iter().map(|s| s.to_string()));

        cases
    }

    #[test]
    fn markup_escaping_round_trips_through_the_typst_parser() {
        let mut failures = Vec::new();
        for input in corpus() {
            if let Err(e) = assert_round_trip(&input) {
                failures.push(format!("{input:?}: {e}"));
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} corpus cases failed:\n{}",
            failures.len(),
            corpus().len(),
            failures.join("\n")
        );
    }

    /// Snapshot the whole corpus as `input → escaped` pairs so any change to
    /// the escape set surfaces as a reviewable diff (ADR 0021). The
    /// round-trip test proves the escaping correct; this pins the exact
    /// bytes it produces.
    #[test]
    fn markup_escaping_snapshot_of_input_to_escaped_pairs() {
        let pairs = corpus()
            .iter()
            .map(|input| {
                let mut writer = TypstWriter::new();
                writer.markup_text(input);
                format!("{input:?} → {:?}", writer.finish())
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(pairs);
    }

    // =====================================================================
    // String-literal context
    // =====================================================================

    /// Emit a full `#raw("…")` call the way the engine does (syntax around,
    /// string_literal inside) and require it to parse cleanly. This is the
    /// shape that broke the earlier converter: markup escaping applied
    /// inside a string, and strings not escaped at all.
    #[test]
    fn string_literal_context_parses_inside_a_function_call() {
        let adversarial = r#"back\slash and "quote" and #hash and ] bracket"#;

        let mut writer = TypstWriter::new();
        writer.syntax("#raw(\"");
        writer.string_literal(adversarial);
        writer.syntax("\")");
        let emitted = writer.finish();

        let root = typst_syntax::parse(&emitted);
        let (errors, _warnings) = root.errors_and_warnings();
        assert!(errors.is_empty(), "parse errors in {emitted:?}: {errors:?}");
    }

    #[test]
    fn string_literal_escapes_exactly_backslash_and_quote() {
        let mut writer = TypstWriter::new();
        writer.string_literal(r#"a\b "c" #not-escaped"#);
        assert_eq!(writer.finish(), r#"a\\b \"c\" #not-escaped"#);
    }

    // =====================================================================
    // Fence sizing
    // =====================================================================

    #[test]
    fn fence_grows_past_the_longest_backtick_run() {
        assert_eq!(fence("no backticks"), "```");
        assert_eq!(fence("inline `code`"), "```");
        assert_eq!(fence("a ``` fence inside"), "````");
        assert_eq!(fence("``````"), "```````");
        assert_eq!(fence(""), "```");
    }
}
