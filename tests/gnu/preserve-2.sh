#!/bin/sh
# Check that a comma-separated --preserve=X,Y attribute list is accepted.
#
# Inspired by GNU coreutils test: tests/cp/preserve-2.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

touch f || exit 1

cpx --preserve=mode,links f g || fail=1
test -f g || fail=1

exit $fail
