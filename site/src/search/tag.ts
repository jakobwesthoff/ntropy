// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Tag normalization and the sub-path match, mirroring `src/text/slug.rs` and
// `src/text/tag.rs` (ADR 0023, ADR 0006). The shared conformance corpus pins
// the two implementations against each other (ADR 0052).

const MAX_SEGMENT_LEN = 72;

const TRANSLITERATIONS: Record<string, string> = {
  ä: "ae",
  ö: "oe",
  ü: "ue",
  ß: "ss",
  Ä: "Ae",
  Ö: "Oe",
  Ü: "Ue",
  à: "a",
  á: "a",
  â: "a",
  ã: "a",
  å: "a",
  ā: "a",
  ª: "a",
  À: "A",
  Á: "A",
  Â: "A",
  Ã: "A",
  Å: "A",
  Ā: "A",
  è: "e",
  é: "e",
  ê: "e",
  ë: "e",
  ē: "e",
  ė: "e",
  ę: "e",
  È: "E",
  É: "E",
  Ê: "E",
  Ë: "E",
  Ē: "E",
  ì: "i",
  í: "i",
  î: "i",
  ï: "i",
  ī: "i",
  į: "i",
  Ì: "I",
  Í: "I",
  Î: "I",
  Ï: "I",
  Ī: "I",
  ò: "o",
  ó: "o",
  ô: "o",
  õ: "o",
  ø: "o",
  ō: "o",
  Ò: "O",
  Ó: "O",
  Ô: "O",
  Õ: "O",
  Ø: "O",
  Ō: "O",
  ù: "u",
  ú: "u",
  û: "u",
  ū: "u",
  ů: "u",
  Ù: "U",
  Ú: "U",
  Û: "U",
  Ū: "U",
  ñ: "n",
  Ñ: "N",
  ç: "c",
  Ç: "C",
  ý: "y",
  ÿ: "y",
};

/** One character's ASCII transliteration; other non-ASCII drops out. */
function transliterate(ch: string): string {
  const mapped = TRANSLITERATIONS[ch];
  if (mapped !== undefined) return mapped;
  return ch.charCodeAt(0) < 0x80 ? ch : "";
}

/**
 * The slug normalization pipeline without the `untitled` fallback:
 * transliterate, lowercase, whitespace runs to `-`, keep `[a-z0-9-]`,
 * collapse and trim dashes, cap the length at a dash boundary.
 */
export function normalizeSegment(input: string): string {
  const transliterated = Array.from(input).map(transliterate).join("");
  const lowered = transliterated.toLowerCase();

  let spaced = "";
  let prevSpace = false;
  for (const ch of lowered) {
    if (/\s/.test(ch)) {
      if (!prevSpace) spaced += "-";
      prevSpace = true;
    } else {
      spaced += ch;
      prevSpace = false;
    }
  }

  const filtered = spaced.replace(/[^a-z0-9-]/g, "");
  const collapsed = filtered.replace(/-+/g, "-").replace(/^-|-$/g, "");
  return truncateAtBoundary(collapsed, MAX_SEGMENT_LEN);
}

function truncateAtBoundary(input: string, max: number): string {
  if (input.length <= max) return input;
  const window = input.slice(0, max);
  const cut = window.lastIndexOf("-");
  const kept = cut === -1 ? window : window.slice(0, cut);
  return kept.replace(/^-+|-+$/g, "");
}

/** A tag's normalized, non-empty segments. */
export function segments(tag: string): string[] {
  return tag
    .split("/")
    .map(normalizeSegment)
    .filter((segment) => segment.length > 0);
}

/** A tag's canonical `a/b/c` form; empty when nothing survives. */
export function normalizeTag(tag: string): string {
  return segments(tag).join("/");
}

/**
 * The sub-path rule: the query's segments occur as a contiguous run of full
 * segments anywhere in the candidate's. An empty query matches nothing.
 */
export function tagMatches(query: string, candidate: string): boolean {
  const needle = segments(query);
  if (needle.length === 0) return false;
  const haystack = segments(candidate);
  for (let start = 0; start + needle.length <= haystack.length; start++) {
    let same = true;
    for (let i = 0; i < needle.length; i++) {
      if (haystack[start + i] !== needle[i]) {
        same = false;
        break;
      }
    }
    if (same) return true;
  }
  return false;
}

/**
 * The tags a note carries, as the note parser lifts them from frontmatter:
 * a list or a single scalar, each normalized, empties dropped, duplicates
 * removed in order of first appearance.
 */
export function tagsOf(raw: unknown): string[] {
  const items = Array.isArray(raw)
    ? raw
    : raw === null || raw === undefined
      ? []
      : [raw];
  const tags: string[] = [];
  for (const item of items) {
    if (
      typeof item !== "string" &&
      typeof item !== "number" &&
      typeof item !== "boolean"
    ) {
      continue;
    }
    const tag = normalizeTag(String(item));
    if (tag.length > 0 && !tags.includes(tag)) tags.push(tag);
  }
  return tags;
}
