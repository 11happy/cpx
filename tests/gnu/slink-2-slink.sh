#!/bin/sh
# --update with --no-dereference where both source and destination are
# symlinks (to the same target, or both dangling) must not fail.
#
# Inspired by GNU coreutils test: tests/cp/slink-2-slink.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

touch file || exit 1
ln -s file a || exit 1
ln -s file b || exit 1
ln -s no-such-file c || exit 1
ln -s no-such-file d || exit 1

cpx --update --no-dereference a b || fail=1
cpx --update --no-dereference c d || fail=1
test -L b || fail=1
test -L d || fail=1

exit $fail
