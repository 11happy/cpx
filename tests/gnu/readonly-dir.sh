#!/bin/sh
# Recursive copies (-r and -a) of a tree whose directories are read-only
# (mode 555) must succeed, copy the contents, and give the copied
# directories the same read-only mode rather than a writable 755.
#
# Inspired by GNU coreutils test: tests/cp/readonly-dir.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'chmod -R u+w "$tmp" 2>/dev/null; rm -rf "$tmp"' EXIT
cd "$tmp"

# A set-gid working directory would show up as an extra bit in the modes.
case $(stat -c%A .) in
  *s*) exit 77 ;;
esac

umask 022

mkdir -p a/b/c/d || exit 1
echo "test content" > a/b/c/d/bar.txt || exit 1
chmod -R -w a || exit 1

# cpx copies a directory *into* an existing destination directory, so
# create the targets first and check the copies underneath them.
mkdir b c || exit 1

# -r must keep the 555 modes all the way down.
cpx -r a b || fail=1
test -f b/a/b/c/d/bar.txt || fail=1
for d in a a/b a/b/c a/b/c/d; do
  mode=$(stat --format=%a b/$d 2>/dev/null) || { fail=1; continue; }
  test "$mode" = 555 || { echo "b/$d: mode $mode, want 555" >&2; fail=1; }
done

# -a must do the same and not fail on the read-only directories.
cpx -a a c || fail=1
test -f c/a/b/c/d/bar.txt || fail=1
for d in a a/b; do
  mode=$(stat --format=%a c/$d 2>/dev/null) || { fail=1; continue; }
  test "$mode" = 555 || { echo "c/$d: mode $mode, want 555" >&2; fail=1; }
done

exit $fail
