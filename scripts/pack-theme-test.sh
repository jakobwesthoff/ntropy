#!/usr/bin/env bash
#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
#
# Packs the theme test vault into the two archives the documentation offers
# for download:
#
#   theme-test-note.tar.gz   the test note, the note it links to and their
#                            images, unpacked into the root of a reader's vault
#   theme-test-vault.tar.gz  the whole test vault as the directory theme-test/
#
# Usage: pack-theme-test.sh <test-vault> <out-dir>
#
# Both archives hold files only, no directory entries: the note archive is
# unpacked into a vault the reader already has, and GNU tar would otherwise
# give that vault's existing directories the modes stored in the archive.
# Every file is listed by path, so view directories, a theme the developer
# made locally and other untracked files stay out.

set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <test-vault> <out-dir>" >&2
  exit 2
fi

vault=$1
out=$2

# macOS tar adds a `._name` entry for a file's metadata unless
# COPYFILE_DISABLE is set, and stores extended attributes in the headers
# unless --no-xattrs is passed (below), which GNU tar warns about on
# extraction. GNU tar ignores the variable and accepts the option.
export COPYFILE_DISABLE=1

note_glob='*-theme-test-note.md'
companion_glob='*-theme-test-companion.md'

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

mkdir -p "$out"
out=$(cd "$out" && pwd)

# =========================================================
# The note archive
# =========================================================

(
  cd "$vault"
  find all-notes -maxdepth 1 -type f \( -name "$note_glob" -o -name "$companion_glob" \)
  find assets/theme-test -type f
) | LC_ALL=C sort > "$workdir/note.list"

if [ "$(grep -c '^all-notes/' "$workdir/note.list")" -ne 2 ]; then
  echo "$0: expected the test note and its companion in $vault/all-notes" >&2
  exit 1
fi

tar -czf "$out/theme-test-note.tar.gz" --no-xattrs --no-recursion \
  -C "$vault" -T "$workdir/note.list"

# =========================================================
# The vault archive
# =========================================================

# A symlink named like the directory the archive unpacks to gives every
# stored path its `theme-test/` prefix, which GNU and BSD tar would otherwise
# need different rename options for. tar stores the files it reads through
# the link, not the link.
ln -s "$(cd "$vault" && pwd)" "$workdir/theme-test"
(
  cd "$vault"
  find README.md .gitignore .ntropy/config.toml .ntropy/templates all-notes assets -type f
) | LC_ALL=C sort | sed 's|^|theme-test/|' > "$workdir/vault.list"

tar -czf "$out/theme-test-vault.tar.gz" --no-xattrs --no-recursion \
  -C "$workdir" -T "$workdir/vault.list"

# =========================================================
# Self-check
# =========================================================

for archive in theme-test-note.tar.gz theme-test-vault.tar.gz; do
  if tar -tzf "$out/$archive" | grep -q '\(^\|/\)\._'; then
    echo "$0: $archive holds macOS metadata entries" >&2
    exit 1
  fi
done

echo "Packed $out/theme-test-note.tar.gz and $out/theme-test-vault.tar.gz"
