// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The search palette (ADR 0052): one box in an overlay over the page,
// opened by the header's search button, `/`, or Ctrl+K and Cmd+K, closed
// by Escape or a click outside. Results come grouped, tags and views before
// notes, with the matched text marked; the arrow keys move the selection
// and Enter opens it.
//
// Two inputs move the selection, the keys and the pointer, and they must
// not fight: a keyboard move scrolls the list, which puts a different row
// under a pointer that has not moved, and the browser reports that as the
// pointer entering the row. So hover selects only after the pointer has
// moved since the last keyboard move.

import { render } from "preact";
import { useEffect, useMemo, useRef, useState } from "preact/hooks";

import { loadSearchData, type SearchData } from "./data";
import { type Compiled, compile } from "./eval";
import { QueryError } from "./query";
import { type Fragment, type Hit, search } from "./results";

/** The outcome of compiling the typed query under the reader's semantics. */
export function compileQuery(
  input: string,
): { compiled: Compiled } | { error: string } | null {
  if (input.trim() === "") return null;
  try {
    return { compiled: compile(input, "reader") };
  } catch (error) {
    if (error instanceof QueryError) return { error: error.message };
    throw error;
  }
}

/**
 * Whether a keypress opens the palette: the bare `/` outside a field where
 * it would be typed, or `k` with Control or Command held anywhere.
 */
export function isSearchShortcut(event: KeyboardEvent): boolean {
  if ((event.ctrlKey || event.metaKey) && !event.altKey) {
    return event.key.toLowerCase() === "k";
  }
  if (event.key !== "/" || event.altKey) return false;
  const target = event.target;
  const inField =
    target instanceof Element &&
    target.closest("input, textarea, select, [contenteditable]") !== null;
  return !inField;
}

/** The sprite icon a result kind is drawn with. */
function iconOf(kind: Hit["kind"]): string {
  if (kind === "note") return "file-text";
  if (kind === "tag") return "tag";
  return "folder";
}

function Marked({ fragments }: { fragments: Fragment[] }) {
  return (
    <>
      {fragments.map((fragment, index) =>
        fragment.mark ? (
          <mark key={index}>{fragment.text}</mark>
        ) : (
          fragment.text
        ),
      )}
    </>
  );
}

interface Props {
  /** Where the search data script lives, relative to the page. */
  dataSrc: string;
  /** The `../` prefix that reaches the site root from the page. */
  prefix: string;
  /** Follows a chosen result; replaced in tests. */
  navigate?: (href: string) => void;
  /** The document holding the search buttons and receiving the shortcuts. */
  doc?: Document;
}

export function Palette({
  dataSrc,
  prefix,
  navigate = (href) => {
    window.location.href = href;
  },
  doc = document,
}: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [data, setData] = useState<SearchData | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState(0);
  const box = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  /** Whether the pointer has moved since the last keyboard move. */
  const pointerMoved = useRef(false);

  useEffect(() => {
    if (!open || data !== null) return;
    loadSearchData(dataSrc).then(setData, (error: Error) =>
      setLoadError(error.message),
    );
  }, [open, data, dataSrc]);

  // The page behind the overlay must not scroll while it is up.
  useEffect(() => {
    if (open) {
      doc.body.classList.add("palette-open");
      box.current?.focus();
    } else {
      doc.body.classList.remove("palette-open");
    }
    return () => doc.body.classList.remove("palette-open");
  }, [open, doc]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (isSearchShortcut(event)) {
        event.preventDefault();
        setOpen(true);
      } else if (event.key === "Escape") {
        setOpen(false);
      }
    };
    const onClick = () => setOpen(true);
    const buttons = Array.from(
      doc.querySelectorAll<HTMLElement>(".search-toggle"),
    );
    doc.addEventListener("keydown", onKey);
    for (const button of buttons) button.addEventListener("click", onClick);
    return () => {
      doc.removeEventListener("keydown", onKey);
      for (const button of buttons)
        button.removeEventListener("click", onClick);
    };
  }, [doc]);

  const outcome = useMemo(() => compileQuery(query), [query]);
  const results = useMemo(() => {
    if (data === null || outcome === null || "error" in outcome) return null;
    return search(data, outcome.compiled, prefix);
  }, [data, outcome, prefix]);
  const flat: Hit[] = useMemo(
    () =>
      results ? [...results.tags, ...results.groups, ...results.notes] : [],
    [results],
  );

  useEffect(() => setSelected(0), [query]);

  useEffect(() => {
    list.current
      ?.querySelector(".is-selected")
      ?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  // The list exists only while the palette is open; watch the pointer on
  // it from the render that creates it.
  useEffect(() => {
    const element = list.current;
    if (!element) return;
    const onMove = () => {
      pointerMoved.current = true;
    };
    element.addEventListener("mousemove", onMove);
    return () => element.removeEventListener("mousemove", onMove);
  }, [open]);

  const onInputKey = (event: KeyboardEvent) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (flat.length === 0) return;
      pointerMoved.current = false;
      const step = event.key === "ArrowDown" ? 1 : -1;
      setSelected((current) => (current + step + flat.length) % flat.length);
    } else if (event.key === "Enter") {
      const hit = flat[selected];
      if (hit) navigate(hit.href);
    }
  };

  if (!open) return null;

  let index = -1;
  const row = (hit: Hit) => {
    index += 1;
    const at = index;
    return (
      <li
        key={hit.href}
        class={`palette-row${at === selected ? " is-selected" : ""}`}
        onMouseEnter={() => {
          if (pointerMoved.current) setSelected(at);
        }}
      >
        <a
          href={hit.href}
          onClick={(event) => {
            event.preventDefault();
            navigate(hit.href);
          }}
        >
          <svg class="icon" aria-hidden="true">
            <use href={`#icon-${iconOf(hit.kind)}`} />
          </svg>
          <span class="palette-text">
            <span class="palette-title">
              <Marked fragments={hit.title} />
              {hit.kind === "group" && hit.section && (
                <span class="palette-section">{hit.section}</span>
              )}
            </span>
            {hit.kind === "note" ? (
              <>
                <span class="palette-meta">
                  <time dateTime={hit.created}>{hit.created}</time>
                  {hit.tags?.map((tag, i) => (
                    <span class="palette-tag" key={i}>
                      <Marked fragments={tag} />
                    </span>
                  ))}
                </span>
                {hit.snippet && hit.snippet.length > 0 && (
                  <span class="palette-snippet">
                    <Marked fragments={hit.snippet} />
                  </span>
                )}
              </>
            ) : (
              <span class="palette-meta">
                {hit.count} {hit.count === 1 ? "note" : "notes"}
              </span>
            )}
          </span>
        </a>
      </li>
    );
  };

  const section = (title: string, hits: Hit[]) =>
    hits.length > 0 && (
      <section class="palette-group">
        <h3>
          {title} <span class="count">{hits.length}</span>
        </h3>
        <ol>{hits.map(row)}</ol>
      </section>
    );

  return (
    // The backdrop is a button, so closing by a click outside is a real
    // control: focusable, labelled, and reachable without a pointer.
    <div class="palette-layer">
      <button
        type="button"
        class="palette-backdrop"
        aria-label="Close search"
        onClick={() => setOpen(false)}
      />
      <div class="palette" role="dialog" aria-label="Search">
        <div class="palette-box">
          <svg class="icon" aria-hidden="true">
            <use href="#icon-search" />
          </svg>
          <input
            ref={box}
            class="palette-input"
            type="search"
            placeholder="Search notes, tags, and views"
            aria-label="Search"
            autocomplete="off"
            spellcheck={false}
            value={query}
            onInput={(event) =>
              setQuery((event.target as HTMLInputElement).value)
            }
            onKeyDown={onInputKey}
          />
        </div>
        <div class="palette-results" ref={list}>
          {loadError !== null && <p class="palette-error">{loadError}</p>}
          {outcome !== null && "error" in outcome && (
            <p class="palette-error">{outcome.error}</p>
          )}
          {data === null && loadError === null && (
            <p class="palette-hint">Loading…</p>
          )}
          {data !== null && outcome === null && (
            <p class="palette-hint">
              A word matches titles, tags, fields, and text. <code>tag:</code>,{" "}
              <code>field:</code>, and <code>text:</code> narrow to one;{" "}
              <code>and</code>, <code>or</code>, <code>not</code>, and
              parentheses combine.
            </p>
          )}
          {results !== null && results.total === 0 && (
            <p class="palette-hint">Nothing matches.</p>
          )}
          {results !== null && (
            <>
              {section("Tags", results.tags)}
              {section("Views", results.groups)}
              {section("Notes", results.notes)}
            </>
          )}
        </div>
        <div class="palette-footer">
          <span class="palette-count">
            {results
              ? `${results.total} ${results.total === 1 ? "result" : "results"}`
              : ""}
          </span>
          <span class="palette-keys">
            <kbd>↑</kbd>
            <kbd>↓</kbd> move <kbd>↵</kbd> open <kbd>esc</kbd> close
          </span>
        </div>
      </div>
    </div>
  );
}

/**
 * Mount the palette for the page's `[data-search]` button. The palette
 * renders into its own container at the end of the body, so it overlays
 * the page whatever the header's stacking.
 */
export function installSearch(doc: Document = document): void {
  const mount = doc.querySelector<HTMLElement>("[data-search]");
  if (!mount) return;
  const root = doc.createElement("div");
  root.className = "palette-root";
  doc.body.appendChild(root);
  render(
    <Palette
      dataSrc={mount.dataset.search ?? ""}
      prefix={mount.dataset.prefix ?? ""}
      doc={doc}
    />,
    root,
  );
}
