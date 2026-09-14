// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Loading the search data on demand. The data is a classic script that
// assigns `window.__ntropySearch`; injecting a script element works over
// `file://`, where `fetch()` of a local file does not, and nothing is loaded
// until a reader searches.

import type { SearchNote, SearchPage } from "./eval";

/** What the export writes: the notes, and the site's own pages. */
export interface SearchData {
  notes: SearchNote[];
  pages: SearchPage[];
}

declare global {
  interface Window {
    __ntropySearch?: SearchData;
  }
}

let pending: Promise<SearchData> | null = null;

/** The search data, loaded once from `src` on first use. */
export function loadSearchData(
  src: string,
  doc: Document = document,
): Promise<SearchData> {
  if (window.__ntropySearch) return Promise.resolve(window.__ntropySearch);
  if (pending) return pending;
  pending = new Promise((resolve, reject) => {
    const script = doc.createElement("script");
    script.src = src;
    script.async = true;
    script.onload = () => {
      const data = window.__ntropySearch;
      if (data) resolve(data);
      else reject(new Error("the search data did not load"));
    };
    script.onerror = () => {
      pending = null;
      reject(new Error(`the search data at ${src} could not be loaded`));
    };
    doc.head.appendChild(script);
  });
  return pending;
}

/** Forget a loaded or failed attempt; for tests. */
export function resetSearchData(): void {
  pending = null;
  window.__ntropySearch = undefined;
}
