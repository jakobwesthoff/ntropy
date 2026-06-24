# 3. Flat single-vault storage layout

Date: 2026-06-24

## Status

Accepted

## Context

Frontmatter-driven filtering and derived views are core. User-managed folders
would be a second, conflicting organizing principle and would make views
redundant.

## Decision

A single flat vault: one root directory in which notes are siblings.
Organization lives entirely in frontmatter; any hierarchy is a derived
projection (views).

ntropy manages one configured vault. A per-invocation override (flag and/or
env var, exact form TBD) points it elsewhere. There is no registry of named
vaults.

## Consequences

- The scanner is a single non-recursive read with no category-vs-note
  ambiguity. Note identity is global within the vault.
- Raw browsing in a file manager or `ls` degrades as the note count grows.
  Accepted: access is via ntropy and generated views, not raw browsing.
