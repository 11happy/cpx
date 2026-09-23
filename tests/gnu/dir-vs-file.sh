#!/bin/sh
# Copying a directory onto an existing regular file must fail and leave
# the file in place.
#
# Inspired by GNU coreutils test: tests/cp/dir-vs-file.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir dir || exit 1
touch file || exit 1

cpx -r dir file 2>/dev/null && fail=1
test -f file || fail=1

exit $fail
