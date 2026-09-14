// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Outline tracking (ADR 0054): the outline entry of the heading the reader
// is at gets `aria-current`, updated as the page scrolls.

/**
 * The id of the heading the reader is at: the last heading whose top has
 * scrolled past the reading line, or the first heading when none has.
 * `tops` are the headings' offsets from the viewport top, in document order.
 */
export function currentHeading(
  headings: readonly { id: string; top: number }[],
  readingLine: number,
): string | null {
  let current: string | null = null;
  for (const heading of headings) {
    if (heading.top <= readingLine) {
      current = heading.id;
    } else {
      break;
    }
  }
  return current ?? headings[0]?.id ?? null;
}

/** Mark the outline link for `id` current, and no other. */
export function markCurrent(outline: Element, id: string | null): void {
  for (const link of outline.querySelectorAll<HTMLAnchorElement>(
    "a[href^='#']",
  )) {
    const target = decodeURIComponent(
      (link.getAttribute("href") ?? "").slice(1),
    );
    if (id !== null && target === id) {
      link.setAttribute("aria-current", "true");
    } else {
      link.removeAttribute("aria-current");
    }
  }
}

/** Keep the outline's current entry in step with the scroll position. */
export function installOutline(doc: Document = document): void {
  const outline = doc.querySelector(".outline");
  if (!outline) return;
  const headings = Array.from(
    doc.querySelectorAll<HTMLElement>(
      ".note-body h1[id], .note-body h2[id], .note-body h3[id], .note-body h4[id], .note-body h5[id], .note-body h6[id]",
    ),
  );
  if (headings.length === 0) return;

  const update = () => {
    const readingLine = Math.max(96, window.innerHeight * 0.2);
    const positions = headings.map((heading) => ({
      id: heading.id,
      top: heading.getBoundingClientRect().top,
    }));
    markCurrent(outline, currentHeading(positions, readingLine));
  };

  let scheduled = false;
  const onScroll = () => {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(() => {
      scheduled = false;
      update();
    });
  };
  window.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("resize", onScroll);
  update();
}
