// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { render } from "preact";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { resetSearchData } from "./data";
import { noteFrom, type SearchPage } from "./eval";
import { compileQuery, installSearch, isSearchShortcut, Palette } from "./ui";

const notes = [
  noteFrom(
    "01ARZ3NDEKTSV4RRFFQ69G5FA1",
    "notes/rust-tips.html",
    "2026-02-01",
    { title: "Rust Tips", tags: ["work/rust"] },
    "The borrow checker explained.\n",
  ),
  noteFrom(
    "01ARZ3NDEKTSV4RRFFQ69G5FA2",
    "notes/garden.html",
    "2026-01-01",
    { title: "Garden", tags: ["home"] },
    "tomatoes and rust on the spade\n",
  ),
];
const pages: SearchPage[] = [
  {
    kind: "tag",
    section: "tags",
    field: null,
    value: "work/rust",
    label: "rust",
    page: "tags/work/rust/index.html",
    count: 1,
  },
  {
    kind: "tag",
    section: "tags",
    field: null,
    value: "home",
    label: "home",
    page: "tags/home/index.html",
    count: 1,
  },
];

// Preact renders on a microtask and runs effects after the next animation
// frame; a state change inside an effect needs a second round before its
// outcome is in the document.
const frame = () =>
  new Promise((resolve) => requestAnimationFrame(() => setTimeout(resolve, 0)));
const flush = async () => {
  await frame();
  await frame();
};

const key = (init: KeyboardEventInit, target: EventTarget = document) => {
  const event = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    ...init,
  });
  target.dispatchEvent(event);
  return event;
};

const type = async (text: string) => {
  const input = document.querySelector<HTMLInputElement>(".palette-input");
  if (!input) throw new Error("the palette is not open");
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await flush();
  return input;
};

describe("compileQuery", () => {
  it("is null for blank input, an error for a bad query, a query otherwise", () => {
    expect(compileQuery("  ")).toBeNull();
    expect(compileQuery("(")).toHaveProperty("error");
    expect(compileQuery("tag:work")).toHaveProperty("compiled");
    // Under the reader's semantics a bare term is never a bad pattern.
    expect(compileQuery("c++ (")).toHaveProperty("error");
    expect(compileQuery('"c++ ("')).toHaveProperty("compiled");
  });
});

describe("isSearchShortcut", () => {
  it("is the bare slash outside a field, or Ctrl or Cmd with k", () => {
    expect(isSearchShortcut(new KeyboardEvent("keydown", { key: "/" }))).toBe(
      true,
    );
    expect(
      isSearchShortcut(
        new KeyboardEvent("keydown", { key: "k", ctrlKey: true }),
      ),
    ).toBe(true);
    expect(
      isSearchShortcut(
        new KeyboardEvent("keydown", { key: "K", metaKey: true }),
      ),
    ).toBe(true);
    expect(isSearchShortcut(new KeyboardEvent("keydown", { key: "k" }))).toBe(
      false,
    );
    expect(
      isSearchShortcut(
        new KeyboardEvent("keydown", { key: "/", altKey: true }),
      ),
    ).toBe(false);
    const input = document.createElement("input");
    document.body.appendChild(input);
    expect(isSearchShortcut(key({ key: "/" }, input))).toBe(false);
    expect(isSearchShortcut(key({ key: "k", ctrlKey: true }, input))).toBe(
      true,
    );
    input.remove();
  });
});

describe("Palette", () => {
  let navigate: ReturnType<typeof vi.fn<(href: string) => void>>;
  let root: HTMLElement;

  beforeEach(async () => {
    resetSearchData();
    window.__ntropySearch = { notes, pages };
    document.body.innerHTML =
      '<button class="search-toggle" data-search="../assets/search-data.js" data-prefix="../">Search</button>';
    root = document.createElement("div");
    document.body.appendChild(root);
    navigate = vi.fn<(href: string) => void>();
    render(
      <Palette
        dataSrc="../assets/search-data.js"
        prefix="../"
        navigate={navigate}
      />,
      root,
    );
    await flush();
  });

  afterEach(() => {
    render(null, root);
    resetSearchData();
    document.body.className = "";
  });

  it("opens from the button, the slash, and Ctrl+K, and closes on Escape", async () => {
    expect(document.querySelector(".palette")).toBeNull();
    document.querySelector<HTMLButtonElement>(".search-toggle")?.click();
    await flush();
    expect(document.querySelector(".palette")).not.toBeNull();
    expect(document.body.classList.contains("palette-open")).toBe(true);
    expect(document.activeElement?.classList.contains("palette-input")).toBe(
      true,
    );

    key({ key: "Escape" });
    await flush();
    expect(document.querySelector(".palette")).toBeNull();
    expect(document.body.classList.contains("palette-open")).toBe(false);

    key({ key: "/" });
    await flush();
    expect(document.querySelector(".palette")).not.toBeNull();
    key({ key: "Escape" });
    await flush();

    const ctrlK = key({ key: "k", ctrlKey: true });
    await flush();
    expect(ctrlK.defaultPrevented).toBe(true);
    expect(document.querySelector(".palette")).not.toBeNull();
  });

  it("closes on a click outside, not inside", async () => {
    key({ key: "/" });
    await flush();
    document.querySelector<HTMLElement>(".palette")?.click();
    await flush();
    expect(document.querySelector(".palette")).not.toBeNull();
    document.querySelector<HTMLElement>(".palette-backdrop")?.click();
    await flush();
    expect(document.querySelector(".palette")).toBeNull();
  });

  it("groups tag pages before notes and marks the matches", async () => {
    key({ key: "/" });
    await flush();
    expect(document.querySelector(".palette-hint")?.textContent).toContain(
      "A word matches",
    );
    await type("rust");
    const headings = Array.from(
      document.querySelectorAll(".palette-group h3"),
    ).map((h) => h.textContent?.replace(/\s+/g, " ").trim());
    expect(headings).toEqual(["Tags 1", "Notes 2"]);
    const hrefs = Array.from(document.querySelectorAll(".palette-row a")).map(
      (a) => a.getAttribute("href"),
    );
    // The tag page first, then the note with the title hit before the
    // note with only a body hit.
    expect(hrefs).toEqual([
      "../tags/work/rust/index.html",
      "../notes/rust-tips.html",
      "../notes/garden.html",
    ]);
    const marks = Array.from(document.querySelectorAll("mark")).map(
      (m) => m.textContent,
    );
    expect(marks).toContain("rust");
    expect(marks).toContain("Rust");
    expect(document.querySelector(".palette-count")?.textContent).toBe(
      "3 results",
    );
    expect(document.querySelector(".palette-snippet")?.textContent).toContain(
      "borrow checker",
    );
  });

  it("moves the selection with the arrows, wraps, and opens with Enter", async () => {
    key({ key: "/" });
    await flush();
    const input = await type("rust");
    const selectedHref = () =>
      document.querySelector(".is-selected a")?.getAttribute("href");
    expect(selectedHref()).toBe("../tags/work/rust/index.html");
    key({ key: "ArrowDown" }, input);
    await flush();
    expect(selectedHref()).toBe("../notes/rust-tips.html");
    key({ key: "ArrowUp" }, input);
    key({ key: "ArrowUp" }, input);
    await flush();
    expect(selectedHref()).toBe("../notes/garden.html");
    key({ key: "Enter" }, input);
    expect(navigate).toHaveBeenCalledWith("../notes/garden.html");
  });

  it("opens a result on click and reports a bad query", async () => {
    key({ key: "/" });
    await flush();
    await type("home");
    document.querySelector<HTMLAnchorElement>(".palette-row a")?.click();
    expect(navigate).toHaveBeenCalledWith("../tags/home/index.html");
    await type("(");
    expect(document.querySelector(".palette-error")?.textContent).toContain(
      "expected",
    );
    await type("zzz-nothing");
    expect(document.querySelector(".palette-hint")?.textContent).toBe(
      "Nothing matches.",
    );
  });
});

describe("installSearch", () => {
  afterEach(() => {
    resetSearchData();
    document.body.innerHTML = "";
  });

  it("mounts a palette root for the page's search button", () => {
    document.body.innerHTML =
      '<button class="search-toggle" data-search="../assets/search-data.js" data-prefix="../">Search</button>';
    installSearch();
    expect(document.querySelector(".palette-root")).not.toBeNull();
  });

  it("reports a data script that fails to load", async () => {
    document.body.innerHTML =
      '<button class="search-toggle" data-search="../assets/search-data.js" data-prefix="../">Search</button>';
    installSearch();
    await flush();
    document.querySelector<HTMLButtonElement>(".search-toggle")?.click();
    await flush();
    const script = document.querySelector<HTMLScriptElement>(
      "script[src='../assets/search-data.js']",
    );
    expect(script).not.toBeNull();
    script?.onerror?.(new Event("error"));
    await flush();
    expect(document.querySelector(".palette-error")?.textContent).toContain(
      "could not be loaded",
    );
  });
});
