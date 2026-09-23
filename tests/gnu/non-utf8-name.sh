#!/bin/sh
# A source directory whose name is not valid UTF-8 must be copied
# normally (using the trailing "/." form) without crashing, in the C
# locale and in UTF-8 locales.
#
# Inspired by GNU coreutils test: tests/cp/non-utf8-name.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

bad="$(printf 'bad\370dir')"
# Skip if the shell or file system cannot handle the byte sequence.
mkdir "$bad" target 2>/dev/null || exit 77
test "$(ls | grep -c dir)" = 1 || exit 77
touch "$bad"/file1 "$bad"/file2 || exit 1

for loc in C C.UTF-8 en_US.UTF-8; do
  if [ "$loc" != C ]; then
    locale -a 2>/dev/null | grep -qix "$(echo "$loc" | tr -d -)" || continue
  fi
  LC_ALL=$loc cpx -r "$bad"/. target 2>/dev/null || { echo "loc=$loc: cpx failed" >&2; fail=1; }
  rm target/file1 2>/dev/null || { echo "loc=$loc: target/file1 missing" >&2; fail=1; }
  rm target/file2 2>/dev/null || { echo "loc=$loc: target/file2 missing" >&2; fail=1; }
  test -f "$bad"/file1 || fail=1
  test -f "$bad"/file2 || fail=1
done

exit $fail
