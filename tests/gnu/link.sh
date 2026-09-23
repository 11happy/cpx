#!/bin/sh
# Check that -f combined with --link or --symbolic-link replaces an
# existing destination file instead of failing on it.
#
# Inspired by GNU coreutils test: tests/cp/link.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

touch src dest dest2 || exit 1

# forced hard link over an existing file
cpx -f --link src dest || fail=1
test "$(stat -c %i src)" = "$(stat -c %i dest)" || fail=1

# forced symlink over an existing file
# (-s takes an optional MODE, so '--' stops it from eating the source name)
cpx -f --symbolic-link -- src dest2 || fail=1
test -L dest2 || fail=1
test "$(readlink dest2)" = src || fail=1

exit $fail
