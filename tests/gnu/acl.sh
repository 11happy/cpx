#!/bin/sh
# copy files/directories across file system boundaries
# and make sure acls are preserved appropriately
#
# Inspired by GNU coreutils test: tests/cp/acl.sh
# Independent reimplementation for CPX.

set -eu

fail=0

command -v cpx >/dev/null 2>&1 || exit 77
command -v getfacl >/dev/null 2>&1 || exit 77
command -v setfacl >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir -p a b || exit 1
touch a/file || exit 1

# Ensure that setfacl and getfacl work on this file system.
skip=no
acl1=$(cd a && getfacl file) || skip=yes
setfacl -m user:bin:rw- a/file 2> /dev/null || skip=yes
test $skip = yes && exit 77

# copy a file without preserving permissions
cpx a/file b/ || fail=1
acl2=$(cd b && getfacl file) || fail=1
test "$acl1" = "$acl2" || fail=1

# Update with acl set above
acl1=$(cd a && getfacl file) || fail=1

# copy a file, preserving permissions
cpx --preserve=mode,ownership,timestamps a/file b/ || fail=1
acl2=$(cd b && getfacl file) || fail=1
test "$acl1" = "$acl2" || fail=1

# copy a file, preserving permissions, with --attributes-only
echo > a/file || exit 1
test -s a/file || exit 1
cpx --preserve=mode,ownership,timestamps --attributes-only a/file b/ || fail=1
cmp /dev/null b/file || fail=1
acl2=$(cd b && getfacl file) || fail=1
test "$acl1" = "$acl2" || fail=1

exit $fail
