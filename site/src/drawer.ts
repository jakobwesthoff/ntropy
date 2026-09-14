// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The navigation drawer on narrow screens. The drawer itself needs no
// script: the menu link targets `#sidebar` and the stylesheet shows the
// pane while it is the `:target`. This adds what links cannot: Escape
// closes the drawer, and following a link inside it closes it too, so the
// next page does not open with the drawer over it.

/** Whether the drawer is open: the sidebar pane is the location target. */
export function drawerOpen(hash: string): boolean {
  return hash === "#sidebar";
}

/**
 * Wire the drawer's keyboard and link behaviour. Returns a function that
 * removes the listeners again.
 */
export function installDrawer(doc: Document = document): () => void {
  const pane = doc.getElementById("sidebar");
  if (!pane) return () => {};

  const close = () => {
    if (drawerOpen(location.hash)) {
      // Replacing the hash keeps the back button from reopening the drawer.
      history.replaceState(null, "", location.pathname + location.search);
    }
  };

  const onKey = (event: KeyboardEvent) => {
    if (event.key === "Escape") close();
  };
  const onClick = (event: Event) => {
    const link = (event.target as Element).closest("a[href]");
    if (link && !link.classList.contains("nav-close")) close();
  };
  doc.addEventListener("keydown", onKey);
  pane.addEventListener("click", onClick);
  return () => {
    doc.removeEventListener("keydown", onKey);
    pane.removeEventListener("click", onClick);
  };
}
