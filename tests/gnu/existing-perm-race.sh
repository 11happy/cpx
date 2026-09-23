#!/bin/sh
# When --preserve overwrites an existing destination that belongs to a
# different group and is group-readable, the result must end up with the
# source's group and the source's restrictive mode; the old, more permissive
# destination mode must not survive.  GNU checks the mode while cp is still
# copying (via a fifo); only the final state is verified here since the
# intermediate one cannot be observed without racing.
# Needs a user that belongs to at least two groups.
#
# Inspired by GNU coreutils test: tests/cp/existing-perm-race.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# Two different groups are needed for a meaningful group check.
set -- $(id -G)
[ $# -ge 2 ] || exit 77
g1=$1
g2=$2

# A default ACL on the directory would change the modes of new files.
if command -v getfacl >/dev/null 2>&1; then
  getfacl . 2>/dev/null | grep -q '^default:' && exit 77
fi

umask 077
echo secret > src || exit 1
touch src-copy || exit 1
chgrp $g1 src || exit 77   # cannot use these groups: skip
chgrp $g2 src-copy || exit 77
chmod g+r src-copy || exit 1

cpx --preserve=mode,ownership,timestamps src src-copy || fail=1

# Final mode and group must be the source's.
set -- $(ls -l -n src-copy)
mode=$1
group=$4
case $mode in
  -rw-------*) test "$group" = "$g1" || fail=1 ;;
  *) echo "src-copy: unexpected mode $mode" >&2; fail=1 ;;
esac
cmp src src-copy || fail=1

exit $fail
