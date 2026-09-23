#!/bin/sh
# Copying a file into a directory that already holds a symlink pointing
# back to that file must be refused, and the source must stay intact.
# GNU's -d is expressed as -P --preserve=links.
#
# Inspired by GNU coreutils test: tests/cp/no-deref-link1.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir a b || exit 1
msg=bar
echo $msg > a/foo || exit 1
(cd b && ln -s ../a/foo .) || exit 1

if cpx -P --preserve=links a/foo b 2>/dev/null; then fail=1; fi
test "$(cat a/foo)" = $msg || fail=1

exit $fail
