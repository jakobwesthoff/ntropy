// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Frontmatter translation: a `serde_yaml_ng::Value` becomes a Typst value
//! literal (`docs/design/typst-engine.md`, "Metadata travels as typed Typst
//! values").
//!
//! Frontmatter reaches the template as a typed Typst value, never as text
//! spliced into markup. The complete mapping is translated recursively into
//! the Typst literal that appears in
//! `#show: note.with(title: ..., frontmatter: (...))`, so a metadata value
//! can never be interpreted as Typst code, whatever it contains.
//!
//! Translation reuses the writer's channel model rather than assembling raw
//! strings: structural tokens (parentheses, commas, colons, numbers, `true`,
//! `none`) go through the module-private `syntax` channel because they are
//! generated Typst code, and every span of user-derived text (string scalars,
//! dictionary keys, the textual form of tagged values) goes through
//! `string_literal`, which owns the `\` and `"` escaping. The user-derived
//! text therefore reaches the output through exactly the one escaped channel
//! that keeps it inert.

use serde_yaml_ng::Value;

use super::writer::TypstWriter;

/// Translate a frontmatter value into a Typst value literal string ready to
/// splice into `note.with(frontmatter: ...)`.
// The document-assembly step of this same phase is the production consumer;
// until it lands the module has no in-crate caller, and clippy runs with
// `-D warnings`. The reviewer removes this allow when assembly wires the
// translation in.
#[allow(dead_code)]
pub fn value_literal(value: &Value) -> String {
    let mut writer = TypstWriter::new();
    write_value(&mut writer, value);
    writer.finish()
}

/// Recursively emit `value` into `writer`.
///
/// One arm per YAML value variant. The recursion depth follows the value's
/// nesting, so arbitrarily nested sequences and mappings translate without
/// special handling.
fn write_value(writer: &mut TypstWriter, value: &Value) {
    match value {
        Value::Null => writer.syntax("none"),
        Value::Bool(b) => writer.syntax(if *b { "true" } else { "false" }),
        Value::Number(n) => write_number(writer, n),

        // The only place user text enters the literal directly. The quotes are
        // Typst syntax and belong to the emitter; the content between them is
        // escaped by `string_literal`.
        Value::String(s) => {
            writer.syntax("\"");
            writer.string_literal(s);
            writer.syntax("\"");
        }

        Value::Sequence(items) => write_sequence(writer, items),
        Value::Mapping(mapping) => write_mapping(writer, mapping),

        // A tagged value has no direct Typst counterpart. Its whole serialized
        // YAML form (tag included) is preserved textually as a string literal,
        // so `!degrees 90` survives as the Typst string `"!degrees 90"` rather
        // than being reinterpreted or silently dropping its tag.
        Value::Tagged(_) => {
            let serialized = serde_yaml_ng::to_string(value)
                .expect("serializing an in-memory YAML value cannot fail");
            writer.syntax("\"");
            writer.string_literal(serialized.trim_end());
            writer.syntax("\"");
        }
    }
}

/// Emit a YAML number, preserving its integer-versus-float identity.
///
/// Integers translate to Typst ints and floats to Typst floats, so numeric
/// frontmatter keeps the type the note author wrote. The three special YAML
/// floats have no literal syntax in either language and map to Typst's native
/// `float.inf` / `float.nan` values.
fn write_number(writer: &mut TypstWriter, n: &serde_yaml_ng::Number) {
    if let Some(i) = n.as_i64() {
        writer.syntax(&i.to_string());
    } else if let Some(u) = n.as_u64() {
        // u64 values above `i64::MAX` still fit here; Typst parses the plain
        // digit run as an integer.
        writer.syntax(&u.to_string());
    } else {
        let f = n
            .as_f64()
            .expect("a Number that is neither i64 nor u64 is f64");
        writer.syntax(&format_float(f));
    }
}

/// Format a finite float so Typst reads it back as a float, and map the three
/// non-finite YAML floats to Typst's own values.
///
/// The `{:?}` formatting of a finite `f64` always carries a decimal point or
/// an exponent, which is what keeps `2.0` from collapsing to the Typst integer
/// `2`.
fn format_float(f: f64) -> String {
    if f.is_nan() {
        "float.nan".to_string()
    } else if f.is_infinite() {
        if f < 0.0 {
            "-float.inf".to_string()
        } else {
            "float.inf".to_string()
        }
    } else {
        format!("{f:?}")
    }
}

/// Emit a Typst array.
///
/// A single-element array needs a trailing comma: `(value)` is a
/// parenthesized value in Typst, and only `(value,)` is a one-element array.
/// The empty array is `()`.
fn write_sequence(writer: &mut TypstWriter, items: &[Value]) {
    writer.syntax("(");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            writer.syntax(", ");
        }
        write_value(writer, item);
    }
    if items.len() == 1 {
        writer.syntax(",");
    }
    writer.syntax(")");
}

/// Emit a Typst dictionary.
///
/// The empty dictionary is `(:)`, distinct from the empty array `()`. Keys are
/// always emitted as quoted string literals rather than bare identifiers: a
/// quoted key is always legal whatever the key contains, so uniform quoting
/// removes any question of which YAML keys form valid Typst identifiers.
fn write_mapping(writer: &mut TypstWriter, mapping: &serde_yaml_ng::Mapping) {
    if mapping.is_empty() {
        writer.syntax("(:)");
        return;
    }

    writer.syntax("(");
    for (index, (key, value)) in mapping.iter().enumerate() {
        if index > 0 {
            writer.syntax(", ");
        }
        writer.syntax("\"");
        writer.string_literal(&key_to_string(key));
        writer.syntax("\": ");
        write_value(writer, value);
    }
    writer.syntax(")");
}

/// Reduce a mapping key to the text of its dictionary key.
///
/// A string key is its own text. Any other scalar (number, bool, null) is
/// reduced to its YAML scalar form, so `1: a` keys the dictionary under
/// `"1"`. Complex keys (a sequence or mapping used as a key) fall through the
/// same serialization; they are rare and still yield a stable, unique string.
fn key_to_string(key: &Value) -> String {
    match key {
        Value::String(s) => s.clone(),
        other => serde_yaml_ng::to_string(other)
            .expect("serializing an in-memory YAML value cannot fail")
            .trim_end()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse `#let x = (<literal>)` and require it error-free. Wrapping the
    /// literal in a binding puts it in Typst code context, the same context
    /// `note.with(frontmatter: ...)` splices it into. Any injection or
    /// malformed literal surfaces as a parse error.
    fn assert_parses(literal: &str) {
        let source = format!("#let x = ({literal})");
        let root = typst_syntax::parse(&source);
        let (errors, _warnings) = root.errors_and_warnings();
        assert!(
            errors.is_empty(),
            "literal {literal:?} produced parse errors in {source:?}: {errors:?}"
        );
    }

    /// Translate a YAML document and return its Typst literal.
    fn literal_of(yaml: &str) -> String {
        let value: Value = serde_yaml_ng::from_str(yaml).expect("the test YAML parses");
        value_literal(&value)
    }

    // =====================================================================
    // Scalars: the four leaf variants
    // =====================================================================

    #[test]
    fn null_becomes_none() {
        assert_eq!(literal_of("null"), "none");
    }

    #[test]
    fn booleans_translate_directly() {
        assert_eq!(literal_of("true"), "true");
        assert_eq!(literal_of("false"), "false");
    }

    #[test]
    fn string_becomes_a_quoted_literal() {
        assert_eq!(literal_of("hello world"), "\"hello world\"");
    }

    // =====================================================================
    // Numbers: integer-ness, sign, floats, special floats
    // =====================================================================

    #[test]
    fn integers_stay_integers() {
        assert_eq!(literal_of("42"), "42");
        assert_eq!(literal_of("0"), "0");
    }

    #[test]
    fn negative_integers_keep_their_sign() {
        assert_eq!(literal_of("-7"), "-7");
    }

    #[test]
    fn large_unsigned_integers_survive() {
        // Above i64::MAX, so it exercises the u64 branch.
        assert_eq!(literal_of("18446744073709551615"), "18446744073709551615");
    }

    #[test]
    fn floats_carry_a_decimal_point() {
        assert_eq!(literal_of("1.5"), "1.5");
        assert_eq!(literal_of("-2.25"), "-2.25");
        // A whole-valued float must not collapse to a Typst integer.
        assert_eq!(literal_of("2.0"), "2.0");
    }

    #[test]
    fn special_floats_map_to_typst_values() {
        assert_eq!(literal_of(".inf"), "float.inf");
        assert_eq!(literal_of("-.inf"), "-float.inf");
        assert_eq!(literal_of(".nan"), "float.nan");
        // The special-float forms must be valid in Typst code context.
        assert_parses("float.inf");
        assert_parses("-float.inf");
        assert_parses("float.nan");
    }

    // =====================================================================
    // Sequences: the empty / single / multi trichotomy
    // =====================================================================

    #[test]
    fn empty_sequence_is_empty_parens() {
        assert_eq!(literal_of("[]"), "()");
    }

    #[test]
    fn single_element_sequence_keeps_a_trailing_comma() {
        // Without the trailing comma `(1)` would be a parenthesized integer,
        // not a one-element array.
        assert_eq!(literal_of("[1]"), "(1,)");
    }

    #[test]
    fn multi_element_sequence_has_no_trailing_comma() {
        assert_eq!(literal_of("[1, 2, 3]"), "(1, 2, 3)");
    }

    // =====================================================================
    // Mappings: emptiness, keys, nesting
    // =====================================================================

    #[test]
    fn empty_mapping_is_the_colon_form() {
        assert_eq!(literal_of("{}"), "(:)");
    }

    #[test]
    fn mapping_keys_are_always_quoted() {
        assert_eq!(literal_of("key: value"), "(\"key\": \"value\")");
    }

    #[test]
    fn keys_needing_escaping_are_escaped() {
        // A key containing a quote and a backslash must stay inert as a key.
        let value: Value = serde_yaml_ng::from_str(r#"{ "a\"b": 1 }"#).expect("parses");
        assert_eq!(value_literal(&value), r#"("a\"b": 1)"#);
    }

    #[test]
    fn non_string_keys_use_their_yaml_scalar_form() {
        assert_eq!(literal_of("1: a"), "(\"1\": \"a\")");
        assert_eq!(literal_of("true: a"), "(\"true\": \"a\")");
        assert_eq!(literal_of("null: a"), "(\"null\": \"a\")");
    }

    #[test]
    fn nested_mapping_snapshot() {
        let literal = literal_of(
            r#"
            title: Example
            tags:
              - area/work
              - programming/rust
            meta:
              draft: true
              revision: 3
            "#,
        );
        insta::assert_snapshot!(
            literal,
            @r#"("title": "Example", "tags": ("area/work", "programming/rust"), "meta": ("draft": true, "revision": 3))"#
        );
    }

    // =====================================================================
    // Tagged values
    // =====================================================================

    #[test]
    fn tagged_value_is_preserved_as_a_string() {
        // The tag survives textually rather than being dropped or executed.
        assert_eq!(literal_of("!degrees 90"), "\"!degrees 90\"");
    }

    // =====================================================================
    // Adversarial content and deep nesting
    // =====================================================================

    #[test]
    fn adversarial_strings_survive_as_string_content() {
        // Typst-significant characters, quotes, and backslashes must all stay
        // literal string content, never structure.
        let value: Value =
            serde_yaml_ng::from_str(r#"payload: "a\\b \"c\" #d [e] $f$ `g` @h""#).expect("parses");
        insta::assert_snapshot!(
            value_literal(&value),
            @r#"("payload": "a\\b \"c\" #d [e] $f$ `g` @h")"#
        );
        // And the whole thing must parse when spliced into code context.
        assert_parses(&value_literal(&value));
    }

    #[test]
    fn deeply_nested_structure_translates_and_parses() {
        let literal = literal_of(
            r#"
            level1:
              - a
              - level2:
                  level3:
                    - deep: [1, 2]
                      more: null
            "#,
        );
        insta::assert_snapshot!(
            literal,
            @r#"("level1": ("a", ("level2": ("level3": (("deep": (1, 2), "more": none),)))))"#
        );
        assert_parses(&literal);
    }

    /// Every one of the seven `Value` variants, spliced together and checked
    /// for a clean parse in code context. This is the injection guardrail:
    /// whatever the frontmatter holds, the literal is inert Typst.
    #[test]
    fn all_variants_together_parse_cleanly() {
        let literal = literal_of(
            r#"
            a_null: null
            a_bool: true
            an_int: -5
            a_float: 3.14
            a_string: 'quote " and backslash \'
            a_sequence: [1, two, false]
            a_mapping: { nested: value }
            "#,
        );
        assert_parses(&literal);
    }
}
