#!/usr/bin/env bash
#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# ntropy access/query benchmark harness.
#
# Generates a reproducible vault of a few thousand realistic notes, then uses
# `hyperfine` to measure every access and query pattern the CLI exposes and
# prints a structured comparison table. The whole run is self-contained: a
# throwaway vault is created in a temporary directory and removed on exit unless
# `--keep` is given.
#
# The corpus is produced by the `generate_vault` example (see
# `examples/generate_vault.rs`), which writes notes directly into `all-notes/`
# and emits a JSON manifest. The manifest carries the exact terms and hit counts
# the benchmark commands need, so this script never hardcodes assumptions about
# the generated content; change the generator and the benchmarks follow.
#
# Usage:
#   scripts/benchmark.sh [--notes N] [--seed S] [--runs R] [--warmup W]
#                        [--vault DIR] [--keep] [--export DIR]
#
#   --notes N     Number of notes to generate (default 3000).
#   --seed S      PRNG seed for the corpus (default 305419896). Same seed and
#                 note count reproduce a byte-identical vault.
#   --runs R      Force exactly R timed runs per command (default: hyperfine
#                 decides, with a floor of 10).
#   --warmup W    Warmup runs per command before timing (default 3).
#   --vault DIR   Generate into DIR instead of a temp dir. Implies --keep.
#   --keep        Do not delete the generated vault on exit.
#   --export DIR  Persist the hyperfine markdown and JSON exports into DIR.

set -euo pipefail

# =========================================================
# Configuration and argument parsing
# =========================================================

NOTES=3000
SEED=305419896
RUNS=""
WARMUP=3
VAULT=""
KEEP=0
EXPORT_DIR=""

die() {
    echo "error: $*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --notes) NOTES="${2:?--notes requires a value}"; shift 2 ;;
        --seed) SEED="${2:?--seed requires a value}"; shift 2 ;;
        --runs) RUNS="${2:?--runs requires a value}"; shift 2 ;;
        --warmup) WARMUP="${2:?--warmup requires a value}"; shift 2 ;;
        --vault) VAULT="${2:?--vault requires a value}"; KEEP=1; shift 2 ;;
        --keep) KEEP=1; shift ;;
        --export) EXPORT_DIR="${2:?--export requires a value}"; shift 2 ;;
        -h|--help) sed -n '8,33p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

# Resolve the repository root from this script's location so the harness works
# from any working directory.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

command -v hyperfine >/dev/null 2>&1 || die \
    "hyperfine not found. Install it with 'brew install hyperfine' or 'cargo install hyperfine'."
command -v jq >/dev/null 2>&1 || die "jq not found. Install it with 'brew install jq'."

# =========================================================
# Build the binary and the corpus generator
# =========================================================

echo "==> Building ntropy and the corpus generator (release)..."
cargo build --release --quiet --manifest-path "$REPO_ROOT/Cargo.toml" \
    --bin ntropy --example generate_vault

BIN="$REPO_ROOT/target/release/ntropy"
GENERATOR="$REPO_ROOT/target/release/examples/generate_vault"
[[ -x "$BIN" ]] || die "built binary missing at $BIN"
[[ -x "$GENERATOR" ]] || die "built generator missing at $GENERATOR"

# =========================================================
# Vault setup and cleanup
# =========================================================

# A temporary working directory holds the manifest and the hyperfine exports
# regardless of where the vault lives, so cleanup has a single root to remove.
WORK_DIR="$(mktemp -d)"

if [[ -z "$VAULT" ]]; then
    VAULT="$WORK_DIR/vault"
fi

cleanup() {
    if [[ "$KEEP" -eq 1 ]]; then
        echo "==> Keeping vault at $VAULT"
    else
        rm -rf "$VAULT"
    fi
    rm -rf "$WORK_DIR"
}
trap cleanup EXIT

echo "==> Initializing vault at $VAULT"
"$BIN" init --vault "$VAULT" >/dev/null

# A second view makes the reconcile benchmark representative of a real
# multi-view vault rather than the single default by-tag tree. A view per
# remaining field is added later for the view-count scaling benchmark.
"$BIN" view add by-status --field status --vault "$VAULT" >/dev/null

MANIFEST="$WORK_DIR/manifest.json"
echo "==> Generating $NOTES notes (seed $SEED)"
"$GENERATOR" --vault "$VAULT" --notes "$NOTES" --seed "$SEED" --manifest "$MANIFEST"

# Build the views once up front so the steady-state query benchmarks run against
# a fully materialized vault. The cost of this first build is measured on its
# own by the reconcile benchmark below.
echo "==> Building views (initial reconcile)"
"$BIN" reconcile --vault "$VAULT" >/dev/null

# =========================================================
# Read the corpus manifest
# =========================================================

read_manifest() { jq -r "$1" "$MANIFEST"; }

SAMPLE_ID="$(read_manifest '.sample_id')"
SAMPLE_SLUG="$(read_manifest '.sample_slug')"
TAG_SHALLOW="$(read_manifest '.tag_shallow')"
TAG_SHALLOW_HITS="$(read_manifest '.tag_shallow_hits')"
TAG_DEEP="$(read_manifest '.tag_deep')"
TAG_DEEP_HITS="$(read_manifest '.tag_deep_hits')"
FIELD_QUERY="$(read_manifest '.field_query')"
FIELD_HITS="$(read_manifest '.field_hits')"
TEXT_COMMON="$(read_manifest '.text_common')"
TEXT_COMMON_HITS="$(read_manifest '.text_common_hits')"
TEXT_RARE="$(read_manifest '.text_rare')"
TEXT_RARE_HITS="$(read_manifest '.text_rare_hits')"

# =========================================================
# Environment header
# =========================================================

NTROPY_VERSION="$("$BIN" --version 2>/dev/null | head -1)"
CPU="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
OS="$(uname -sr)"

echo
echo "========================================================================"
echo " ntropy benchmark"
echo "========================================================================"
echo " ntropy:     $NTROPY_VERSION"
echo " host:       $CPU"
echo " os:         $OS"
echo " corpus:     $NOTES notes, seed $SEED"
echo " selectivity:"
printf "   %-22s %s\n" "tag:$TAG_SHALLOW" "$TAG_SHALLOW_HITS hits"
printf "   %-22s %s\n" "tag:$TAG_DEEP" "$TAG_DEEP_HITS hits"
printf "   %-22s %s\n" "$FIELD_QUERY" "$FIELD_HITS hits"
printf "   %-22s %s\n" "text:$TEXT_COMMON" "$TEXT_COMMON_HITS hits"
printf "   %-22s %s\n" "text:$TEXT_RARE" "$TEXT_RARE_HITS hits"
echo "========================================================================"
echo

# =========================================================
# Benchmark command set
# =========================================================

# Every benchmarked command runs non-interactively (`-n`) against the generated
# vault. There is no `edit` benchmark: `edit` only opens the editor and syncs
# views on a TTY, and `-n` (and the piped stdout) forces the plain path, which is
# just a resolve-and-print already covered by the query rows. The view-sync cost
# `edit` would pay interactively is the same one `reconcile` and `delete` measure.

# Quote the binary and vault paths for the shell hyperfine spawns per command.
printf -v NT '%q -n --vault %q' "$BIN" "$VAULT"

# Named commands, in `name|command` form. Each name becomes a row in the
# comparison table; the command is the exact CLI invocation being timed.
COMMANDS=(
    "list-all|$NT search"
    "tag-shallow|$NT search tag:$TAG_SHALLOW"
    "tag-deep|$NT search tag:$TAG_DEEP"
    "field-equality|$NT search $FIELD_QUERY"
    "text-common|$NT search text:$TEXT_COMMON"
    "text-rare|$NT search text:$TEXT_RARE"
    "text-regex|$NT search 'text:\"roa.?map\"'"
    "combined-tag-and-text|$NT search 'tag:$TAG_SHALLOW and text:$TEXT_COMMON'"
    "tags-aggregate|$NT tags"
    "info-stats|$NT info"
    "reconcile|$NT reconcile"
)

# Assemble the hyperfine argument vector: shared options first, then a
# (--command-name, command) pair per benchmark.
HYPERFINE_ARGS=(--warmup "$WARMUP" --shell sh)
if [[ -n "$RUNS" ]]; then
    HYPERFINE_ARGS+=(--runs "$RUNS")
else
    HYPERFINE_ARGS+=(--min-runs 10)
fi

MARKDOWN_EXPORT="$WORK_DIR/results.md"
JSON_EXPORT="$WORK_DIR/results.json"
HYPERFINE_ARGS+=(--export-markdown "$MARKDOWN_EXPORT" --export-json "$JSON_EXPORT")

for entry in "${COMMANDS[@]}"; do
    name="${entry%%|*}"
    command="${entry#*|}"
    HYPERFINE_ARGS+=(--command-name "$name" "$command")
done

echo "==> Benchmarking access and query patterns"
hyperfine "${HYPERFINE_ARGS[@]}"

# =========================================================
# Destructive pattern: delete (measured in isolation)
# =========================================================

# `delete` removes a note, so it cannot share the comparison table with the
# repeatable read benchmarks. hyperfine's --prepare restores the sample note
# before every timed run, keeping the corpus size constant across iterations.
SAMPLE_FILE="$VAULT/all-notes/$SAMPLE_ID-$SAMPLE_SLUG.md"
SAMPLE_BACKUP="$WORK_DIR/sample-note.md"
if [[ -f "$SAMPLE_FILE" ]]; then
    cp "$SAMPLE_FILE" "$SAMPLE_BACKUP"
    printf -v RESTORE 'cp %q %q' "$SAMPLE_BACKUP" "$SAMPLE_FILE"

    DELETE_ARGS=(--warmup 1 --shell sh --prepare "$RESTORE")
    if [[ -n "$RUNS" ]]; then
        DELETE_ARGS+=(--runs "$RUNS")
    else
        DELETE_ARGS+=(--min-runs 10)
    fi
    DELETE_ARGS+=(--command-name "delete-one" "$NT delete $SAMPLE_ID -f")

    echo
    echo "==> Benchmarking delete (destructive, restored between runs)"
    hyperfine "${DELETE_ARGS[@]}"
fi

# =========================================================
# View-count scaling: reconcile with a view per field
# =========================================================

# The corpus carries `priority` and `codename` beyond the already-configured
# `tags` and `status`, so a view on each exercises reconcile against every
# grouping field the notes hold (`title` is the per-note identity, not a
# category, so it is skipped). Reconcile's cost scales with the view tree it must
# read, so this shows how it grows as views are added — the case the incremental
# sync is meant to keep cheap. Comparable to the 2-view `reconcile` row above.
echo
echo "==> Configuring a view for every remaining field (priority, codename)"
"$BIN" view add by-priority --field priority --vault "$VAULT" >/dev/null
"$BIN" view add by-codename --field codename --vault "$VAULT" >/dev/null
# Build the new trees once so the timed run measures a steady-state (no-op) sync,
# matching the `reconcile` row above.
"$BIN" reconcile --vault "$VAULT" >/dev/null

ALL_VIEWS_ARGS=(--warmup "$WARMUP" --shell sh)
if [[ -n "$RUNS" ]]; then
    ALL_VIEWS_ARGS+=(--runs "$RUNS")
else
    ALL_VIEWS_ARGS+=(--min-runs 10)
fi
ALL_VIEWS_ARGS+=(--command-name "reconcile-4-views" "$NT reconcile")

echo
echo "==> Benchmarking reconcile with 4 views (tags, status, priority, codename)"
hyperfine "${ALL_VIEWS_ARGS[@]}"

# =========================================================
# Persist exports if requested
# =========================================================

if [[ -n "$EXPORT_DIR" ]]; then
    mkdir -p "$EXPORT_DIR"
    cp "$MARKDOWN_EXPORT" "$EXPORT_DIR/results.md"
    cp "$JSON_EXPORT" "$EXPORT_DIR/results.json"
    cp "$MANIFEST" "$EXPORT_DIR/manifest.json"
    echo
    echo "==> Exports written to $EXPORT_DIR"
fi

echo
echo "==> Done"
