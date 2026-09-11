// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { resetSearchData } from "./data";
import { compile, noteFrom } from "./eval";
import { compileQuery, installSearch, isSearchShortcut, results } from "./ui";

const notes = [
  noteFrom(
    "01ARZ3NDEKTSV4RRFFQ69G5FA1",
    "notes/rust-tips.html",
    "2026-02-01",
    { title: "Rust Tips", tags: ["work/rust"] },
    "borrow checker\n",
  ),
  noteFrom(
    "01ARZ3NDEKTSV4RRFFQ69G5FA2",
    "notes/garden.html",
    "2026-01-01",
    { title: "Garden", tags: ["home"] },
    "tomatoes\n",
  ),
];

describe("compileQuery", () => {
  it("is null for blank input, an error for a bad query, a query otherwise", () => {
    expect(compileQuery("  ")).toBeNull();
    expect(compileQuery("(")).toHaveProperty("error");
    expect(compileQuery("tag:work")).toHaveProperty("compiled");
  });
});

describe("isSearchShortcut", () => {
  const key = (init: KeyboardEventInit, target?: Element) => {
    const event = new KeyboardEvent("keydown", init);
    if (target) {
      target.dispatchEvent(event);
    }
    return event;
  };

  it("is the bare slash outside a field", () => {
    expect(isSearchShortcut(key({ key: "/" }))).toBe(true);
    expect(isSearchShortcut(key({ key: "/", ctrlKey: true }))).toBe(false);
    expect(isSearchShortcut(key({ key: "a" }))).toBe(false);
    const input = document.createElement("input");
    document.body.appendChild(input);
    expect(isSearchShortcut(key({ key: "/", bubbles: true }, input))).toBe(
      false,
    );
    input.remove();
  });
});

describe("results", () => {
  it("selects by query and narrows fuzzily", () => {
    expect(
      results(notes, compile("tag:work or tag:home"), "").map((n) => n.title),
    ).toEqual(["Rust Tips", "Garden"]);
    expect(
      results(notes, compile("tag:work or tag:home"), "grdn").map(
        (n) => n.title,
      ),
    ).toEqual(["Garden"]);
    expect(results(notes, compile("tomatoes"), "").map((n) => n.title)).toEqual(
      ["Garden"],
    );
  });
});

describe("installSearch", () => {
  // Preact runs effects after the next animation frame; a macrotask after
  // that frame sees their outcome.
  const flush = () =>
    new Promise((resolve) =>
      requestAnimationFrame(() => setTimeout(resolve, 0)),
    );

  beforeEach(() => {
    resetSearchData();
    document.body.innerHTML =
      '<div data-search="../assets/search-data.js" data-prefix="../"></div>';
  });

  afterEach(() => {
    resetSearchData();
  });

  it("opens on the toggle and renders results from the loaded data", async () => {
    // The data is present already, so no script is injected.
    window.__ntropySearch = { notes };
    installSearch();
    const toggle = document.querySelector<HTMLButtonElement>(".search-toggle");
    expect(toggle).not.toBeNull();
    toggle?.click();
    await flush();

    const query = document.querySelector<HTMLInputElement>(".search-query");
    expect(query).not.toBeNull();
    if (query === null) return;
    query.value = "tag:work";
    query.dispatchEvent(new Event("input", { bubbles: true }));
    await flush();

    const links = Array.from(
      document.querySelectorAll(".search-results a"),
    ).map((a) => a.getAttribute("href"));
    expect(links).toContain("../notes/rust-tips.html");
    expect(links).toContain("../tags/work/rust/index.html");
    expect(document.querySelector(".search-status")?.textContent).toBe(
      "1 note",
    );

    query.value = "(";
    query.dispatchEvent(new Event("input", { bubbles: true }));
    await flush();
    expect(document.querySelector(".search-error")?.textContent).toContain(
      "expected",
    );

    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await flush();
    expect(document.querySelector(".search-panel")).toBeNull();

    // The slash shortcut opens the panel again with the query box focused.
    document.body.dispatchEvent(
      new KeyboardEvent("keydown", { key: "/", bubbles: true }),
    );
    await flush();
    expect(document.querySelector(".search-panel")).not.toBeNull();
    expect(document.activeElement?.classList.contains("search-query")).toBe(
      true,
    );
  });

  it("reports a data script that fails to load", async () => {
    installSearch();
    document.querySelector<HTMLButtonElement>(".search-toggle")?.click();
    await flush();
    const script = document.querySelector<HTMLScriptElement>(
      "script[src='../assets/search-data.js']",
    );
    expect(script).not.toBeNull();
    script?.onerror?.(new Event("error"));
    await flush();
    expect(document.querySelector(".search-error")?.textContent).toContain(
      "could not be loaded",
    );
  });
});
