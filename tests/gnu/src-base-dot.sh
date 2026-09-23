#!/bin/sh
# "cp -a ../x/. ." must copy the *contents* of x into the current
# directory rather than creating "./x"; with an empty x there is nothing
# to do, so no directory appears and -v prints nothing.
#
# Inspired by GNU coreutils test: tests/cp/src-base-dot.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir x y || exit 1
cd y || exit 1

cpx -v -a --backup=numbered ../x/. . > out 2>&1 || fail=1
test -s out && { cat out >&2; fail=1; }
test -e x && { echo "created ./x instead of copying contents" >&2; fail=1; }

# Same with a non-empty source: the file lands directly in "."
echo hi > ../x/f || exit 1
cpx -a ../x/. . 2>/dev/null || fail=1
test -f f || fail=1
test -e x && fail=1

exit $fail
