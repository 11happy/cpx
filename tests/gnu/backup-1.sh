#!/bin/sh
# Copying a file onto itself with --force --backup=simple must succeed,
# leaving both the file and its backup with the original contents.
# cpx has no --suffix, so the default "~" suffix is used.
#
# Inspired by GNU coreutils test: tests/cp/backup-1.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

file=F
file_backup="$file~"

echo test > $file || exit 1

cpx --force --backup=simple $file $file || fail=1

test -f $file || fail=1
test -f $file_backup || fail=1
cmp $file $file_backup > /dev/null || fail=1

exit $fail
