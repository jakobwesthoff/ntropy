// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { beforeEach, describe, expect, it } from "vitest";

import {
  effectiveTheme,
  installThemeToggle,
  STORAGE_KEY,
  toggled,
} from "./theme";

describe("effectiveTheme", () => {
  it("prefers an explicit choice over the system preference", () => {
    expect(effectiveTheme("light", true)).toBe("light");
    expect(effectiveTheme("dark", false)).toBe("dark");
  });

  it("falls back to the system preference", () => {
    expect(effectiveTheme(null, true)).toBe("dark");
    expect(effectiveTheme(undefined, false)).toBe("light");
    expect(effectiveTheme("garbage", true)).toBe("dark");
  });
});

describe("toggled", () => {
  it("flips between the two themes", () => {
    expect(toggled("dark")).toBe("light");
    expect(toggled("light")).toBe("dark");
  });
});

describe("installThemeToggle", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
    document.body.innerHTML = '<button class="theme-toggle">Theme</button>';
  });

  it("applies a stored choice on install", () => {
    localStorage.setItem(STORAGE_KEY, "dark");
    installThemeToggle();
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(
      document.querySelector(".theme-toggle")?.getAttribute("aria-pressed"),
    ).toBe("true");
  });

  it("leaves the system preference alone without a stored choice", () => {
    installThemeToggle();
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it("switches the theme and remembers it on click", () => {
    installThemeToggle();
    const button = document.querySelector<HTMLElement>(".theme-toggle")!;
    button.click();
    const first = document.documentElement.dataset.theme;
    expect(first === "dark" || first === "light").toBe(true);
    expect(localStorage.getItem(STORAGE_KEY)).toBe(first);
    button.click();
    expect(document.documentElement.dataset.theme).toBe(
      toggled(first as "dark" | "light"),
    );
  });
});
