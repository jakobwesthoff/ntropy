// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";

import { compile, noteFrom, type SearchPage } from "./eval";
import { mark, needles, search, snippet } from "./results";

const notes = [
  noteFrom(
    "01ARZ3NDEKTSV4RRFFQ69G5FA1",
    "notes/rust-tips.html",
    "2026-02-01",
    { title: "Rust Tips", tags: ["work/rust"], status: "open" },
    "The borrow checker explained at length.\n",
  ),
  noteFrom(
    "01ARZ3NDEKTSV4RRFFQ69G5FA2",
    "notes/garden.html",
    "2026-01-01",
    { title: "Garden", tags: ["home"], status: "done" },
    `${"Soil first. ".repeat(20)}Then rust on the spade, then tomatoes.\n`,
  ),
  noteFrom(
    "01ARZ3NDEKTSV4RRFFQ69G5FA3",
    "notes/rusty.html",
    "2026-03-01",
    { title: "Rusty nails", tags: ["home"] },
    "Nothing else.\n",
  ),
];
const page = (
  kind: SearchPage["kind"],
  section: string,
  value: string,
  count: number,
  field: string | null = null,
): SearchPage => ({
  kind,
  section,
  field,
  value,
  label: value.split("/").pop() ?? value,
  page: `${section === "tags" ? "tags" : `views/${section}`}/${value}/index.html`,
  count,
});
const pages = [
  page("section", "tags", "tags", 3),
  page("tag", "tags", "work", 1),
  page("tag", "tags", "work/rust", 1),
  page("tag", "tags", "home", 2),
  page("section", "by-status", "by-status", 2, "status"),
  page("group", "by-status", "open", 1, "status"),
  page("group", "by-status", "done", 1, "status"),
];
const data = { notes, pages };

describe("needles", () => {
  it("collects positive terms, tags, and fields, and text patterns apart", () => {
    const found = needles(
      compile("rust tag:Work status:open text:soil", "reader"),
    );
    expect(found.texts).toEqual(["rust", "work", "open"]);
    expect(found.regexes.map((r) => r.source)).toEqual(["soil"]);
  });

  it("leaves out what a negation names", () => {
    const found = needles(
      compile("garden and not (rust or tag:home)", "reader"),
    );
    expect(found.texts).toEqual(["garden"]);
    expect(needles(compile("not not rust", "reader")).texts).toEqual(["rust"]);
  });
});

describe("mark", () => {
  it("marks every case-insensitive hit and merges overlaps", () => {
    expect(
      mark("Rust and rusty", { texts: ["rust", "rusty"], regexes: [] }),
    ).toEqual([
      { text: "Rust", mark: true },
      { text: " and ", mark: false },
      { text: "rusty", mark: true },
    ]);
    expect(mark("plain", { texts: ["x"], regexes: [] })).toEqual([
      { text: "plain", mark: false },
    ]);
    expect(mark("a1 b22", { texts: [], regexes: [/\d+/] })).toEqual([
      { text: "a", mark: false },
      { text: "1", mark: true },
      { text: " b", mark: false },
      { text: "22", mark: true },
    ]);
  });
});

describe("snippet", () => {
  it("opens near the first hit with ellipses and falls back to the start", () => {
    const body = notes[1]?.body ?? "";
    const near = snippet(body, { texts: ["rust"], regexes: [] });
    expect(near[0]?.text).toBe("… ");
    expect(near.some((f) => f.mark && f.text === "rust")).toBe(true);
    const text = near.map((f) => f.text).join("");
    expect(text).toContain("Then rust on the spade");
    expect(text.length).toBeLessThan(170);

    const start = snippet(body, { texts: ["zzz"], regexes: [] });
    expect(start[0]?.text.startsWith("Soil first.")).toBe(true);
    expect(start[start.length - 1]?.text).toBe(" …");
    expect(snippet("short", { texts: [], regexes: [] })).toEqual([
      { text: "short", mark: false },
    ]);
  });
});

describe("search", () => {
  it("groups pages and notes, ranks title hits first, and links with the prefix", () => {
    const results = search(data, compile("rust", "reader"), "../");
    expect(results.tags.map((h) => h.href)).toEqual([
      "../tags/work/rust/index.html",
    ]);
    expect(results.groups).toEqual([]);
    // Rusty nails (title, newest) and Rust Tips (title and tag) outrank the
    // body-only hit; the tag hit breaks the tie in favour of Rust Tips.
    expect(results.notes.map((h) => h.href)).toEqual([
      "../notes/rust-tips.html",
      "../notes/rusty.html",
      "../notes/garden.html",
    ]);
    expect(results.total).toBe(4);
    expect(results.notes[0]?.title).toEqual([
      { text: "Rust", mark: true },
      { text: " Tips", mark: false },
    ]);
    expect(results.notes[0]?.tags?.[0]).toEqual([
      { text: "work/", mark: false },
      { text: "rust", mark: true },
    ]);
  });

  it("finds tag pages by partial tag and group pages by field", () => {
    const partial = search(data, compile("tag:rus", "reader"), "");
    expect(partial.tags.map((h) => h.href)).toEqual([
      "tags/work/rust/index.html",
    ]);
    expect(partial.notes.map((h) => h.href)).toEqual(["notes/rust-tips.html"]);
    const group = search(data, compile("status:ope", "reader"), "");
    expect(
      group.groups.map((h) => ({
        href: h.href,
        section: h.section,
        count: h.count,
      })),
    ).toEqual([
      {
        href: "views/by-status/open/index.html",
        section: "by-status",
        count: 1,
      },
    ]);
    expect(group.notes.map((h) => h.href)).toEqual(["notes/rust-tips.html"]);
  });

  it("matches section pages by name and never pages by text patterns", () => {
    const sections = search(data, compile("tags", "reader"), "");
    expect(sections.tags).toEqual([]);
    expect(sections.groups.map((h) => h.href)).toEqual([
      "tags/tags/index.html",
    ]);
    const text = search(data, compile("text:soil", "reader"), "");
    expect(text.tags).toEqual([]);
    expect(text.groups).toEqual([]);
    expect(text.notes.map((h) => h.href)).toEqual(["notes/garden.html"]);
  });

  it("orders pages by note count and honours negation", () => {
    const home = search(data, compile("tag:home or tag:work", "reader"), "");
    expect(home.tags.map((h) => [h.href, h.count])).toEqual([
      ["tags/home/index.html", 2],
      ["tags/work/index.html", 1],
      ["tags/work/rust/index.html", 1],
    ]);
    const not = search(data, compile("not rust", "reader"), "");
    expect(not.notes).toEqual([]);
    expect(not.tags.map((h) => h.href)).toEqual([
      "tags/home/index.html",
      "tags/work/index.html",
    ]);
  });
});
