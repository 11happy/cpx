#!/bin/sh
# Copying a directory into itself (cp -r dir dir, also with -l and with
# extra sources) must be rejected with a non-zero status, must not
# recurse forever, and must leave the tree unchanged.
#
# Inspired by GNU coreutils test: tests/cp/into-self.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
command -v timeout >/dev/null 2>&1 || exit 77  # timeout needed to bound runaway recursion

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

mkdir a dir || exit 1

# dir must not gain a nested copy of itself.
unchanged() {
  test ! -e dir/dir || { echo "$1: created dir/dir" >&2; return 1; }
}

if timeout 20 cpx -r dir dir 2>/dev/null; then
  echo "cpx -r dir dir: expected failure" >&2; fail=1
fi
unchanged "cpx -r dir dir" || fail=1

if timeout 20 cpx -r -l dir dir 2>/dev/null; then
  echo "cpx -rl dir dir: expected failure" >&2; fail=1
fi
unchanged "cpx -rl dir dir" || fail=1

if timeout 20 cpx -r -l a dir dir 2>/dev/null; then
  echo "cpx -rl a dir dir: expected failure" >&2; fail=1
fi
unchanged "cpx -rl a dir dir" || fail=1
# (GNU cp refuses the into-self part; either way 'dir' must not end up
#  containing a copy of itself.)

# Second attempt with the same arguments must also fail cleanly.
if timeout 20 cpx -r -l a dir dir 2>/dev/null; then fail=1; fi
unchanged "cpx -rl a dir dir (2)" || fail=1

exit $fail
