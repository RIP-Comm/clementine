#!/usr/bin/env bash
#
# Fetch the jsmolka gba-tests ROMs used by the accuracy harness.
#
# The ROMs are not vendored in this repo. This clones (or updates) them into a
# directory of your choice and tells you the env var to export so the tests can
# find them.
#
# Usage:
#   scripts/fetch-test-roms.sh [target-dir]
#
# Target directory resolution order:
#   1. the first argument
#   2. $CLEMENTINE_TEST_ROMS
#   3. ./.test-roms (gitignored by the *.gba rule)

set -euo pipefail

REPO_URL="https://github.com/jsmolka/gba-tests.git"

target="${1:-${CLEMENTINE_TEST_ROMS:-./.test-roms}}"

if [ -d "$target/.git" ]; then
    echo "Updating existing checkout in $target"
    git -C "$target" pull --ff-only
else
    echo "Cloning $REPO_URL into $target"
    git clone --depth 1 "$REPO_URL" "$target"
fi

abs_target="$(cd "$target" && pwd)"

echo
echo "Done. Export this so the harness can find the ROMs:"
echo
echo "  export CLEMENTINE_TEST_ROMS=\"$abs_target\""
echo
echo "A real BIOS is also required: put gba_bios.bin in the repo root, or set"
echo "  export CLEMENTINE_BIOS=/path/to/gba_bios.bin"
