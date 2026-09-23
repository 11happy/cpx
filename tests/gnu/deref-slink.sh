#!/bin/sh
# Copying a regular file onto an existing symlink destination (with
# --no-dereference on the source side) must succeed and write through
# to the symlink's target.  GNU's -d is expressed as
# -P --preserve=links.
#
# Inspired by GNU coreutils test: tests/cp/deref-slink.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

echo data > f || exit 1
touch slink-target || exit 1
ln -s slink-target slink || exit 1

cpx -P --preserve=links f slink || fail=1
# the destination symlink remains and its target got the contents
test -L slink || fail=1
test "$(cat slink-target)" = data || fail=1

exit $fail
