#!/bin/sh
# -P with timestamp preservation on a dangling symlink must copy the
# symlink's own modification time onto the new symlink.
#
# Inspired by GNU coreutils test: tests/cp/preserve-slink-time.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

ln -s no-such dangle || exit 1

# a file system with 1-second symlink timestamps needs a pause so the
# comparison below is meaningful
case $(stat --format=%y dangle) in
  *.000000000) sleep 2 ;;
esac

copy_timestamp() {
  sleep "$1"
  rm -f d2
  cpx -P --preserve=mode,ownership,timestamps dangle d2 || return 1
  test -L d2 || return 1
  test "$(stat --format=%y dangle)" = "$(stat --format=%y d2)"
}

# retry a few times with growing delays like the original
ok=0
delay=.1
for i in 1 2 3 4; do
  if copy_timestamp $delay; then ok=1; break; fi
  delay=$(awk "BEGIN{print $delay*2}")
done
test $ok -eq 1 || fail=1

exit $fail
