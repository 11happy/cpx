#!/bin/sh
# A --reflink copy with --preserve must still get the source's permissions
# and timestamps, whatever the umask, and --attributes-only must win over
# --reflink: the destination is created but stays empty.
#
# Inspired by GNU coreutils test: tests/cp/reflink-perm.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

preserve='--preserve=mode,ownership,timestamps'

> time_check
> file
ts='2009-08-28 19:00'
touch -d "$ts" file || exit 1
# Clock is behind the fixed timestamp: the -nt check would be meaningless.
test time_check -nt file || exit 77

chmod a=rwx file || exit 1
umask 077
cpx --reflink=auto $preserve file copy || fail=1

mode=$(stat --printf "%A" copy)
test "$mode" = "-rwxrwxrwx" || { echo "copy mode: $mode" >&2; fail=1; }

# Timestamps preserved: the copy must not be newer than the original.
test copy -nt file && fail=1

# --attributes-only overrides --reflink entirely.
echo > file2
cpx --reflink=auto $preserve --attributes-only file2 empty_copy || fail=1
test -f empty_copy || fail=1
cmp /dev/null empty_copy || fail=1
cpx --reflink=always $preserve --attributes-only file2 empty_copy || fail=1
cmp /dev/null empty_copy || fail=1

exit $fail
