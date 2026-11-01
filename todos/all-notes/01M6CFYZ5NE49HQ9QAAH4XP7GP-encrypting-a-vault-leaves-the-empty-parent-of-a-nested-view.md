---
title: "Encrypting a vault leaves the empty parent of a nested view"
kind: bug
component: crypto
impact: low
origin: review
---
# Encrypting a vault leaves the empty parent of a nested view

`vault encrypt` removes the materialized view trees, since an encrypted
vault keeps no plaintext views. For a nested view such as `area/work` it
removes `area/work` but leaves the now empty `area/` in the vault root.

## Problem

Reproduced on 2026-11-01 by the docs consolidation's fact check: a vault
with the view `area/work`, then `vault encrypt -y`, leaves an empty
`area/` directory. The note "Encrypted vaults" says encrypt removes the
view trees. The removal deletes only each view's own directory.

## Suggested fix

After removing a view tree, remove each parent directory up to the vault
root while it is empty and was a view path component, never a directory
that holds anything else. Test with a nested view and with a parent that
also holds a user file (that one must stay).
