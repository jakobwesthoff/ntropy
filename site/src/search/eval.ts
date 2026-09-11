// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Query evaluation over the notes the site embeds, mirroring
// `src/query/eval.rs` and `src/query/text_search.rs` (ADR 0052).
//
// `tag:` uses the sub-path rule, `field:` compares a scalar or list member
// exactly, and `text:` is a regular expression over the body with smart case:
// case-insensitive unless the pattern holds an uppercase literal. A pattern
// using a construct Rust's `regex` rejects (lookaround, backreferences) is
// refused here too, so a query that works on the site works in the CLI.

import { parse, type Query, QueryError } from "./query";
import { tagMatches, tagsOf } from "./tag";

/** A note as the site's search data carries it. */
export interface SearchNote {
  id: string;
  title: string;
  /** Site-relative page path. */
  page: string;
  created: string;
  /** Normalized tags. */
  tags: string[];
  /** Frontmatter as JSON; only string keys, tagged values as null. */
  frontmatter: Record<string, unknown>;
  body: string;
}

/** A note built from frontmatter and body the way the note parser does it. */
export function noteFrom(
  id: string,
  page: string,
  created: string,
  frontmatter: Record<string, unknown>,
  body: string,
): SearchNote {
  const title = frontmatter.title;
  return {
    id,
    title: typeof title === "string" ? title : "",
    page,
    created,
    tags: tagsOf(frontmatter.tags),
    frontmatter,
    body,
  };
}

export type Compiled =
  | { kind: "and"; left: Compiled; right: Compiled }
  | { kind: "or"; left: Compiled; right: Compiled }
  | { kind: "not"; operand: Compiled }
  | { kind: "tag"; value: string }
  | { kind: "field"; name: string; value: string }
  | { kind: "text"; regex: RegExp };

/** Parse and compile a query; throws a `QueryError`. */
export function compile(input: string): Compiled {
  return lower(parse(input));
}

function lower(query: Query): Compiled {
  switch (query.kind) {
    case "and":
      return {
        kind: "and",
        left: lower(query.left),
        right: lower(query.right),
      };
    case "or":
      return { kind: "or", left: lower(query.left), right: lower(query.right) };
    case "not":
      return { kind: "not", operand: lower(query.operand) };
    case "tag":
    case "field":
      return query;
    case "text":
      return { kind: "text", regex: textRegex(query.pattern) };
  }
}

export function matches(query: Compiled, note: SearchNote): boolean {
  switch (query.kind) {
    case "and":
      return matches(query.left, note) && matches(query.right, note);
    case "or":
      return matches(query.left, note) || matches(query.right, note);
    case "not":
      return !matches(query.operand, note);
    case "tag":
      return note.tags.some((tag) => tagMatches(query.value, tag));
    case "field":
      return fieldMatches(note, query.name, query.value);
    case "text":
      return query.regex.test(note.body);
  }
}

/** Exact frontmatter match: scalar equality, or membership for a list. */
function fieldMatches(note: SearchNote, name: string, value: string): boolean {
  if (!Object.hasOwn(note.frontmatter, name)) return false;
  const field = note.frontmatter[name];
  if (Array.isArray(field))
    return field.some((item) => scalarText(item) === value);
  return scalarText(field) === value;
}

/** A scalar's text as the CLI compares it; non-scalars have none. */
function scalarText(value: unknown): string | null {
  if (typeof value === "string") return value;
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number") return String(value);
  return null;
}

// ---------------------------------------------------------------------------
// Text patterns
// ---------------------------------------------------------------------------

const REJECTED: [RegExp, string][] = [
  [/\(\?=|\(\?!|\(\?<=|\(\?<!/, "look-around is not supported"],
  [/\\[1-9]|\\k<\w+>/, "backreferences are not supported"],
];

/** Compile a `text:` pattern with smart case and line-anchored `^`/`$`. */
export function textRegex(pattern: string): RegExp {
  for (const [construct, message] of REJECTED) {
    if (construct.test(pattern)) throw QueryError.pattern(pattern, message);
  }
  const { source, flags: inline } = leadingFlags(pattern);
  const insensitive = inline.has("i") || smartCaseInsensitive(source);
  const flags = `m${insensitive ? "i" : ""}${inline.has("s") ? "s" : ""}`;
  // Unicode mode makes `.` and classes match by code point as Rust does,
  // but it also refuses escapes Rust accepts (`\-` outside a class); such
  // a pattern falls back to the legacy mode rather than failing.
  try {
    return new RegExp(source, `${flags}u`);
  } catch {
    try {
      return new RegExp(source, flags);
    } catch (error) {
      throw QueryError.pattern(
        pattern,
        error instanceof Error ? error.message : String(error),
      );
    }
  }
}

/**
 * Inline flag groups at the start of a pattern, `(?i)` or `(?is)`, which
 * Rust accepts anywhere and JavaScript nowhere: the leading ones become
 * `RegExp` flags, since that is where they are written in practice. Only
 * `i` and `s` translate (`m` is always on); a flag group elsewhere, a
 * negated flag, or another letter is refused rather than misread.
 */
function leadingFlags(pattern: string): { source: string; flags: Set<string> } {
  const flags = new Set<string>();
  let source = pattern;
  const leading = /^\(\?([a-zA-Z-]+)\)/;
  for (;;) {
    const match = leading.exec(source);
    if (match === null) break;
    const letters = match[1] as string;
    for (const letter of letters) {
      if (letter === "i" || letter === "s") flags.add(letter);
      else if (letter !== "m") {
        throw QueryError.pattern(
          pattern,
          `the flag \`${letter}\` is not supported here`,
        );
      }
    }
    source = source.slice(match[0].length);
  }
  if (/\(\?[a-zA-Z-]*[a-zA-Z][a-zA-Z-]*\)/.test(source)) {
    throw QueryError.pattern(
      pattern,
      "inline flags are only supported at the start of the pattern",
    );
  }
  return { source, flags };
}

/**
 * Whether a smart-case search of `pattern` is case-insensitive: it holds at
 * least one literal character and no uppercase literal. Only literals count;
 * `\W`, `\p{Lu}`, and inline flags carry uppercase letters in their syntax
 * and contribute none.
 */
export function smartCaseInsensitive(pattern: string): boolean {
  let anyLiteral = false;
  let anyUpper = false;
  const chars = Array.from(pattern);
  let i = 0;
  const record = (ch: string) => {
    anyLiteral = true;
    if (ch !== ch.toLowerCase()) anyUpper = true;
  };
  while (i < chars.length) {
    const ch = chars[i] as string;
    if (ch === "\\") {
      const next = chars[i + 1];
      if (next === undefined) break;
      if (/[A-Za-z]/.test(next)) {
        // A class, anchor, or property escape; `\p{..}`/`\P{..}` and
        // `\x{..}` carry braces to skip.
        i += 2;
        if (
          (next === "p" || next === "P" || next === "x" || next === "u") &&
          chars[i] === "{"
        ) {
          while (i < chars.length && chars[i] !== "}") i += 1;
          i += 1;
        }
        continue;
      }
      // An escaped punctuation character is that literal.
      record(next);
      i += 2;
      continue;
    }
    if (ch === "(" && chars[i + 1] === "?") {
      // A group header never holds literals: a named group's ends at `>`,
      // a flag group's at `:` or `)`.
      i += 2;
      const named =
        chars[i] === "<" || (chars[i] === "P" && chars[i + 1] === "<");
      const end = named ? ">" : null;
      while (i < chars.length) {
        const c = chars[i];
        if (end === null ? c === ":" || c === ")" : c === end) break;
        i += 1;
      }
      i += 1;
      continue;
    }
    if (ch === "[") {
      // Inside a class, literals and range bounds count; escapes and the
      // closing bracket are handled like the outer level.
      i += 1;
      if (chars[i] === "^") i += 1;
      while (i < chars.length && chars[i] !== "]") {
        const c = chars[i] as string;
        if (c === "\\") {
          const next = chars[i + 1];
          if (next !== undefined && !/[A-Za-z]/.test(next)) record(next);
          i += 2;
          continue;
        }
        if (c !== "-") record(c);
        i += 1;
      }
      i += 1;
      continue;
    }
    if (ch === "{") {
      while (i < chars.length && chars[i] !== "}") i += 1;
      i += 1;
      continue;
    }
    if (!"()|.*+?^$".includes(ch)) record(ch);
    i += 1;
  }
  return anyLiteral && !anyUpper;
}
