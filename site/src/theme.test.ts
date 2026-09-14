// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { beforeEach, describe, expect, it } from "vitest";

import {
  choiceOf,
  effectiveTheme,
  installThemeSwitch,
  STORAGE_KEY,
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

describe("choiceOf", () => {
  it("reads light and dark, and treats anything else as system", () => {
    expect(choiceOf("light")).toBe("light");
    expect(choiceOf("dark")).toBe("dark");
    expect(choiceOf(null)).toBe("system");
    expect(choiceOf("garbage")).toBe("system");
  });
});

describe("installThemeSwitch", () => {
  const pressed = () =>
    Array.from(document.querySelectorAll("[data-theme-choice]"))
      .filter((b) => b.getAttribute("aria-pressed") === "true")
      .map((b) => (b as HTMLElement).dataset.themeChoice);
  const click = (choice: string) =>
    document
      .querySelector<HTMLButtonElement>(`[data-theme-choice="${choice}"]`)
      ?.click();

  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
    document.body.innerHTML = `
      <button data-theme-choice="system" aria-pressed="true"></button>
      <button data-theme-choice="light" aria-pressed="false"></button>
      <button data-theme-choice="dark" aria-pressed="false"></button>`;
  });

  it("applies a stored choice on install", () => {
    localStorage.setItem(STORAGE_KEY, "dark");
    installThemeSwitch();
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(pressed()).toEqual(["dark"]);
  });

  it("marks system pressed and leaves the root alone without a choice", () => {
    installThemeSwitch();
    expect(document.documentElement.dataset.theme).toBeUndefined();
    expect(pressed()).toEqual(["system"]);
  });

  it("switches and remembers a choice, and forgets it on system", () => {
    installThemeSwitch();
    click("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem(STORAGE_KEY)).toBe("light");
    expect(pressed()).toEqual(["light"]);

    click("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem(STORAGE_KEY)).toBe("dark");

    click("system");
    expect(document.documentElement.dataset.theme).toBeUndefined();
    expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
    expect(pressed()).toEqual(["system"]);
  });

  it("keeps working when storage throws", () => {
    const original = Storage.prototype.setItem;
    Storage.prototype.setItem = () => {
      throw new Error("blocked");
    };
    try {
      installThemeSwitch();
      click("dark");
      expect(document.documentElement.dataset.theme).toBe("dark");
    } finally {
      Storage.prototype.setItem = original;
    }
  });
});
