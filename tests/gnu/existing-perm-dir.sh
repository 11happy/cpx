#!/bin/sh
# A recursive copy into an existing destination directory must keep the
# destination directory's own (more restrictive) permissions rather than
# widening them to match the source directory.
#
# Inspired by GNU coreutils test: tests/cp/existing-perm-dir.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

umask 002
mkdir -p -m ug-s,u=rwx,g=rwx,o=rx src/dir || exit 1
mkdir -p -m ug-s,u=rwx,g=,o= dst/dir || exit 1

cpx -r src/. dst/ || fail=1

mode=$(stat -c %A dst/dir)
test "$mode" = drwx------ || fail=1

exit $fail
