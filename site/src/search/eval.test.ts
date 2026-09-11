// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";

import {
  compile,
  matches,
  noteFrom,
  smartCaseInsensitive,
  textRegex,
} from "./eval";
import { QueryError } from "./query";

const note = noteFrom(
  "01ARZ3NDEKTSV4RRFFQ69G5FAV",
  "notes/x.html",
  "2026-01-01",
  {
    title: "Rust Tips",
    tags: ["Programming/Rust"],
    status: "draft",
    n: 2,
    list: [1, "b"],
  },
  "Body with Tokens.\nsecond line\n",
);

describe("smartCaseInsensitive", () => {
  it("is insensitive for lowercase literals and sensitive for uppercase ones", () => {
    expect(smartCaseInsensitive("foo")).toBe(true);
    expect(smartCaseInsensitive("Foo")).toBe(false);
    expect(smartCaseInsensitive("foo Bar")).toBe(false);
  });

  it("ignores class shorthands, properties, flags, and quantifiers", () => {
    expect(smartCaseInsensitive("foo\\W")).toBe(true);
    expect(smartCaseInsensitive("\\p{Lu}foo")).toBe(true);
    expect(smartCaseInsensitive("(?i)foo")).toBe(true);
    expect(smartCaseInsensitive("(?P<Name>foo)")).toBe(true);
    expect(smartCaseInsensitive("a{2,3}")).toBe(true);
    expect(smartCaseInsensitive("\\Bfoo")).toBe(true);
  });

  it("counts literals inside classes and escaped punctuation", () => {
    expect(smartCaseInsensitive("[A-Z]")).toBe(false);
    expect(smartCaseInsensitive("[a-z]")).toBe(true);
    expect(smartCaseInsensitive("\\.")).toBe(true);
  });

  it("is sensitive without any literal", () => {
    expect(smartCaseInsensitive("\\w+")).toBe(false);
    expect(smartCaseInsensitive("^$")).toBe(false);
  });
});

describe("textRegex", () => {
  it("anchors to lines and applies smart case", () => {
    expect(textRegex("^second").test("first\nsecond")).toBe(true);
    expect(textRegex("tokens").test("With Tokens")).toBe(true);
    expect(textRegex("Tokens").test("with tokens")).toBe(false);
  });

  it("refuses constructs the CLI's regex engine rejects", () => {
    expect(() => textRegex("a(?=b)")).toThrow(QueryError);
    expect(() => textRegex("a(?<!b)")).toThrow(QueryError);
    expect(() => textRegex("(a)\\1")).toThrow(QueryError);
    expect(() => textRegex("(?<x>a)\\k<x>")).toThrow(QueryError);
  });

  it("reports an invalid pattern as a pattern error without a position", () => {
    try {
      textRegex("[");
      expect.unreachable();
    } catch (error) {
      expect(error).toBeInstanceOf(QueryError);
      expect((error as QueryError).position).toBeNull();
      expect((error as QueryError).message).toContain("invalid search pattern");
    }
  });

  it("refuses flags JavaScript lacks and inline flags after the start", () => {
    expect(() => textRegex("(?x)a b")).toThrow("the flag `x` is not supported");
    expect(() => textRegex("a(?i)b")).toThrow("only supported at the start");
    expect(textRegex("(?i)(?s)a.b").test("A\nB")).toBe(true);
  });

  it("keeps escaped punctuation inside classes out of the smart-case check", () => {
    // `\-` inside the class is punctuation, not a cased letter: the
    // pattern stays case-insensitive.
    expect(textRegex("[a\\-z]").test("Q")).toBe(false);
    expect(textRegex("[\\.x]").test("X")).toBe(true);
  });

  it("falls back from Unicode mode for escapes Rust accepts", () => {
    expect(textRegex("a\\-b").test("a-b")).toBe(true);
  });
});

describe("matches", () => {
  it("evaluates every predicate kind", () => {
    expect(matches(compile("tag:programming"), note)).toBe(true);
    expect(matches(compile("status:draft"), note)).toBe(true);
    expect(matches(compile("n:2"), note)).toBe(true);
    expect(matches(compile("list:b"), note)).toBe(true);
    expect(matches(compile("list:1"), note)).toBe(true);
    expect(matches(compile("title:Rust"), note)).toBe(false);
    expect(matches(compile("tokens"), note)).toBe(true);
    expect(matches(compile("not tokens"), note)).toBe(false);
    expect(matches(compile("tokens and status:done or tag:rust"), note)).toBe(
      true,
    );
  });

  it("ignores prototype properties as fields", () => {
    expect(matches(compile("constructor:x"), note)).toBe(false);
    expect(matches(compile("toString:x"), note)).toBe(false);
  });
});

describe("noteFrom", () => {
  it("derives title and normalized tags from the frontmatter", () => {
    expect(note.title).toBe("Rust Tips");
    expect(note.tags).toEqual(["programming/rust"]);
    const untitled = noteFrom("x", "p", "d", { tags: "One" }, "");
    expect(untitled.title).toBe("");
    expect(untitled.tags).toEqual(["one"]);
  });
});
