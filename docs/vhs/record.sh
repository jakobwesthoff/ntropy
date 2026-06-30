#!/bin/bash
# Record the ntropy demo video.
# Run from anywhere: docs/vhs/record.sh
#
# This builds a throwaway demo vault under /tmp, seeds it with a small but
# coherent set of prepared notes, records demo.tape against it, and then tears
# the vault down again so the recording is fully reproducible.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VAULT="/tmp/ntropy-demo"

# ---------------------------------------------------------------------------
# Tooling preconditions
# ---------------------------------------------------------------------------
# The demo drives the real binaries, so they have to be on PATH. mkulid lets us
# pin each seeded note to a fixed creation date (ntropy derives the date from
# the ULID in the filename), which keeps the picker's date column stable across
# recordings instead of collapsing every note onto "today".
for tool in vhs ntropy mkulid; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: '$tool' is required but not on PATH" >&2
    exit 1
  fi
done

# ---------------------------------------------------------------------------
# Fresh vault
# ---------------------------------------------------------------------------
echo "==> Cleaning previous demo vault..."
rm -rf "$VAULT"

echo "==> Scaffolding vault..."
# init seeds .ntropy/, the default + today templates, and a by-tag view.
ntropy init "$VAULT" >/dev/null

# ---------------------------------------------------------------------------
# Prepared notes
# ---------------------------------------------------------------------------
# We write notes straight into all-notes/ (far faster than a `ntropy new` per
# note, and it lets us pin dates and tags exactly). A later `reconcile` fixes
# any slug drift and materialises the by-tag view.
echo "==> Seeding prepared notes..."
seed() {
  # seed <iso-datetime> <slug>   (note body comes from stdin)
  local dt="$1" slug="$2" ulid
  ulid="$(mkulid -l --datetime "$dt")"
  cat >"$VAULT/all-notes/${ulid}-${slug}.md"
}

seed "2026-05-12T09:14:00Z" "refactor-the-markdown-parser" <<'EOF'
---
title: Refactor the Markdown parser
tags: [work, rust]
status: in progress
---
# Refactor the Markdown parser

Split the tokenizer from the block assembler so inline parsing stops leaking into layout.
EOF

seed "2026-05-20T16:40:00Z" "borrow-checker-mental-model" <<'EOF'
---
title: Borrow checker mental model
tags: [rust, learning]
---
# Borrow checker mental model

Ownership is a tree, borrows are temporary edges, and the checker just refuses to let an edge outlive its node.
EOF

seed "2026-06-01T20:05:00Z" "the-rust-programming-language-ch-10" <<'EOF'
---
title: The Rust Programming Language, ch. 10
tags: [rust, reading]
---
# The Rust Programming Language, ch. 10

Generics, traits, and lifetimes are the same idea at three levels: abstract over types, over behaviour, over time.
EOF

seed "2026-06-10T11:25:00Z" "q3-roadmap-planning" <<'EOF'
---
title: Q3 roadmap planning
tags: [work, planning]
status: in progress
---
# Q3 roadmap planning

Ship the parser refactor first; everything else on the board depends on it landing.
EOF

seed "2026-06-15T18:30:00Z" "sourdough-hydration-experiments" <<'EOF'
---
title: Sourdough hydration experiments
tags: [cooking, learning]
---
# Sourdough hydration experiments

Pushing past 75% hydration finally opened up the crumb, at the cost of a much stickier shaping stage.
EOF

seed "2026-06-22T08:50:00Z" "async-runtimes-compared" <<'EOF'
---
title: Async runtimes compared
tags: [rust, reading]
---
# Async runtimes compared

Tokio wins on ecosystem; smol wins on how little of it you have to understand to get started.
EOF

echo "==> Building views..."
ntropy reconcile --vault "$VAULT" >/dev/null

# ---------------------------------------------------------------------------
# Record
# ---------------------------------------------------------------------------
echo "==> Recording demo..."
cd "$SCRIPT_DIR"
vhs demo.tape

# ---------------------------------------------------------------------------
# Teardown
# ---------------------------------------------------------------------------
echo "==> Cleaning up demo vault..."
rm -rf "$VAULT"

echo "==> Done. Outputs: docs/pages/assets/demo.webm and docs/pages/assets/demo.mp4"
