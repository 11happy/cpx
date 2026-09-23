#!/bin/sh
# A symlink to a directory given with a trailing slash must be followed
# even under -P: the copy is an (empty) directory, not a symlink.
# GNU's -d is expressed as -P --preserve=links.
#
# Inspired by GNU coreutils test: tests/cp/symlink-slash.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir dir || exit 1
ln -s dir symlink || exit 1

cpx -P --preserve=links -r symlink/ s || fail=1
test -d s || fail=1
test ! -L s || fail=1
test -z "$(ls -A s)" || fail=1

exit $fail
