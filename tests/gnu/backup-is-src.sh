#!/bin/sh
# When the backup name of the destination is the source itself
# (cp --backup=simple a~ a), cp must refuse rather than destroy the source.
#
# Inspired by GNU coreutils test: tests/cp/backup-is-src.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

echo a > a || exit 1
echo a-tilde > a~ || exit 1

cpx --backup=simple a~ a > out 2>&1 && fail=1

# source must be intact, destination unchanged
test "$(cat a~)" = a-tilde || fail=1
test "$(cat a)" = a || fail=1

exit $fail
