// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The conformance corpus shared with the Rust query module (ADR 0052):
// `tests/fixtures/query-corpus.json` must yield the same results here as in
// `tests/query_corpus.rs`.

import { describe, expect, it } from "vitest";

import corpus from "../../../tests/fixtures/query-corpus.json";
import { compile, matches, noteFrom } from "./eval";

interface Case {
  query: string;
  expect?: string[];
  error?: boolean;
}

const notes = corpus.notes.map((entry) =>
  noteFrom(
    entry.id,
    `notes/${entry.id}.html`,
    "2026-01-01",
    entry.frontmatter,
    entry.body,
  ),
);

describe("query corpus", () => {
  for (const testCase of corpus.cases as Case[]) {
    it(`${JSON.stringify(testCase.query)} → ${testCase.error ? "error" : JSON.stringify(testCase.expect)}`, () => {
      if (testCase.error) {
        expect(() => compile(testCase.query)).toThrow();
        return;
      }
      const compiled = compile(testCase.query);
      const got = notes
        .filter((note) => matches(compiled, note))
        .map((note) => note.id)
        .sort();
      expect(got).toEqual([...(testCase.expect ?? [])].sort());
    });
  }
});
