#!/bin/sh
# Like no-deref-link1 but the source lives in the current directory:
# copying it into a directory holding a symlink to it must fail and
# leave the source untouched.  GNU's -d is expressed as
# -P --preserve=links.
#
# Inspired by GNU coreutils test: tests/cp/no-deref-link2.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir b || exit 1
msg=bar
echo $msg > a || exit 1
(cd b && ln -s ../a .) || exit 1

if cpx -P --preserve=links a b 2>/dev/null; then fail=1; fi
test "$(cat a)" = $msg || fail=1

exit $fail
