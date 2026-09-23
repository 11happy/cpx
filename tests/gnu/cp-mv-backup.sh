#!/bin/sh
# Exercise every --backup mode (none, numbered, existing, simple) against a
# range of pre-existing destination/backup files and compare the resulting
# file names with what GNU cp produces.  Only the cp half of the GNU test is
# ported; the mode aliases (off, t, nil, never) are not accepted by cpx.
#
# Inspired by GNU coreutils test: tests/cp/cp-mv-backup.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

umask 022

for initial_files in 'x' 'x y' 'x y y~' 'x y y.~1~' 'x y y~ y.~1~'; do
  for opt in none numbered existing simple; do
    touch $initial_files
    cpx --backup=$opt x y 2>/dev/null || { fail=1; echo "cpx failed: $initial_files $opt" >&2; }
    echo $initial_files $opt: $(ls [xy]*)
    rm -f x y y~ y.~?~
  done
done > actual

cat <<\EXP > expected
x none: x y
x numbered: x y
x existing: x y
x simple: x y
x y none: x y
x y numbered: x y y.~1~
x y existing: x y y~
x y simple: x y y~
x y y~ none: x y y~
x y y~ numbered: x y y.~1~ y~
x y y~ existing: x y y~
x y y~ simple: x y y~
x y y.~1~ none: x y y.~1~
x y y.~1~ numbered: x y y.~1~ y.~2~
x y y.~1~ existing: x y y.~1~ y.~2~
x y y.~1~ simple: x y y.~1~ y~
x y y~ y.~1~ none: x y y.~1~ y~
x y y~ y.~1~ numbered: x y y.~1~ y.~2~ y~
x y y~ y.~1~ existing: x y y.~1~ y.~2~ y~
x y y~ y.~1~ simple: x y y.~1~ y~
EXP

if ! cmp expected actual >/dev/null; then
  diff expected actual >&2 || :
  fail=1
fi

exit $fail
