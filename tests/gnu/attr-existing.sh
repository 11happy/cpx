#!/bin/sh
# --attributes-only must not truncate or remove an existing destination,
# even when it is hard-linked or the source is a symlink; only
# --remove-destination may replace it.
#
# Inspired by GNU coreutils test: tests/cp/attr-existing.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

printf 1 > file1 || exit 1
printf 2 > file2 || exit 1
printf 2 > file2.exp || exit 1

cpx --attributes-only file1 file2 || fail=1
cmp file2 file2.exp || fail=1

# hard-linked destination keeps its data
ln file2 link2 || exit 1
cpx -a --attributes-only file1 file2 || fail=1
cmp file2 file2.exp || fail=1

# symlink source cannot replace a regular file: error, data kept
ln -s file1 sym1 || exit 1
cpx -a --attributes-only sym1 file2 2>/dev/null && fail=1
cmp file2 file2.exp || fail=1

# with --remove-destination the replacement is allowed
cpx -a --remove-destination --attributes-only sym1 file2 || fail=1
test -L file2 || fail=1
cmp file1 file2 || fail=1

exit $fail
