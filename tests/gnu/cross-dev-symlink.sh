#!/bin/sh
# Forced symbolic-link creation (-s -f) where the source lives on a
# different file system and the destination is an existing symlink to
# that same source must succeed.  GNU mounts an ext4 image as root;
# here we instead look for an already-mounted, writable, different
# file system and skip when none is available.
#
# Inspired by GNU coreutils test: tests/cp/cross-dev-symlink.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp" "${other:-}"' EXIT
cd "$tmp"

here=$(stat -c %d "$tmp")
other=
for cand in /dev/shm /run/user/$(id -u) /var/tmp /tmp "$HOME"; do
  [ -d "$cand" ] && [ -w "$cand" ] || continue
  [ "$(stat -c %d "$cand")" != "$here" ] || continue
  other=$(mktemp -d "$cand/cpx-xdev.XXXXXX") || continue
  break
done
# no second writable file system found; cannot test a cross-device link
[ -n "$other" ] || exit 77

mkdir "$other/path1" || exit 1
touch "$other/path1/file" || exit 1
mkdir path2 || exit 1
cd path2 && ln -s "$other/path1/file" . || exit 1

cpx -P --preserve=links -s=auto -f "$other/path1/file" . || fail=1
test -L file || fail=1
test "$(readlink -f file)" = "$(readlink -f "$other/path1/file")" || fail=1

exit $fail
