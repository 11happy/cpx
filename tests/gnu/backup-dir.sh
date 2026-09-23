#!/bin/sh
# -b must not create a backup of a destination directory when copying a
# directory over it, and a recursive --backup copy must be able to back up
# and replace a file nested inside an existing destination directory.
#
# Inspired by GNU coreutils test: tests/cp/backup-dir.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir x y || exit 1

cpx -a x y || fail=1

# no y/x~ must appear
cpx -a --backup=existing x y || fail=1
test -d y/x || fail=1
test -d y/x~ && fail=1

mkdir -p src/foo dst/foo || exit 1
echo new > src/foo/bar || exit 1
echo old > dst/foo/bar || exit 1
cpx --recursive --backup=existing src/* dst || fail=1
test "$(cat dst/foo/bar)" = new || fail=1
test "$(cat dst/foo/bar~)" = old || fail=1

exit $fail
