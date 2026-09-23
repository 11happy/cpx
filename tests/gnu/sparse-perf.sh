#!/bin/sh
# Copying a huge, almost entirely empty sparse file (1 TiB with a
# single byte) must finish quickly, because the copier should skip
# holes rather than reading and writing terabytes of zeros.  A file
# that only looks sparse (all data) must also copy correctly.
# The --debug assertions of the GNU test are not available in cpx.
#
# Inspired by GNU coreutils test: tests/cp/sparse-perf.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
command -v timeout >/dev/null 2>&1 || exit 77  # timeout bounds the copy

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# Small sparse file: 'x' then a hole up to 1 MiB.
printf x > k || exit 1
truncate -s1M k || exit 1
cpx k k2 || fail=1
cmp k k2 || fail=1

# Non-sparse file that is all data.
yes | head -n 100000 > mls || exit 1
cpx mls mls.cp || fail=1
cmp mls mls.cp || fail=1

# 1 TiB sparse file; skip if the file system cannot create it.
timeout 10 truncate -s1T f 2>/dev/null || exit 77
test "$(stat -c %b f)" -lt 1024 || exit 77  # not stored sparsely

timeout 10 cpx --reflink=never f f2 || { echo "1T sparse copy failed or timed out" >&2; fail=1; }
test "$(stat -c %s f)" = "$(stat -c %s f2 2>/dev/null)" || fail=1

exit $fail
