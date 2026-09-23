#!/bin/sh
# A file with a leading byte and a hole ending exactly at EOF (via seek
# past end), with and without trailing data, must copy byte for byte.
# The --sparse=always/never and --debug parts of the GNU test cannot be
# expressed with cpx, so only content correctness is verified.
#
# Inspired by GNU coreutils test: tests/cp/sparse-2.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

printf x > k || exit 1
dd bs=1k seek=128 of=k < /dev/null 2>/dev/null || exit 1

for append in no yes; do
  test $append = yes && { printf y >> k || exit 1; }
  rm -f k2
  cpx --reflink=never k k2 || fail=1
  cmp k k2 || fail=1
  test "$(stat -c %s k)" = "$(stat -c %s k2)" || fail=1
done

# Data followed by a long run of explicit zeros.
rm -f k k2
printf x > k || exit 1
dd bs=1k seek=1 of=k count=255 < /dev/zero 2>/dev/null || exit 1
cpx --reflink=never k k2 || fail=1
cmp k k2 || fail=1

exit $fail
