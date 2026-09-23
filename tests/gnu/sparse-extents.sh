#!/bin/sh
# Files containing unwritten (preallocated) extents made with fallocate
# read as zeros; copying them must produce a file of the same size with
# identical content, regardless of where the unwritten region sits.
# cpx has no --sparse option, so only its default mode is exercised.
#
# Inspired by GNU coreutils test: tests/cp/sparse-extents.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
command -v fallocate >/dev/null 2>&1 || exit 77  # fallocate utility required

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

touch falloc.test || exit 1
# Skip if this file system does not support fallocate keep-size.
fallocate -l 1 -o 1 -n falloc.test 2>/dev/null || exit 77
rm falloc.test

for alloc in '-l 4194304' '-l 1048576 -o 4194304' '-l 1'; do
  dd count=10 if=/dev/urandom iflag=fullblock of=unwritten.withdata 2>/dev/null || exit 1
  truncate -s 2MiB unwritten.withdata || exit 1
  fallocate $alloc -n unwritten.withdata || exit 1
  cpx --reflink=never unwritten.withdata cp.test || fail=1
  test "$(stat -c %s unwritten.withdata)" = "$(stat -c %s cp.test)" || fail=1
  cmp unwritten.withdata cp.test || fail=1
  rm -f unwritten.withdata cp.test || exit 1
done

exit $fail
