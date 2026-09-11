// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";

import { currentHeading, markCurrent } from "./outline";

const headings = [
  { id: "intro", top: -300 },
  { id: "details", top: 40 },
  { id: "end", top: 900 },
];

describe("currentHeading", () => {
  it("picks the last heading above the reading line", () => {
    expect(currentHeading(headings, 100)).toBe("details");
    expect(currentHeading(headings, 0)).toBe("intro");
    expect(currentHeading(headings, 1000)).toBe("end");
  });

  it("falls back to the first heading before any has scrolled past", () => {
    expect(currentHeading([{ id: "a", top: 500 }], 100)).toBe("a");
  });

  it("is null without headings", () => {
    expect(currentHeading([], 100)).toBeNull();
  });
});

describe("markCurrent", () => {
  it("marks exactly the link for the id", () => {
    document.body.innerHTML =
      '<nav class="outline"><a href="#a">A</a><a href="#b%20c" aria-current="true">B c</a></nav>';
    const outline = document.querySelector(".outline")!;
    markCurrent(outline, "a");
    expect(
      outline.querySelector("a[href='#a']")?.getAttribute("aria-current"),
    ).toBe("true");
    expect(
      outline.querySelector("a[href='#b%20c']")?.hasAttribute("aria-current"),
    ).toBe(false);
    markCurrent(outline, "b c");
    expect(
      outline.querySelector("a[href='#b%20c']")?.getAttribute("aria-current"),
    ).toBe("true");
    markCurrent(outline, null);
    expect(outline.querySelectorAll("[aria-current]").length).toBe(0);
  });
});
