// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Code highlighting with Shiki in the browser (ADR 0053).
//
// A page includes one script per grammar it needs; each appends its
// grammar to `window.__ntropyGrammars`. The highlighter is created from
// that registry with Shiki's JavaScript regex engine and the two soft
// Gruvbox themes, whose warm token colors sit on the built-in theme's
// paper and warm black; every fenced block whose language has a grammar is
// replaced by Shiki's markup. Both themes' colors travel as CSS custom
// properties, and the stylesheet picks one per color scheme and keeps its
// own block background; a block whose language has no grammar stays as it
// is.

import {
  createHighlighterCore,
  type HighlighterCore,
  type LanguageRegistration,
} from "@shikijs/core";
import { createJavaScriptRegexEngine } from "@shikijs/engine-javascript";
import gruvboxDark from "@shikijs/themes/gruvbox-dark-soft";
import gruvboxLight from "@shikijs/themes/gruvbox-light-soft";

declare global {
  interface Window {
    __ntropyGrammars?: LanguageRegistration[];
  }
}

const LANGUAGE_CLASS = "language-";

/** The fence language a code element carries, or null. */
export function languageOf(code: Element): string | null {
  for (const name of code.classList) {
    if (name.startsWith(LANGUAGE_CLASS))
      return name.slice(LANGUAGE_CLASS.length);
  }
  return null;
}

/**
 * The grammar name a fence language resolves to among the registered
 * grammars: an exact name or one of a grammar's aliases.
 */
export function resolveLanguage(
  language: string,
  grammars: readonly Pick<LanguageRegistration, "name" | "aliases">[],
): string | null {
  const wanted = language.toLowerCase();
  for (const grammar of grammars) {
    if (grammar.name === wanted) return grammar.name;
  }
  for (const grammar of grammars) {
    if (grammar.aliases?.some((alias) => alias.toLowerCase() === wanted))
      return grammar.name;
  }
  return null;
}

export async function highlighterFor(
  grammars: LanguageRegistration[],
): Promise<HighlighterCore> {
  return createHighlighterCore({
    langs: [grammars],
    themes: [gruvboxLight, gruvboxDark],
    engine: createJavaScriptRegexEngine({ forgiving: true }),
  });
}

/** Highlight every fenced block on the page that has a grammar. */
export async function installHighlighting(
  doc: Document = document,
): Promise<void> {
  const grammars = window.__ntropyGrammars ?? [];
  if (grammars.length === 0) return;
  const blocks = Array.from(
    doc.querySelectorAll<HTMLElement>("pre > code[class*='language-']"),
  );
  if (blocks.length === 0) return;

  const highlighter = await highlighterFor(grammars);
  for (const code of blocks) {
    const language = languageOf(code);
    const lang = language === null ? null : resolveLanguage(language, grammars);
    const pre = code.parentElement;
    if (lang === null || pre === null) continue;
    const html = highlighter.codeToHtml(code.textContent ?? "", {
      lang,
      themes: { light: "gruvbox-light-soft", dark: "gruvbox-dark-soft" },
      defaultColor: false,
    });
    const template = doc.createElement("template");
    template.innerHTML = html;
    const replacement = template.content.firstElementChild;
    if (replacement === null) continue;
    replacement.setAttribute("data-language", lang);
    pre.replaceWith(replacement);
  }
}
