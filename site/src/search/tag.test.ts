// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";

import {
  normalizeSegment,
  normalizeTag,
  segments,
  tagMatches,
  tagsOf,
} from "./tag";

describe("normalizeSegment", () => {
  it("lowercases, hyphenates, and drops punctuation", () => {
    expect(normalizeSegment("My First Note")).toBe("my-first-note");
    expect(normalizeSegment("C++ & Rust!")).toBe("c-rust");
    expect(normalizeSegment("  spaced   out  ")).toBe("spaced-out");
  });

  it("transliterates German and folds Latin accents", () => {
    expect(normalizeSegment("Über Größe und Spaß")).toBe(
      "ueber-groesse-und-spass",
    );
    expect(normalizeSegment("Café Ñandú")).toBe("cafe-nandu");
  });

  it("drops other non-ASCII characters", () => {
    expect(normalizeSegment("日本語 notes")).toBe("notes");
  });

  it("caps the length at a dash boundary", () => {
    const long = Array.from({ length: 20 }, (_, i) => `word${i}`).join("-");
    const out = normalizeSegment(long);
    expect(out.length).toBeLessThanOrEqual(72);
    expect(out.endsWith("-")).toBe(false);
    expect(long.startsWith(out)).toBe(true);
  });

  it("can come back empty", () => {
    expect(normalizeSegment("!!!")).toBe("");
  });
});

describe("segments and normalizeTag", () => {
  it("splits on slashes and drops empty segments", () => {
    expect(segments("Programming//Rust/")).toEqual(["programming", "rust"]);
    expect(normalizeTag("Area / Work")).toBe("area/work");
    expect(normalizeTag("///")).toBe("");
  });
});

describe("tagMatches", () => {
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
    expect(tagsOf(["Work", "work", "Programming/Rust", "", "!!!"])).toEqual([
      "work",
      "programming/rust",
    ]);
    expect(tagsOf("Solo")).toEqual(["solo"]);
    expect(tagsOf(undefined)).toEqual([]);
    expect(tagsOf([{ nested: true }, 3])).toEqual(["3"]);
  });
});
