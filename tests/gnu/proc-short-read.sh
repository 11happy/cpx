#!/bin/sh
# Files under /proc report st_size 0 and each read returns less than
# requested.  cp must keep reading until EOF so that the copy matches
# what cat produces.
#
# Inspired by GNU coreutils test: tests/cp/proc-short-read.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

proc_large=/proc/cpuinfo
test -r $proc_large || exit 77  # system without a readable /proc/cpuinfo

cpx $proc_large 1 2>err || { cat err >&2; fail=1; }
cat $proc_large > 2 || fail=1

# Drop the lines that legitimately change between two reads.
del_varying='/MHz/d; /[Bb][Oo][Gg][Oo][Mm][Ii][Pp][Ss]/d;'
sed "$del_varying" 1 > proc.cp || exit 1
sed "$del_varying" 2 > proc.cat || exit 1
cmp proc.cp proc.cat || fail=1

# Also a small procfs file with a fixed st_size of 0.
if test -r /proc/version; then
  cpx /proc/version version || fail=1
  cmp version /proc/version || fail=1
fi

exit $fail
