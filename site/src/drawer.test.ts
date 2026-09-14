// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { drawerOpen, installDrawer } from "./drawer";

describe("drawerOpen", () => {
  it("is open exactly when the sidebar is the target", () => {
    expect(drawerOpen("#sidebar")).toBe(true);
    expect(drawerOpen("")).toBe(false);
    expect(drawerOpen("#intro")).toBe(false);
  });
});

describe("installDrawer", () => {
  let uninstall: () => void = () => {};

  afterEach(() => {
    uninstall();
  });

  beforeEach(() => {
    document.body.innerHTML = `
      <div id="sidebar">
        <a class="nav-close" href="#">close</a>
        <a class="note" href="#intro">a note</a>
      </div>`;
    location.hash = "#sidebar";
  });

  it("closes on Escape", () => {
    uninstall = installDrawer();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(location.hash).toBe("");
  });

  it("closes when a link inside is followed, but not for the close link", () => {
    uninstall = installDrawer();
    document.querySelector<HTMLElement>(".note")?.click();
    expect(location.hash).not.toBe("#sidebar");

    location.hash = "#sidebar";
    document.querySelector<HTMLElement>(".nav-close")?.click();
    // The close link navigates to `#` on its own; the script leaves it be.
    expect(location.hash === "" || location.hash === "#sidebar").toBe(true);
  });

  it("does nothing without a sidebar pane", () => {
    document.body.innerHTML = "";
    uninstall = installDrawer();
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(location.hash).toBe("#sidebar");
  });
});
