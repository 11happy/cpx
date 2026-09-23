#!/bin/sh
# A regular file copied onto a dangling symlink must be refused (also
# with -f) and the symlink's target must not be created.  Copying with
# -f onto a self-referencing symlink loop must replace the loop with a
# regular file.
#
# Inspired by GNU coreutils test: tests/cp/thru-dangling.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

ln -s no-such dangle || exit 1
echo hi > f || exit 1

for opt in '' '-f'; do
  # shellcheck disable=SC2086
  if cpx $opt f dangle >/dev/null 2>&1; then fail=1; fi
  test -e no-such && fail=1
done

# POSIXLY_CORRECT mode is a GNU-only notion; cpx has no equivalent, so
# that part of the original is not exercised.

ln -s loop loop || exit 1
cpx -f f loop 2>err || fail=1
test -s err && fail=1
test -f loop || fail=1
test ! -L loop || fail=1
test "$(cat loop)" = hi || fail=1

exit $fail
