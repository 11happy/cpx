#!/bin/sh
# Extended attributes: a plain copy must not carry user xattrs over,
# --preserve=xattr, --preserve=all and -a must, -a must do so silently,
# and --preserve=xattr must also work (and keep the mode) when the source
# is not writable.  Only the cp cases of the GNU test are ported.
# Uses setfattr/getfattr when present, otherwise python3's os.*xattr.
#
# Inspired by GNU coreutils test: tests/cp/xattr.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'chmod -R u+w "$tmp" 2>/dev/null; rm -rf "$tmp"' EXIT
cd "$tmp"

xattr_name="user.foo"
xattr_value="bar"

if command -v setfattr >/dev/null 2>&1 && command -v getfattr >/dev/null 2>&1; then
  xset() { setfattr -n "$xattr_name" -v "$xattr_value" "$1"; }
  # prints "name=value" for the attribute if present
  xget() { getfattr --only-values -n "$xattr_name" "$1" 2>/dev/null; }
elif command -v python3 >/dev/null 2>&1; then
  xset() {
    python3 -c 'import os,sys; os.setxattr(sys.argv[1], sys.argv[2], sys.argv[3].encode())' \
      "$1" "$xattr_name" "$xattr_value"
  }
  xget() {
    python3 -c 'import os,sys
try: sys.stdout.write(os.getxattr(sys.argv[1], sys.argv[2]).decode())
except OSError: sys.exit(1)' "$1" "$xattr_name" 2>/dev/null
  }
else
  # No tool available to set or read extended attributes.
  exit 77
fi

has_xattr() { test "$(xget "$1")" = "$xattr_value"; }

# Make sure this filesystem supports user xattrs at all.
touch a || exit 1
has_xattr a && exit 1
xset a 2>/dev/null || exit 77   # filesystem without user xattr support
has_xattr a || exit 77

# Default copy: xattrs are not preserved.
cpx a b || fail=1
has_xattr b && fail=1

# --preserve=xattr
cpx --preserve=xattr a b || fail=1
has_xattr b || fail=1

# --preserve=all
cpx --preserve=all a c || fail=1
has_xattr c || fail=1

# -a must preserve xattrs and print nothing on stderr.
cpx -a a d 2>err || fail=1
test -s err && { cat err >&2; fail=1; }
has_xattr d || fail=1

# --preserve=xattr on a source without write access, mode kept too.
chmod a-w a || exit 1
rm -f e
cpx --preserve=xattr a e || fail=1
has_xattr e || fail=1
test "$(stat --format=%a e)" = "$(stat --format=%a a)" || fail=1
chmod u+w a || exit 1

exit $fail
