#!/bin/sh
# Copying a sparse file (created with a large seek past EOF) must yield
# a byte-identical copy that does not take more disk blocks than the
# source, i.e. holes are preserved rather than being written as zeros.
# cpx has no --sparse option, so only the default behaviour is checked
# (GNU's default --sparse=auto also preserves holes here).
#
# Inspired by GNU coreutils test: tests/cp/sparse.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

size=$((128 * 1024 + 1))
dd bs=1 seek=$size of=sparse < /dev/null 2>/dev/null || exit 1
# Skip unless the file system actually stores this file sparsely.
test "$(stat -c %b sparse)" -lt "$((size / 512))" || exit 77

cpx --reflink=never sparse copy || fail=1
cmp sparse copy || fail=1
test "$(stat -c %b copy)" -le "$(stat -c %b sparse)" \
  || { echo "copy uses more blocks than sparse source" >&2; fail=1; }

# Files with alternating data/zero chunks: content must match and the
# copy of an already-sparse file must not grow.
hole_size=$(stat -c %o copy)
dd if=/dev/zero bs=$hole_size count=8 of=zeros 2>/dev/null || exit 1
tr '\0' 'U' < zeros > nonzero || exit 1
: > file.in
for i in nonzero zeros nonzero zeros; do
  cat $i >> file.in || exit 1
done
cpx --reflink=never file.in out1 || fail=1
cmp file.in out1 || fail=1
cpx --reflink=never out1 out2 || fail=1
cmp file.in out2 || fail=1
test "$(stat -c %b out2)" -le "$(stat -c %b out1)" || fail=1

exit $fail
