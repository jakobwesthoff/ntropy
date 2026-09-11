// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The page script of an exported site, built as one classic script
// (`docs/design/site-frontend.md`). Everything here is an addition to pages
// that are complete without it: the theme toggle, the outline tracking,
// and highlighting.

import { installHighlighting } from "./highlight";
import { installOutline } from "./outline";
import { installThemeToggle } from "./theme";

installThemeToggle();
installOutline();
void installHighlighting();
