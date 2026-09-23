#!/bin/sh
# Copy a file made of many small extents separated by holes with
# --reflink=never and verify the copy is byte-identical.  GNU runs this
# under valgrind to catch a free-memory read in the extent logic; here
# valgrind is used when available and the copy is checked either way.
#
# Inspired by GNU coreutils test: tests/cp/copy-FMR.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
# perl is needed to build the sparse input
command -v perl >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

perl -e 'for (1..600) { sysseek (*STDOUT, 4096, 1)' \
     -e '&& syswrite (*STDOUT, "a" x 1024) or die "$!"}' > j || exit 1

if command -v valgrind >/dev/null 2>&1; then
  valgrind --quiet --error-exitcode=3 "$(command -v cpx)" --reflink=never j j2 || fail=1
else
  cpx --reflink=never j j2 || fail=1
fi
cmp j j2 || fail=1

exit $fail
