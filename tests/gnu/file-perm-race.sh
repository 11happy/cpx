#!/bin/sh
# A --preserve copy of a file with restrictive permissions must not produce
# a destination readable by group/other, even though the umask alone would
# allow it.  GNU inspects the destination while cp is blocked on a fifo;
# this port only verifies the final state, which is the best non-racy
# approximation available.
#
# Inspired by GNU coreutils test: tests/cp/file-perm-race.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

umask 022
echo data > src || exit 1
chmod 600 src || exit 1

cpx --preserve=mode,ownership,timestamps src src-copy || fail=1

case $(ls -l src-copy) in
  -???------*) ;;
  *) echo "src-copy: unexpected mode: $(ls -l src-copy)" >&2; fail=1 ;;
esac
cmp src src-copy || fail=1

exit $fail
