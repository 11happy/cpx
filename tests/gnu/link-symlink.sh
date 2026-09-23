#!/bin/sh
# With -al a symlink whose own modification time was set must be
# copied so that the resulting symlink carries the same timestamp.
#
# Inspired by GNU coreutils test: tests/cp/link-symlink.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

touch file || exit 1
ln -s file link || exit 1

# skip if symlink timestamps cannot be updated here
touch -m -h -d 2011-01-01 link 2>/dev/null || exit 77
case $(stat --format=%y link) in
  2011-01-01*) ;;
  *) exit 77 ;;
esac

cpx -al link link.cp || fail=1
test -L link.cp || fail=1
case $(stat --format=%y link.cp) in
  2011-01-01*) ;;
  *) fail=1 ;;
esac

exit $fail
