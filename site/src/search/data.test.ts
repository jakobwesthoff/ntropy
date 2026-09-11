// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { loadSearchData, resetSearchData } from "./data";

const src = "../assets/search-data.js";

// A stand-in document whose head is a detached element: a script appended
// there never connects to the page, so nothing tries to fetch it and the
// test fires its load and error handlers itself.
const head = document.createElement("div");
const doc = {
  createElement: (tag: string) => document.createElement(tag),
  head,
} as unknown as Document;

function injected(): HTMLScriptElement[] {
  return Array.from(
    head.querySelectorAll<HTMLScriptElement>(`script[src='${src}']`),
  );
}

describe("loadSearchData", () => {
  beforeEach(() => {
    resetSearchData();
    head.innerHTML = "";
  });

  afterEach(() => {
    resetSearchData();
  });

  it("returns present data without injecting a script", async () => {
    window.__ntropySearch = { notes: [] };
    await expect(loadSearchData(src, doc)).resolves.toEqual([]);
    expect(injected()).toHaveLength(0);
  });

  it("injects the script once and resolves with what it assigned", async () => {
    const first = loadSearchData(src, doc);
    const second = loadSearchData(src, doc);
    expect(second).toBe(first);
    expect(injected()).toHaveLength(1);
    window.__ntropySearch = { notes: [] };
    injected()[0]?.onload?.(new Event("load"));
    await expect(first).resolves.toEqual([]);
  });

  it("rejects when the loaded script assigned nothing", async () => {
    const loading = loadSearchData(src, doc);
    injected()[0]?.onload?.(new Event("load"));
    await expect(loading).rejects.toThrow("did not load");
  });

  it("rejects a failed load and allows a retry", async () => {
    const loading = loadSearchData(src, doc);
    injected()[0]?.onerror?.(new Event("error"));
    await expect(loading).rejects.toThrow("could not be loaded");
    expect(loadSearchData(src, doc)).not.toBe(loading);
    expect(injected()).toHaveLength(2);
  });
});
