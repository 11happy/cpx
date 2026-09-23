#!/bin/sh
# Copying a file onto a symlink that points to that same file must be
# refused so the file is not truncated.  GNU's -d is expressed as
# -P --preserve=links.
#
# Inspired by GNU coreutils test: tests/cp/no-deref-link3.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

msg=bar
echo $msg > a || exit 1
ln -s a b || exit 1

if cpx -P --preserve=links a b 2>/dev/null; then fail=1; fi
test "$(cat a)" = $msg || fail=1

exit $fail
