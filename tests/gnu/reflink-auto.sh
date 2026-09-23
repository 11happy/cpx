#!/bin/sh
# --reflink=always must fail when the source lives on a different
# filesystem (no cloning possible), --reflink=auto must fall back to a
# normal copy, and --reflink=never must copy normally as well.
# The GNU --sparse=always fallback case is not ported (cpx has no --sparse).
#
# Inspired by GNU coreutils test: tests/cp/reflink-auto.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
other=
trap 'rm -rf "$tmp" "$other"' EXIT
cd "$tmp"

# Find a writable directory on a different filesystem than $tmp.
here=$(stat -c %d .)
for cand in "${HOME:-/nonexistent}" /var/tmp /dev/shm /tmp "$PWD"; do
  test -d "$cand" && test -w "$cand" || continue
  test "$(stat -c %d "$cand")" = "$here" && continue
  other=$(mktemp -d "$cand/cpx-reflink.XXXXXX") 2>/dev/null || continue
  break
done
# No second filesystem available to test cross-device behaviour.
test -n "$other" || exit 77

a_other="$other/a"
echo non_zero_size > "$a_other" || exit 1

# Cloning across filesystems is impossible: must fail.
if cpx --reflink=always "$a_other" b; then fail=1; fi

# auto falls back to a regular copy.
cpx --reflink=auto "$a_other" b || fail=1
test -s b || fail=1
cmp "$a_other" b || fail=1

# never copies normally too.
rm -f b
cpx --reflink=never "$a_other" b || fail=1
test -s b || fail=1
cmp "$a_other" b || fail=1

exit $fail
