#!/bin/sh
# Files built from many data/hole pairs of various sizes must copy
# without any data loss, and the copy should keep its holes (checked
# via allocated block count rather than filefrag extent maps).
# cpx has no --sparse option, so its default mode is used.
#
# Inspired by GNU coreutils test: tests/cp/sparse-extents-2.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
command -v perl >/dev/null 2>&1 || exit 77  # perl generates the test files

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# Skip if holes are not stored sparsely on this file system.
truncate -s 1M probe || exit 1
test "$(stat -c %b probe)" -lt 2048 || exit 77
rm probe

for i in 1 5 11 21; do
  for j in 1 2 31; do
    perl -e '$n = '$i' * 1024; *F = *STDOUT;' \
         -e 'for (1..'$j') { sysseek (*F, $n, 1)' \
         -e '&& syswrite (*F, chr($_)x$n) or die "$!"}' > j1 || fail=1
    rm -f j2
    cpx --reflink=never j1 j2 || fail=1
    cmp j1 j2 || { echo "data loss i=$i j=$j" >&2; fail=1; }
    test "$(stat -c %b j2)" -le "$(stat -c %b j1)" \
      || { echo "holes lost i=$i j=$j" >&2; fail=1; }
    test $fail = 1 && break 2
  done
done

exit $fail
