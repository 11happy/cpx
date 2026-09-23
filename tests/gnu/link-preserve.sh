#!/bin/sh
# Verify that hard links among sources are reproduced in the destination
# when links are preserved (-a, --preserve=links with -H or -L) and that
# hard links are broken when --preserve=links is not requested.
# GNU's --no-preserve=links case is expressed as -r -P without any
# link preservation.  The final --no-preserve=mode case has no cpx
# equivalent and is omitted.
#
# Inspired by GNU coreutils test: tests/cp/link-preserve.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

same_inode() {
  test "$(stat -c %i "$1")" = "$(stat -c %i "$2")"
}

# two hard-linked files copied together with -a stay linked
touch a || exit 1
ln a b || exit 1
mkdir c || exit 1
cpx -a a b c || fail=1
test -f c/a || fail=1
test -f c/b || fail=1
same_inode c/a c/b || fail=1

# a file and a symlink to it, both dereferenced via -H, become one inode
rm -rf a b c
touch a; ln -s a b; mkdir c
cpx --preserve=links -r -H a b c || fail=1
same_inode c/a c/b || fail=1

# same inside a directory with -L
rm -rf a b c d; mkdir d; (cd d; touch a; ln -s a b)
cpx --preserve=links -r -L d c || fail=1
same_inode c/a c/b || fail=1

# hard links inside a directory with -L
rm -rf a b c d; mkdir d; (cd d; touch a; ln a b)
cpx --preserve=links -r -L d c || fail=1
same_inode c/a c/b || fail=1

# without link preservation the hard links are split
rm -rf a b c d; mkdir d; (cd d; touch a; ln a b)
cpx -r -P d c || fail=1
same_inode c/a c/b && fail=1

# -a with explicit file arguments
rm -rf a b c d
touch a; ln a b
mkdir c
cpx -a a b c || fail=1
same_inode c/a c/b || fail=1

exit $fail
