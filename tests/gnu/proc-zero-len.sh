#!/bin/sh
# A procfs file whose stat size is zero still has content; the copy
# must be non-empty whenever cat shows the source is non-empty.
#
# Inspired by GNU coreutils test: tests/cp/proc-zero-len.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

touch empty || exit 1
f=/proc/cpuinfo
test -r $f || f=empty

cat $f > out || fail=1
cpx $f exp 2>err || { cat err >&2; fail=1; }

# Reduce both to "empty" / "nonempty" and compare.
test -s out && { rm -f out; echo nonempty > out; }
test -s exp && { rm -f exp; echo nonempty > exp; }
cmp exp out || fail=1

exit $fail
