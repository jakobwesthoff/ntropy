// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The color scheme switch (ADR 0054): three choices, system, light, and
// dark. The stylesheet follows the system preference by default and honors
// an explicit `data-theme` on the root element; a choice of light or dark
// sets that attribute and is remembered in `localStorage`, which the page
// head applies before the first paint. Choosing system removes both, so the
// page follows the OS again.

export type Theme = "light" | "dark";
export type Choice = Theme | "system";

export const STORAGE_KEY = "ntropy-theme";

/** The theme in effect: the explicit choice, else the system preference. */
export function effectiveTheme(
  explicit: string | null | undefined,
  prefersDark: boolean,
): Theme {
  if (explicit === "light" || explicit === "dark") return explicit;
  return prefersDark ? "dark" : "light";
}

/** The choice a stored value stands for; anything unexpected is system. */
export function choiceOf(stored: string | null | undefined): Choice {
  return stored === "light" || stored === "dark" ? stored : "system";
}

function readStored(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function store(choice: Choice): void {
  try {
    if (choice === "system") localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, choice);
  } catch {
    // Storage can be unavailable (privacy mode, file:// in some browsers);
    // the choice then lasts for the page only.
  }
}

/** Wire every `[data-theme-choice]` button on the page. */
export function installThemeSwitch(doc: Document = document): void {
  const root = doc.documentElement;
  const buttons = Array.from(
    doc.querySelectorAll<HTMLElement>("[data-theme-choice]"),
  );

  const apply = (choice: Choice) => {
    if (choice === "system") delete root.dataset.theme;
    else root.dataset.theme = choice;
    for (const button of buttons) {
      const pressed = button.dataset.themeChoice === choice;
      button.setAttribute("aria-pressed", pressed ? "true" : "false");
    }
  };

  apply(choiceOf(root.dataset.theme ?? readStored()));

  for (const button of buttons) {
    button.addEventListener("click", () => {
      const choice = choiceOf(button.dataset.themeChoice);
      apply(choice);
      store(choice);
    });
  }
}
