// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Shared Vite configuration: Preact's JSX transform for the sources, and
// the Vitest environment. The production build itself runs through
// `build.ts`, one invocation per output file, because the site loads only
// classic scripts and a classic-script (IIFE) bundle cannot be split.

import preact from "@preact/preset-vite";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [preact()],
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
