#!/bin/sh
# On a filesystem that does not support user xattrs, -a and --preserve=all
# must still copy the data and stay silent, while an explicit
# --preserve=xattr must fail with a diagnostic.  Needs root to mount the
# loopback/ramfs filesystems.  Only the cp cases of the GNU test are ported.
#
# Inspired by GNU coreutils test: tests/cp/cp-mv-enotsup-xattr.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
# Mounting test filesystems requires root.
[ "$(id -u)" -eq 0 ] || exit 77
command -v setfattr >/dev/null 2>&1 || exit 77
command -v getfattr >/dev/null 2>&1 || exit 77
command -v mkfs >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
cleanup() {
  cd /
  umount "$tmp/noxattr" 2>/dev/null || :
  umount "$tmp/xattr" 2>/dev/null || :
  rm -rf "$tmp"
}
trap cleanup EXIT
cd "$tmp"

# make_fs DIR OPTS: mount a small filesystem at DIR; ext2 with user_xattr,
# or ramfs (which has no user xattr support) otherwise.
make_fs() {
  where=$1
  opts=$2
  mkdir "$where" || exit 1
  if test "$opts" = user_xattr; then
    fs="$where.bin"
    dd if=/dev/zero of="$fs" bs=8192 count=200 >/dev/null 2>&1 || exit 77
    mkfs -t ext2 -F "$fs" >/dev/null 2>&1 || exit 77
    mount -oloop,$opts "$fs" "$where" || exit 77
  else
    mount -t ramfs ramfs "$where" || exit 77
  fi
  echo test > "$where"/f && test -s "$where"/f || exit 77
  if test "$opts" = nouser_xattr; then
    # If user xattrs work here, the "unsupported" side of the test is moot.
    setfattr -n user.test -v value "$where"/f 2>/dev/null && exit 77
  fi
}

make_fs noxattr nouser_xattr
make_fs xattr   user_xattr

xattr_name="user.foo"
xattr_value="bar"
xattr_pair="$xattr_name=\"$xattr_value\""

echo test > xattr/a || exit 1
setfattr -n "$xattr_name" -v "$xattr_value" xattr/a || exit 77
getfattr -d xattr/a | grep -F "$xattr_pair" >/dev/null || exit 77

# -a: succeed, copy data, no diagnostics.
cpx -a xattr/a noxattr/ 2>err || fail=1
test -s noxattr/a || fail=1
test -s err && { cat err >&2; fail=1; }
rm -f err noxattr/a

# --preserve=all, new destination: same.
cpx --preserve=all xattr/a noxattr/ 2>err || fail=1
test -s noxattr/a || fail=1
test -s err && { cat err >&2; fail=1; }

# --preserve=all, existing destination: same.
cpx --preserve=all xattr/a noxattr/ 2>err || fail=1
test -s noxattr/a || fail=1
test -s err && { cat err >&2; fail=1; }
rm -f err noxattr/a

# Explicit --preserve=xattr: must fail and say so.
if cpx -a --preserve=xattr xattr/a noxattr/ 2>err; then fail=1; fi
test -s err || fail=1
rm -f err noxattr/a

exit $fail
