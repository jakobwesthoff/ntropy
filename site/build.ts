// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Builds the browser-side files the ntropy binary embeds
// (`docs/design/site-frontend.md`, ADR 0051, ADR 0053).
//
// Every script is a classic script: the exported site works from `file://`,
// where module scripts and lazily loaded chunks are refused. The outputs
// land in `src/site/dist/` and are committed; `cargo build` never runs this.
//
//   app.js        the page script: theme toggle, outline tracking, search,
//                 and the highlighter runtime (Shiki's core, its JavaScript
//                 regex engine, the two themes), one self-contained IIFE
//   grammars.zst  every grammar the curated languages need, their embedded
//                 languages included, as one zstd-compressed JSON array; the
//                 export inflates it and writes the grammars a site uses

import { mkdir, rm } from "node:fs/promises";
import { resolve } from "node:path";
import preact from "@preact/preset-vite";
import { build } from "vite";

import grammars from "./grammars.json";

const HERE = import.meta.dirname;
const OUT = resolve(HERE, "../src/site/dist");

async function bundleApp(): Promise<void> {
  await build({
    configFile: false,
    logLevel: "warn",
    plugins: [preact()],
    build: {
      outDir: OUT,
      emptyOutDir: false,
      minify: true,
      sourcemap: false,
      lib: {
        entry: resolve(HERE, "src/app.ts"),
        name: "ntropy",
        formats: ["iife"],
        fileName: () => "app.js",
      },
    },
  });
}

/**
 * The curated grammars and, transitively, every grammar they embed, each
 * once, ordered by name so the blob is byte-stable across builds. A
 * language's module exports its own grammar last, after the grammars it
 * embeds, so taking every element of every module covers the closure.
 */
async function grammarClosure(): Promise<object[]> {
  const byName = new Map<string, object>();
  for (const lang of grammars) {
    const mod = (await import(`@shikijs/langs/${lang}`)) as {
      default: { name: string }[];
    };
    for (const grammar of mod.default) {
      byName.set(grammar.name, grammar);
    }
  }
  return [...byName.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([, g]) => g);
}

async function writeGrammars(): Promise<number> {
  const closure = await grammarClosure();
  const json = Buffer.from(JSON.stringify(closure));
  const compressed = Bun.zstdCompressSync(json, { level: 19 });
  await Bun.write(resolve(OUT, "grammars.zst"), compressed);
  return closure.length;
}

await rm(OUT, { recursive: true, force: true });
await mkdir(OUT, { recursive: true });
await bundleApp();
const count = await writeGrammars();
console.log(`built app.js and grammars.zst (${count} grammars) into ${OUT}`);
