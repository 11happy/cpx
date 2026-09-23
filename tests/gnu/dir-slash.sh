#!/bin/sh
# cp -R dir1/ dir2 (trailing slash on the source) must still copy dir1 as
# dir2/dir1 rather than spilling its contents directly into dir2.
#
# Inspired by GNU coreutils test: tests/cp/dir-slash.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir dir1 dir2 || exit 1
touch dir1/file || exit 1

cpx -r dir1/ dir2 || fail=1

test -r dir2/file && fail=1
test -r dir2/dir1/file || fail=1
test -r dir1/file || fail=1

exit $fail
