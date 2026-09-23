#!/bin/sh
# --preserve must keep the set-uid/set-gid bits when run by root, and an
# unprivileged user copying a file it does not own must not get those bits
# on its copy.  Root is required both to keep the bits and to switch user.
#
# Inspired by GNU coreutils test: tests/cp/special-bits.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
# Preserving set-uid/set-gid bits across owners requires root.
[ "$(id -u)" -eq 0 ] || exit 77
non_root=${NON_ROOT_USERNAME:-nobody}
id "$non_root" >/dev/null 2>&1 || exit 77
# GNU chroot --user is needed to rerun cpx as the unprivileged user.
chroot --skip-chdir --user="$non_root" / true >/dev/null 2>&1 || exit 77

tmp="$(TMPDIR=/tmp mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

preserve='--preserve=mode,ownership,timestamps'

touch a b c || exit 1
chmod u+sx,go= a || exit 1
chmod u=rwx,g=sx,o= b || exit 1
chmod a=r,ug+sx c || exit 1
chown "$non_root" . || exit 1
chmod u=rwx,g=rx,o=rx . || exit 1

perms() { set -- $(ls -l "$1"); echo "$1"; }

cpx $preserve a a2 || fail=1
test "$(perms a)" = "$(perms a2)" || fail=1

cpx $preserve b b2 || fail=1
test "$(perms b)" = "$(perms b2)" || fail=1

# The unprivileged user must not end up with root's set-id bits.
chroot --skip-chdir --user="$non_root" / \
  env PATH="$PATH" HOME=/nonexistent cpx $preserve "$tmp/c" "$tmp/c2" || fail=1
test "$(perms c)" = "$(perms c2)" && fail=1

exit $fail
