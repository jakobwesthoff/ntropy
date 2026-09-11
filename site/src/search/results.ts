// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What a search shows: the pages and notes a query selects under the
// reader's semantics, grouped by kind and ranked, each with the matched
// text marked and, for a note, one line of body around the first hit.

import {
  type Compiled,
  contains,
  matches,
  matchesPage,
  type SearchNote,
  type SearchPage,
  scalarValues,
} from "./eval";
import { normalizeTag } from "./tag";

/** A run of text, marked when it is what the query matched. */
export interface Fragment {
  text: string;
  mark: boolean;
}

export interface Hit {
  kind: "section" | "tag" | "group" | "note";
  href: string;
  title: Fragment[];
  /** The section a group belongs to: `by-status`. */
  section?: string;
  count?: number;
  created?: string;
  tags?: Fragment[][];
  snippet?: Fragment[];
}

export interface Results {
  tags: Hit[];
  groups: Hit[];
  notes: Hit[];
  /** Every match, before the per-group limits. */
  total: number;
}

export const LIMITS = { tags: 8, groups: 8, notes: 50 };

/** The characters of body shown around a hit. */
const SNIPPET_WIDTH = 150;

/**
 * The needles to mark: the texts of the query's positive predicates, since
 * what a negated one names is what is absent. `text:` patterns come as
 * regexes, the rest as substrings.
 */
export function needles(query: Compiled): {
  texts: string[];
  regexes: RegExp[];
} {
  const texts: string[] = [];
  const regexes: RegExp[] = [];
  const walk = (node: Compiled, negated: boolean) => {
    switch (node.kind) {
      case "and":
      case "or":
        walk(node.left, negated);
        walk(node.right, negated);
        return;
      case "not":
        walk(node.operand, !negated);
        return;
      case "term":
        if (!negated && node.text !== "") texts.push(node.text);
        return;
      case "tag":
        if (!negated) texts.push(normalizeTag(node.value));
        return;
      case "field":
        if (!negated && node.value !== "") texts.push(node.value);
        return;
      case "text":
        if (!negated) regexes.push(node.regex);
        return;
    }
  };
  walk(query, false);
  return { texts, regexes };
}

/** The `[start, end)` spans in `text` the needles match. */
function spans(
  text: string,
  { texts, regexes }: { texts: string[]; regexes: RegExp[] },
): [number, number][] {
  const found: [number, number][] = [];
  const lower = text.toLowerCase();
  for (const needle of texts) {
    const n = needle.toLowerCase();
    let at = lower.indexOf(n);
    while (at !== -1) {
      found.push([at, at + n.length]);
      at = lower.indexOf(n, at + n.length);
    }
  }
  for (const regex of regexes) {
    const global = new RegExp(regex.source, `${regex.flags.replace("g", "")}g`);
    for (const match of text.matchAll(global)) {
      if (match[0] === "") continue;
      found.push([match.index, match.index + match[0].length]);
    }
  }
  found.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const merged: [number, number][] = [];
  for (const span of found) {
    const last = merged[merged.length - 1];
    if (last && span[0] <= last[1]) last[1] = Math.max(last[1], span[1]);
    else merged.push([span[0], span[1]]);
  }
  return merged;
}

/** `text` split into marked and unmarked runs. */
export function mark(
  text: string,
  found: { texts: string[]; regexes: RegExp[] },
): Fragment[] {
  const out: Fragment[] = [];
  let at = 0;
  for (const [start, end] of spans(text, found)) {
    if (start > at) out.push({ text: text.slice(at, start), mark: false });
    out.push({ text: text.slice(start, end), mark: true });
    at = end;
  }
  if (at < text.length || out.length === 0) {
    out.push({ text: text.slice(at), mark: false });
  }
  return out;
}

/**
 * One line of the body around its first hit, marked; the opening of the
 * body when nothing in it matched (the hit was in the title or the tags).
 */
export function snippet(
  body: string,
  found: { texts: string[]; regexes: RegExp[] },
): Fragment[] {
  const flat = body.replace(/\s+/g, " ").trim();
  const first = spans(flat, found)[0];
  let start = 0;
  if (first) {
    start = Math.max(0, first[0] - Math.floor(SNIPPET_WIDTH / 3));
    // Start at a word boundary so the window does not open mid-word.
    const space = flat.lastIndexOf(" ", start);
    if (space > 0 && start - space < 20) start = space + 1;
  }
  const end = Math.min(flat.length, start + SNIPPET_WIDTH);
  const window = flat.slice(start, end);
  const fragments = mark(window, found);
  if (start > 0) fragments.unshift({ text: "… ", mark: false });
  if (end < flat.length) fragments.push({ text: " …", mark: false });
  return fragments;
}

/** How well a note answers the query, for the order of the note list. */
export function score(
  note: SearchNote,
  found: { texts: string[]; regexes: RegExp[] },
): number {
  let total = 0;
  for (const text of found.texts) {
    if (contains(note.title, text)) total += 4;
    if (note.tags.some((tag) => contains(tag, text))) total += 3;
    if (scalarValues(note.frontmatter).some((v) => contains(v, text)))
      total += 2;
    if (contains(note.body, text)) total += 1;
  }
  for (const regex of found.regexes) {
    if (regex.test(note.body)) total += 1;
  }
  return total;
}

/** The results of `query` over the data, links relative to `prefix`. */
export function search(
  data: { notes: SearchNote[]; pages: SearchPage[] },
  query: Compiled,
  prefix: string,
): Results {
  const found = needles(query);
  const pages = data.pages.filter((page) => matchesPage(query, page));
  const tags = pages.filter((page) => page.kind === "tag");
  const groups = pages.filter((page) => page.kind !== "tag");
  const byCount = (a: SearchPage, b: SearchPage) =>
    b.count - a.count || a.value.localeCompare(b.value);
  tags.sort(byCount);
  groups.sort(byCount);

  const selected = data.notes.filter((note) => matches(query, note, "reader"));
  const scored = selected.map((note) => ({ note, score: score(note, found) }));
  scored.sort(
    (a, b) => b.score - a.score || b.note.created.localeCompare(a.note.created),
  );

  const pageHit = (page: SearchPage): Hit => ({
    kind: page.kind,
    href: `${prefix}${page.page}`,
    title: mark(page.value, found),
    section: page.section,
    count: page.count,
  });
  return {
    tags: tags.slice(0, LIMITS.tags).map(pageHit),
    groups: groups.slice(0, LIMITS.groups).map(pageHit),
    notes: scored.slice(0, LIMITS.notes).map(({ note }) => ({
      kind: "note",
      href: `${prefix}${note.page}`,
      title: mark(note.title, found),
      created: note.created,
      tags: note.tags.map((tag) => mark(tag, found)),
      snippet: snippet(note.body, found),
    })),
    total: tags.length + groups.length + selected.length,
  };
}
