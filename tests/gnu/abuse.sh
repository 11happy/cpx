#!/bin/sh
# A recursive copy that first creates a symlink in the destination must not
# then write a later source file through that symlink.  Both a dangling
# link target and an existing, writable target are checked: cp must fail
# and the target must be left untouched.
#
# Inspired by GNU coreutils test: tests/cp/abuse.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir a b c || exit 1
ln -s ../t a/1 || exit 1
echo payload > b/1 || exit 1

for i in dangling-dest existing-dest; do
  test $i = existing-dest && echo i > t
  test $i = dangling-dest && rm -f t

  # cp -dR equivalent
  cpx -P --preserve=links -r a/1 b/1 c 2>/dev/null && fail=1

  # dangling: the link target must not have been created
  test $i = dangling-dest && test -f t && fail=1

  # existing: the link target must keep its contents
  test $i = existing-dest && { test "$(cat t)" = i || fail=1; }

  rm -rf c && mkdir c || exit 1
done

exit $fail
