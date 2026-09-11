// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The narrowing layer over query results (ADR 0052): the CLI's picker
// fuzzy-matches over rows of date, title, and tags, and so does this. A
// row matches when the typed characters occur in order; the score rewards
// runs of consecutive matches most, then matches at word starts.

/** The text a note is narrowed by: its date, title, and tags. */
export function rowText(note: {
  created: string;
  title: string;
  tags: string[];
}): string {
  return `${note.created} ${note.title} ${note.tags.join(" ")}`;
}

/**
 * The match score of `needle` against `haystack`, or null when the needle's
 * characters do not occur in order. Higher is better; an empty needle
 * matches everything with score 0. Case-insensitive.
 */
export function fuzzyScore(needle: string, haystack: string): number | null {
  const query = Array.from(needle.toLowerCase());
  if (query.length === 0) return 0;
  const text = Array.from(haystack.toLowerCase());
  let score = 0;
  let position = 0;
  let previous = -2;
  for (const ch of query) {
    if (ch === " ") continue;
    let found = -1;
    for (let i = position; i < text.length; i++) {
      if (text[i] === ch) {
        found = i;
        break;
      }
    }
    if (found === -1) return null;
    score += 1;
    if (found === previous + 1) score += 3;
    if (found === 0 || /[\s/_-]/.test(text[found - 1] as string)) score += 2;
    previous = found;
    position = found + 1;
  }
  return score;
}

/** The items matching `needle`, best score first, ties in input order. */
export function narrow<T>(
  needle: string,
  items: readonly T[],
  text: (item: T) => string,
): T[] {
  const scored: [number, number, T][] = [];
  items.forEach((item, index) => {
    const score = fuzzyScore(needle, text(item));
    if (score !== null) scored.push([score, index, item]);
  });
  scored.sort((a, b) => b[0] - a[0] || a[1] - b[1]);
  return scored.map(([, , item]) => item);
}
