// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";

import { parse, QueryError, tokenize } from "./query";

describe("tokenize", () => {
  it("splits words, colons, parentheses, and strings with positions", () => {
    expect(tokenize('(tag:work) "a b"')).toEqual([
      { kind: "lparen", pos: 0 },
      { kind: "word", value: "tag", pos: 1 },
      { kind: "colon", pos: 4 },
      { kind: "word", value: "work", pos: 5 },
      { kind: "rparen", pos: 9 },
      { kind: "string", value: "a b", pos: 11 },
    ]);
  });

  it("keeps Unicode letters, slashes, underscores, and hyphens in words", () => {
    expect(tokenize("Über/größe_x-y")).toEqual([
      { kind: "word", value: "Über/größe_x-y", pos: 0 },
    ]);
  });

  it("resolves quote and backslash escapes and keeps other backslashes", () => {
    expect(tokenize('"a\\"b\\\\c\\.d"')).toEqual([
      { kind: "string", value: 'a"b\\c\\.d', pos: 0 },
    ]);
  });

  it("reports the position of an unexpected character", () => {
    expect(() => tokenize("a!b")).toThrow(QueryError);
    try {
      tokenize("a!b");
    } catch (error) {
      expect((error as QueryError).position).toBe(1);
      expect((error as QueryError).message).toContain("quote it");
    }
  });

  it("reports an unterminated string at its opening quote", () => {
    try {
      tokenize('x "abc');
      expect.unreachable();
    } catch (error) {
      expect((error as QueryError).position).toBe(2);
    }
  });
});

describe("parse", () => {
  it("builds predicates", () => {
    expect(parse("tag:Work")).toEqual({ kind: "tag", value: "Work" });
    expect(parse("TEXT:foo")).toEqual({ kind: "text", pattern: "foo" });
    expect(parse("status:draft")).toEqual({
      kind: "field",
      name: "status",
      value: "draft",
    });
    expect(parse("bare")).toEqual({ kind: "term", text: "bare" });
    expect(parse('"a phrase"')).toEqual({ kind: "term", text: "a phrase" });
    expect(parse('status:"in progress"')).toEqual({
      kind: "field",
      name: "status",
      value: "in progress",
    });
  });

  it("applies precedence not > and > or with parentheses overriding", () => {
    expect(parse("a or b and not c")).toEqual({
      kind: "or",
      left: { kind: "term", text: "a" },
      right: {
        kind: "and",
        left: { kind: "term", text: "b" },
        right: { kind: "not", operand: { kind: "term", text: "c" } },
      },
    });
    expect(parse("(a or b) and c")).toEqual({
      kind: "and",
      left: {
        kind: "or",
        left: { kind: "term", text: "a" },
        right: { kind: "term", text: "b" },
      },
      right: { kind: "term", text: "c" },
    });
  });

  it("joins adjacent predicates with and only when asked", () => {
    expect(() => parse("a b")).toThrow(QueryError);
    expect(parse("a b", { implicitAnd: true })).toEqual({
      kind: "and",
      left: { kind: "term", text: "a" },
      right: { kind: "term", text: "b" },
    });
    expect(parse("a b or c d", { implicitAnd: true })).toEqual({
      kind: "or",
      left: {
        kind: "and",
        left: { kind: "term", text: "a" },
        right: { kind: "term", text: "b" },
      },
      right: {
        kind: "and",
        left: { kind: "term", text: "c" },
        right: { kind: "term", text: "d" },
      },
    });
    expect(parse('tag:x "a b" (c)', { implicitAnd: true }).kind).toBe("and");
    expect(parse("a not b", { implicitAnd: true })).toEqual({
      kind: "and",
      left: { kind: "term", text: "a" },
      right: { kind: "not", operand: { kind: "term", text: "b" } },
    });
  });

  it("treats a keyword before a colon as a field name", () => {
    expect(parse("or:x")).toEqual({ kind: "field", name: "or", value: "x" });
    expect(parse("a AND b").kind).toBe("and");
  });

  it("reports positioned errors", () => {
    const position = (input: string) => {
      try {
        parse(input);
        return null;
      } catch (error) {
        return (error as QueryError).position;
      }
    };
    expect(position("")).toBe(0);
    expect(position("(a")).toBe(2);
    expect(position("a)")).toBe(1);
    expect(position("tag:")).toBe(4);
    expect(position("a and")).toBe(5);
    // `and` in predicate position is a bare term; the error is the trailing `a`.
    expect(position("and a")).toBe(4);
    expect(position(":x")).toBe(0);
    expect(position("(a b")).toBe(3);
    expect(position("(a :")).toBe(4);
    expect(position("tag:(x)")).toBe(4);
    expect(position("tag")).toBe(null);
  });
});
