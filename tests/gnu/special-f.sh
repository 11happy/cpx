#!/bin/sh
# "cpx -r fifo e" onto an existing regular file must replace e with a fifo
# instead of trying to read the fifo's contents; with -f it must unlink e
# and retry.  Either way cpx must terminate and the source fifo stays a fifo.
#
# Inspired by GNU coreutils test: tests/cp/special-f.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
command -v timeout >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# Filesystem without fifo support: nothing to test.
mkfifo fifo || exit 77

touch e || exit 1

for force in '' '-f'; do
  # A cp that opens the fifo for reading would block forever; KILL it
  # (cpx does not exit on SIGTERM while blocked) and count that as failure.
  timeout -s KILL 10 cpx -r $force fifo e || fail=1
  test -p fifo || fail=1
  test -p e || fail=1
done

exit $fail
