#!/bin/sh
# When the destination is an existing FIFO, the data of a sparse source
# (holes included, as zeros) must be written through the pipe so the
# reader receives a byte-identical stream.  cpx lacks -T; the fifo is a
# non-directory so the plain two-argument form behaves the same.
#
# Inspired by GNU coreutils test: tests/cp/sparse-to-pipe.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
command -v timeout >/dev/null 2>&1 || exit 77  # timeout guards against hangs

tmp="$(mktemp -d)"
pid=
trap 'if [ -n "$pid" ]; then kill $pid 2>/dev/null; fi; rm -rf "$tmp"' EXIT
cd "$tmp"

mkfifo pipe 2>/dev/null || exit 77  # fifos not supported here

timeout 10 cat pipe > copy & pid=$!

truncate -s1M sparse || exit 1
timeout 10 cpx sparse pipe || fail=1
wait $pid || fail=1
pid=

cmp sparse copy || fail=1
test -p pipe || { echo "destination fifo was replaced" >&2; fail=1; }

exit $fail
