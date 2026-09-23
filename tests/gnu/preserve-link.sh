#!/bin/sh
# With -au a set of hard-linked source files must all end up as hard
# links in the destination, whether the destination already has a
# matching link, lacks it, or holds a separate older or newer file in
# its place.
#
# Inspired by GNU coreutils test: tests/cp/preserve-link.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

same_inode() {
  test "$(stat -c %i "$1")" = "$(stat -c %i "$2")"
}

create_source_tree() {
  rm -rf s
  mkdir s || exit 1
  touch s/f || exit 1
  ln s/f s/linkm || exit 1   # missing in dest: will be created as a link
  ln s/f s/linke || exit 1   # already a link in dest: kept
  ln s/f s/fileo || exit 1   # separate older file in dest: replaced
  ln s/f s/fileu || exit 1   # separate newer file in dest: replaced too
}

create_target_tree() {
  rm -rf t
  mkdir -p t/s || exit 1
  touch "t/s/$1" || exit 1
  ln "t/s/$1" t/s/linke || exit 1
  touch -d '-1 hour' t/s/fileo || exit 1
  touch -d '+1 hour' t/s/fileu || exit 1
}

create_source_tree
for f in f linkm; do
  create_target_tree $f
  cpx -au s t || fail=1
  same_inode t/s/f t/s/linkm || fail=1
  same_inode t/s/f t/s/linke || fail=1
  same_inode t/s/f t/s/fileo || fail=1
  same_inode t/s/f t/s/fileu || fail=1
done

exit $fail
