#!/bin/sh
# --parents with -R and --preserve=mode or --preserve=ownership must not
# leave the parent directories it creates with more permissions than they
# should end up with.  GNU freezes cp on a fifo to look at the directory
# while the copy is in progress; here only the final state is checked
# (no non-racy way exists to observe the intermediate state), so this is a
# best-effort port: the created parent must exist, be a directory and have
# exactly the expected mode once cpx is done.
#
# Inspired by GNU coreutils test: tests/cp/parent-perm-race.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

umask 002
umask 022
mkdir mode ownership d || exit 1
chmod g+s d 2>/dev/null || :   # valid either way

for attr in mode ownership; do
  echo data > $attr/file || exit 1
  chmod 750 $attr || exit 1

  cpx --preserve=$attr -r --parents $attr d || fail=1
  test -d d/$attr || fail=1
  test -f d/$attr/file || fail=1

  ls_output=$(ls -ld d/$attr)
  case $attr,$ls_output in
    # --preserve=mode: the source mode (rwxr-x---) is reproduced.
    mode,drwxr-[xs]---* ) ;;
    # --preserve=ownership only: the directory gets 0777 & ~umask (the
    # test runs with umask 022, set above), like any new directory.
    ownership,drwxr-[xs]r-x* ) ;;
    *)
      echo "d/$attr: unexpected mode: $ls_output" >&2
      fail=1 ;;
  esac
done

exit $fail
