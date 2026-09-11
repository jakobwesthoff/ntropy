// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The light/dark toggle (ADR 0054). The stylesheet follows the system
// preference by default and honors an explicit `data-theme` on the root
// element; the toggle sets that attribute and remembers the choice in
// `localStorage`, which the page head applies before the first paint.

export type Theme = "light" | "dark";

export const STORAGE_KEY = "ntropy-theme";

/** The theme in effect: the explicit choice, else the system preference. */
export function effectiveTheme(
  explicit: string | null | undefined,
  prefersDark: boolean,
): Theme {
  if (explicit === "light" || explicit === "dark") return explicit;
  return prefersDark ? "dark" : "light";
}

/** The theme a click switches to. */
export function toggled(current: Theme): Theme {
  return current === "dark" ? "light" : "dark";
}

function readStored(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function store(theme: Theme): void {
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Storage can be unavailable (privacy mode, file:// in some browsers);
    // the choice then lasts for the page only.
  }
}

/** Wire every `.theme-toggle` button on the page. */
export function installThemeToggle(doc: Document = document): void {
  const root = doc.documentElement;
  const prefersDark = () =>
    typeof matchMedia === "function" &&
    matchMedia("(prefers-color-scheme: dark)").matches;
  const current = (): Theme =>
    effectiveTheme(root.dataset.theme ?? readStored(), prefersDark());

  const apply = (theme: Theme) => {
    root.dataset.theme = theme;
    for (const button of doc.querySelectorAll<HTMLElement>(".theme-toggle")) {
      button.setAttribute("aria-pressed", theme === "dark" ? "true" : "false");
    }
  };

  const stored = readStored();
  if (stored === "light" || stored === "dark") apply(stored);

  for (const button of doc.querySelectorAll<HTMLElement>(".theme-toggle")) {
    button.addEventListener("click", () => {
      const next = toggled(current());
      apply(next);
      store(next);
    });
  }
}
