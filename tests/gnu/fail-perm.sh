#!/bin/sh
# A recursive --preserve copy that hits an unreadable file must fail, yet the
# directory it already created must still receive the source directory's
# restrictive mode.  Also, a destination that is a symlink into an
# inaccessible directory must be reported as an error (both as a plain
# destination and via -t).
#
# Inspired by GNU coreutils test: tests/cp/fail-perm.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
# Root can read anything, so the permission failures would not occur.
[ "$(id -u)" -ne 0 ] || exit 77

tmp="$(mktemp -d)"
trap 'chmod -R u+rwx "$tmp" 2>/dev/null; rm -rf "$tmp"' EXIT
cd "$tmp"

preserve='--preserve=mode,ownership,timestamps'

chmod g-s . || exit 1
mkdir D D/D || exit 1
touch D/a || exit 1
chmod 0 D/a || exit 1
chmod u=rx,go=,-st D || exit 1

# cpx copies a directory *into* an existing destination directory, so use
# that form to get a predictable path for the copy of D.
mkdir dest || exit 1

# Must fail: D/a cannot be read.
if cpx $preserve -r D dest >/dev/null 2>&1; then fail=1; fi

# The copied directory must exist and carry D's mode.
test -d dest/D || fail=1
mode=$(ls -ld dest/D | cut -b-10)
test "$mode" = dr-x------ || { echo "dest/D mode: $mode" >&2; fail=1; }

chmod 0 D
ln -s D/D symlink
touch F

# The symlink target cannot be stat'ed: copying onto it must fail.
if cpx F symlink 2>/dev/null; then fail=1; fi
if cpx -t symlink F 2>/dev/null; then fail=1; fi

chmod 700 D

exit $fail
