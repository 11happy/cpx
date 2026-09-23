#!/bin/sh
# --remove-destination works with a recursive directory copy whether or
# not the destination already exists, and replaces a self-referencing
# symlink destination without tripping over ELOOP.
#
# Inspired by GNU coreutils test: tests/cp/dir-rm-dest.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir d e || exit 1

cpx -r --remove-destination d e || fail=1
cpx -r --remove-destination d e || fail=1

ln -s loop loop || exit 1
touch file || exit 1
cpx --remove-destination file loop || fail=1
test -f loop || fail=1
test -L loop && fail=1

exit $fail
