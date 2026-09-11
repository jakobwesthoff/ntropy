// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The search panel (ADR 0052), mirroring the CLI's two layers: a query in
// the language the CLI speaks selects notes, then a second box narrows the
// results the way the picker does, fuzzily over date, title, and tags.

import { render } from "preact";
import { useEffect, useMemo, useState } from "preact/hooks";

import { loadSearchData } from "./data";
import { type Compiled, compile, matches, type SearchNote } from "./eval";
import { narrow, rowText } from "./fuzzy";
import { QueryError } from "./query";

const RESULT_LIMIT = 50;

/** The outcome of compiling the typed query. */
export function compileQuery(
  input: string,
): { compiled: Compiled } | { error: string } | null {
  if (input.trim() === "") return null;
  try {
    return { compiled: compile(input) };
  } catch (error) {
    if (error instanceof QueryError) return { error: error.message };
    throw error;
  }
}

/** The notes a query selects, narrowed by the filter, newest first. */
export function results(
  notes: SearchNote[],
  compiled: Compiled,
  filter: string,
): SearchNote[] {
  const selected = notes.filter((note) => matches(compiled, note));
  return narrow(filter, selected, rowText);
}

interface Props {
  /** Where the search data script lives, relative to the page. */
  dataSrc: string;
  /** The `../` prefix that reaches the site root from the page. */
  prefix: string;
}

export function Search({ dataSrc, prefix }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("");
  const [notes, setNotes] = useState<SearchNote[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || notes !== null) return;
    loadSearchData(dataSrc).then(setNotes, (error: Error) =>
      setLoadError(error.message),
    );
  }, [open, notes, dataSrc]);

  const outcome = useMemo(() => compileQuery(query), [query]);
  const hits = useMemo(() => {
    if (notes === null || outcome === null || "error" in outcome) return [];
    return results(notes, outcome.compiled, filter);
  }, [notes, outcome, filter]);

  return (
    <div class={`search${open ? " open" : ""}`}>
      <button
        type="button"
        class="search-toggle"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        Search
      </button>
      {open && (
        <div class="search-panel" role="dialog" aria-label="Search notes">
          <input
            class="search-query"
            type="search"
            placeholder="tag:work and text:deadline"
            aria-label="Query"
            value={query}
            onInput={(event) =>
              setQuery((event.target as HTMLInputElement).value)
            }
            onKeyDown={(event) => {
              if (event.key === "Escape") setOpen(false);
            }}
          />
          <input
            class="search-filter"
            type="search"
            placeholder="narrow by title or tag"
            aria-label="Narrow"
            value={filter}
            onInput={(event) =>
              setFilter((event.target as HTMLInputElement).value)
            }
          />
          {loadError !== null && <p class="search-error">{loadError}</p>}
          {outcome !== null && "error" in outcome && (
            <p class="search-error">{outcome.error}</p>
          )}
          {notes === null && loadError === null && (
            <p class="search-status">Loading…</p>
          )}
          {notes !== null && outcome !== null && !("error" in outcome) && (
            <>
              <p class="search-status">
                {hits.length === 0
                  ? "No notes match."
                  : `${hits.length} ${hits.length === 1 ? "note" : "notes"}`}
              </p>
              <ul class="search-results">
                {hits.slice(0, RESULT_LIMIT).map((note) => (
                  <li key={note.id}>
                    <time dateTime={note.created}>{note.created}</time>{" "}
                    <a href={`${prefix}${note.page}`}>{note.title}</a>
                    {note.tags.length > 0 && (
                      <span class="tags">
                        {note.tags.map((tag) => (
                          <a
                            class="tag"
                            key={tag}
                            href={`${prefix}tags/${tag}/index.html`}
                          >
                            {tag}
                          </a>
                        ))}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>
      )}
    </div>
  );
}

/** Mount the panel into every `[data-search]` element on the page. */
export function installSearch(doc: Document = document): void {
  for (const mount of doc.querySelectorAll<HTMLElement>("[data-search]")) {
    const dataSrc = mount.dataset.search ?? "";
    const prefix = mount.dataset.prefix ?? "";
    render(<Search dataSrc={dataSrc} prefix={prefix} />, mount);
  }
}
