// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The search script of an exported site, built as its own classic script
// beside the page script (ADR 0057): the palette and the query evaluator,
// mounted on the page's search button. A page without the button, such as
// a standalone rendered note, never loads this file.

import { installSearch } from "./search/ui";

installSearch();
