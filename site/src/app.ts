// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The page script of an exported site and of a standalone rendered note,
// built as one classic script (`docs/design/site-frontend.md`). Everything
// here is an addition to pages that are complete without it: the color
// scheme switch, the navigation drawer's keyboard handling, the outline
// tracking, and highlighting. Each installer finds its own markup or does
// nothing, which is what lets a page without a drawer run the same script.
// The search lives in its own script (ADR 0057), so a page that has no
// search carries none of its code.

import { installDrawer } from "./drawer";
import { installHighlighting } from "./highlight";
import { installOutline } from "./outline";
import { installThemeSwitch } from "./theme";

installThemeSwitch();
installDrawer();
installOutline();
void installHighlighting();
