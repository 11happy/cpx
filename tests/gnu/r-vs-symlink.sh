#!/bin/sh
# With -r (and no explicit dereference option) symlinks named on the
# command line are copied as symlinks: a dangling one must not be an
# error and a good one must produce a symlink, not a regular file.
#
# Inspired by GNU coreutils test: tests/cp/r-vs-symlink.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

echo abc > foo || exit 1
ln -s foo slink || exit 1
ln -s no-such-file no-file || exit 1

cpx -r no-file junk 2>/dev/null || fail=1
test -L junk || fail=1

cpx -r slink bar 2>/dev/null || fail=1
test -L bar || fail=1
test "$(readlink bar)" = foo || fail=1

exit $fail
