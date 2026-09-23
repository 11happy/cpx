#!/bin/sh
# Check the mode bits cpx gives newly created files.  Without --preserve the
# new file gets the source mode filtered through the umask; --preserve=ownership
# alone must not change that; and a umask that removes the owner write bit
# must not stop the data from being written (cpx writes through the fd it
# created, so the resulting 0400 file still holds the full contents).
# The GNU --no-preserve=mode cases are not ported: cpx has no --no-preserve.
#
# Inspired by GNU coreutils test: tests/cp/preserve-mode.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# A default ACL on the directory would change the modes of new files.
if command -v getfacl >/dev/null 2>&1; then
  getfacl . 2>/dev/null | grep -q '^default:' && exit 77
fi

get_mode() { stat -c%f "$1"; }

umask 0022 || exit 1

# Regular file: the umask is applied to the source mode.
touch a b || exit 1
chmod 600 b || exit 1
cpx b c || fail=1
test "$(stat -c%a c)" = 600 || fail=1
chmod 666 b || exit 1
cpx b c2 || fail=1
test "$(stat -c%a c2)" = 644 || fail=1

# Existing destination: its mode is left alone.
chmod 600 c || exit 1
cpx a c || fail=1
test "$(stat -c%a c)" = 600 || fail=1

# Directory: mode filtered through the umask when not preserving.
mkdir d1 d2 dest || exit 1
chmod 705 d2 || exit 1
cpx -r d2 dest || fail=1
test "$(get_mode d1)" = "$(get_mode dest/d2)" || fail=1

# Plain --preserve=ownership must not affect the destination mode.
rm -f a b c || exit 1
touch a || exit 1
chmod 660 a || exit 1
cpx a b || fail=1
cpx --preserve=ownership a c || fail=1
test "$(get_mode b)" = "$(get_mode c)" || fail=1

# A umask removing the owner write bit must not prevent writing the data.
rm -f a b || exit 1
echo not-writable-dest > a || exit 1
chmod 644 a || exit 1
(umask 377 && cpx a b) || fail=1
cmp a b || fail=1
test "$(stat -c%a b)" = 400 || fail=1

exit $fail
