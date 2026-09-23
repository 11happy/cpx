#!/bin/sh
# Copying a tree with -al must create hard links for many entries
# without excessive memory use.  A directory with a few thousand files
# and subdirectories is duplicated with hard links, then the pair is
# copied again with -al under an address-space limit.
#
# Inspired by GNU coreutils test: tests/cp/link-heap.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# Find a baseline for the virtual-memory ceiling; skip if ulimit -v is
# unavailable in this shell.
touch f || exit 1
( ulimit -v 4000000 ) 2>/dev/null || exit 77
if ! (ulimit -v 4000000 && cpx -al f f2) >/dev/null 2>&1; then
  # cannot run cpx under a vm limit at all (e.g. sanitizers); skip
  exit 77
fi
rm -f f f2

n=2000
a=$(printf %031d 0)
b=$(printf %031d 1)
( mkdir "$a" && cd "$a" &&
  seq --format=%031g $n | xargs touch &&
  seq --format=d%030g $n | xargs mkdir ) || exit 1

cpx -al "$a" "$b" || exit 1
mkdir e || exit 1
mv "$a" "$b" e || exit 1

# every entry of e must be linked, not copied, under a modest limit
(ulimit -v 4000000 && cpx -al e f) || fail=1

test "$(stat -c %i "e/$a/$(printf %031d 5)")" = \
     "$(stat -c %i "f/$a/$(printf %031d 5)")" || fail=1
test "$(stat -c %i "e/$b/$(printf %031d 5)")" = \
     "$(stat -c %i "f/$b/$(printf %031d 5)")" || fail=1
test -d "f/$b/$(printf d%030d 7)" || fail=1

exit $fail
