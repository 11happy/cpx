#!/bin/sh
# --link together with --no-dereference on a dangling symlink must
# succeed and produce a link to the symlink rather than failing to
# resolve its target.
#
# Inspired by GNU coreutils test: tests/cp/link-no-deref.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

ln -s no-such-file dangling-slink || exit 1

cpx --link --no-dereference dangling-slink d2 || fail=1
test -L d2 || fail=1
test "$(readlink d2)" = no-such-file || fail=1

exit $fail
