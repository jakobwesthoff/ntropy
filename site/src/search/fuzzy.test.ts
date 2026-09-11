// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";

import { fuzzyScore, narrow, rowText } from "./fuzzy";

describe("fuzzyScore", () => {
  it("matches characters in order, case-insensitively", () => {
    expect(fuzzyScore("rt", "Rust Tips")).not.toBeNull();
    expect(fuzzyScore("tr", "Rust Tips")).toBeNull();
    expect(fuzzyScore("RUST", "rust")).not.toBeNull();
  });

  it("scores consecutive and word-start matches higher", () => {
    const consecutive = fuzzyScore("rust", "rust tips") as number;
    const scattered = fuzzyScore("rust", "r u s t") as number;
    expect(consecutive).toBeGreaterThan(scattered);
    const wordStart = fuzzyScore("t", "tips rust") as number;
    const midWord = fuzzyScore("s", "tips rust") as number;
    expect(wordStart).toBeGreaterThan(midWord);
  });

  it("matches everything with an empty needle and ignores spaces in it", () => {
    expect(fuzzyScore("", "anything")).toBe(0);
    expect(fuzzyScore("ru ti", "rust tips")).not.toBeNull();
  });
});

describe("narrow", () => {
  it("keeps matches, best first, ties in input order", () => {
    const items = ["alpha beta", "beta", "gamma", "ab"];
    expect(narrow("ab", items, (s) => s)).toEqual(["ab", "alpha beta"]);
    expect(narrow("", items, (s) => s)).toEqual(items);
  });
});

describe("rowText", () => {
  it("joins date, title, and tags", () => {
    expect(
      rowText({ created: "2026-01-01", title: "T", tags: ["a", "b/c"] }),
    ).toBe("2026-01-01 T a b/c");
  });
});
