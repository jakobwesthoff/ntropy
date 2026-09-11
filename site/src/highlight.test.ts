// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import rust from "@shikijs/langs/rust";
import typescript from "@shikijs/langs/typescript";
import { afterEach, describe, expect, it } from "vitest";

import { installHighlighting, languageOf, resolveLanguage } from "./highlight";

describe("languageOf", () => {
  it("reads the language class and ignores others", () => {
    document.body.innerHTML =
      '<pre><code class="x language-rust y">a</code></pre>';
    expect(languageOf(document.querySelector("code")!)).toBe("rust");
    document.body.innerHTML = "<pre><code>a</code></pre>";
    expect(languageOf(document.querySelector("code")!)).toBeNull();
  });
});

describe("resolveLanguage", () => {
  const grammars = [
    { name: "typescript", aliases: ["ts"] },
    { name: "rust", aliases: ["rs"] },
  ];

  it("matches names and aliases case-insensitively", () => {
    expect(resolveLanguage("rust", grammars)).toBe("rust");
    expect(resolveLanguage("TS", grammars)).toBe("typescript");
    expect(resolveLanguage("rs", grammars)).toBe("rust");
  });

  it("is null for a language without a grammar", () => {
    expect(resolveLanguage("cobol", grammars)).toBeNull();
  });
});

describe("installHighlighting", () => {
  afterEach(() => {
    window.__ntropyGrammars = undefined;
  });

  it("replaces blocks that have a grammar and leaves the rest", async () => {
    window.__ntropyGrammars = [...rust, ...typescript];
    document.body.innerHTML = [
      '<pre><code class="language-rust">fn main() {}</code></pre>',
      '<pre><code class="language-cobol">DISPLAY.</code></pre>',
      "<pre><code>plain</code></pre>",
    ].join("");
    await installHighlighting();
    const pres = document.querySelectorAll("pre");
    expect(pres.length).toBe(3);
    expect(pres[0]?.classList.contains("shiki")).toBe(true);
    expect(pres[0]?.getAttribute("data-language")).toBe("rust");
    expect(pres[0]?.textContent).toBe("fn main() {}");
    // Both themes' colors ride along as custom properties.
    expect(pres[0]?.innerHTML).toContain("--shiki-dark");
    expect(pres[1]?.classList.contains("shiki")).toBe(false);
    expect(pres[2]?.classList.contains("shiki")).toBe(false);
  });

  it("does nothing without registered grammars", async () => {
    document.body.innerHTML = '<pre><code class="language-rust">x</code></pre>';
    await installHighlighting();
    expect(document.querySelector("pre")?.classList.contains("shiki")).toBe(
      false,
    );
  });
});
