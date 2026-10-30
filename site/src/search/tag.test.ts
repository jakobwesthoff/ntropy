// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";

import {
  normalizeTag,
  normalizeTagSegment,
  segments,
  tagMatches,
  tagsOf,
} from "./tag";

describe("normalizeTagSegment", () => {
  it("lowercases and hyphenates whitespace", () => {
    expect(normalizeTagSegment("My First Note")).toBe("my-first-note");
    expect(normalizeTagSegment("  spaced   out  ")).toBe("spaced-out");
  });

  it("keeps punctuation as written", () => {
    expect(normalizeTagSegment("0.13")).toBe("0.13");
    expect(normalizeTagSegment("C++")).toBe("c++");
    expect(normalizeTagSegment("C#")).toBe("c#");
    expect(normalizeTagSegment("Node.JS")).toBe("node.js");
    expect(normalizeTagSegment("a,b")).toBe("a,b");
    expect(normalizeTagSegment("C++ & Rust!")).toBe("c++-&-rust!");
  });

  it("drops control characters and slashes", () => {
    expect(normalizeTagSegment("a\u001bb")).toBe("ab");
    expect(normalizeTagSegment("a\u007fb")).toBe("ab");
    expect(normalizeTagSegment("a/b")).toBe("ab");
  });

  it("transliterates German and folds Latin accents", () => {
    expect(normalizeTagSegment("Über Größe und Spaß")).toBe(
      "ueber-groesse-und-spass",
    );
    expect(normalizeTagSegment("Café Ñandú")).toBe("cafe-nandu");
  });

  it("drops other non-ASCII characters", () => {
    expect(normalizeTagSegment("日本語 notes")).toBe("notes");
  });

  it("caps the length at a dash boundary", () => {
    const long = Array.from({ length: 20 }, (_, i) => `word${i}`).join("-");
    const out = normalizeTagSegment(long);
    expect(out.length).toBeLessThanOrEqual(72);
    expect(out.endsWith("-")).toBe(false);
    expect(long.startsWith(out)).toBe(true);
  });

  it("can come back empty", () => {
    expect(normalizeTagSegment("\u001b")).toBe("");
  });
});

describe("segments and normalizeTag", () => {
  it("splits on slashes and drops empty segments", () => {
    expect(segments("Programming//Rust/")).toEqual(["programming", "rust"]);
    expect(normalizeTag("Area / Work")).toBe("area/work");
    expect(normalizeTag("///")).toBe("");
  });

  it("keeps a dotted segment whole", () => {
    expect(segments("milestone/0.13")).toEqual(["milestone", "0.13"]);
  });
});

describe("tagMatches", () => {
  it("tells a dotted tag from the undotted one", () => {
    expect(tagMatches("0.13", "milestone/0.13")).toBe(true);
    expect(tagMatches("013", "milestone/0.13")).toBe(false);
  });

  it("matches a contiguous run of whole segments", () => {
    expect(tagMatches("programming", "programming/rust")).toBe(true);
    expect(tagMatches("programming", "area/programming/cli")).toBe(true);
    expect(tagMatches("programming/rust", "area/programming/rust")).toBe(true);
    expect(tagMatches("area/rust", "area/programming/rust")).toBe(false);
    expect(tagMatches("prog", "programming")).toBe(false);
  });

  it("is case-insensitive through normalization and rejects an empty query", () => {
    expect(tagMatches("RUST", "programming/rust")).toBe(true);
    expect(tagMatches("", "programming")).toBe(false);
    expect(tagMatches("!!!", "programming")).toBe(false);
  });
});

describe("tagsOf", () => {
  it("accepts a list or a scalar, normalizes, and deduplicates", () => {
    expect(tagsOf(["Work", "work", "Programming/Rust", "", "///"])).toEqual([
      "work",
      "programming/rust",
    ]);
    expect(tagsOf("Solo")).toEqual(["solo"]);
    expect(tagsOf(undefined)).toEqual([]);
    expect(tagsOf([{ nested: true }, 3])).toEqual(["3"]);
  });
});
