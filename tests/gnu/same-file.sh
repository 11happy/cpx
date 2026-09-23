#!/bin/sh
# Copying a file onto itself, onto a symlink that resolves to itself, or
# onto a hard link of itself must never destroy the data.  Plain copies
# must be refused ("same file"), while -f/-b/-l/-s/--remove-destination
# have their own well-defined outcomes.  This is a representative subset
# of the full GNU option matrix; every case checks both the exit status
# and that the original content survives.
#
# Inspired by GNU coreutils test: tests/cp/same-file.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

contents=XYZ

# Rebuild a fresh fixture directory: foo, symlink -> foo, hardlink (=foo),
# sl1 -> foo, sl2 -> foo.
setup() {
  rm -rf dir
  mkdir dir || exit 1
  cd dir || exit 1
  echo "$contents" > foo || exit 1
  ln -s foo symlink || exit 1
  ln foo hardlink || exit 1
  ln -s foo sl1 || exit 1
  ln -s foo sl2 || exit 1
}

# Verify that foo still holds the original data (readable through the
# name, not a symlink loop).
check_foo() {
  test -f foo || { echo "$1: foo is gone or not a regular file" >&2; return 1; }
  test "$(cat foo 2>/dev/null)" = "$contents" \
    || { echo "$1: foo content destroyed" >&2; return 1; }
}

# --- 1. same file: cp foo foo (and -f) must fail and leave foo intact.
for opt in '' -f --remove-destination --backup=numbered --symbolic-link=auto; do
  setup
  if cpx $opt foo foo 2>/dev/null; then
    echo "cpx $opt foo foo: expected failure" >&2; fail=1
  fi
  check_foo "cpx $opt foo foo" || fail=1
  test -e foo.~1~ && { echo "cpx $opt foo foo: made a backup" >&2; fail=1; }
  cd ..
done

# -b -f on the same file: GNU makes a numbered backup and succeeds;
# the content must survive either way.
setup
cpx --backup=numbered -f foo foo 2>/dev/null || fail=1
check_foo "cpx -bf foo foo" || fail=1
test -f foo.~1~ || fail=1
cd ..

# -l foo foo: a hard link onto itself is a no-op success in GNU cp.
setup
cpx -l foo foo 2>/dev/null || fail=1
check_foo "cpx -l foo foo" || fail=1
cd ..
setup
cpx -f -l foo foo 2>/dev/null || fail=1
check_foo "cpx -fl foo foo" || fail=1
cd ..

# --- 2. destination is a symlink to the source: cp foo symlink.
setup
if cpx foo symlink 2>/dev/null; then fail=1; fi
check_foo "cpx foo symlink" || fail=1
test -h symlink || fail=1
cd ..

setup
if cpx -f foo symlink 2>/dev/null; then fail=1; fi
check_foo "cpx -f foo symlink" || fail=1
test -h symlink || fail=1
cd ..

# --remove-destination: the symlink is removed and replaced by a copy.
setup
cpx --remove-destination foo symlink 2>/dev/null || fail=1
check_foo "cpx --rem foo symlink" || fail=1
test -f symlink && ! test -h symlink || fail=1
test "$(cat symlink)" = "$contents" || fail=1
cd ..

# -b: the symlink is backed up and replaced by a regular copy.
setup
cpx --backup=numbered foo symlink 2>/dev/null || fail=1
check_foo "cpx -b foo symlink" || fail=1
test -h symlink.~1~ || fail=1
test -f symlink && ! test -h symlink || fail=1
cd ..

# -l: cannot hard link over the existing symlink; -fl replaces it.
setup
if cpx -l foo symlink 2>/dev/null; then fail=1; fi
check_foo "cpx -l foo symlink" || fail=1
test -h symlink || fail=1
cd ..
setup
cpx -f -l foo symlink 2>/dev/null || fail=1
check_foo "cpx -fl foo symlink" || fail=1
test ! -h symlink || fail=1
test "$(stat -c %i foo)" = "$(stat -c %i symlink)" || fail=1
cd ..

# -s: cannot create the symlink over an existing one; -sf succeeds.
setup
if cpx --symbolic-link=auto foo symlink 2>/dev/null; then fail=1; fi
check_foo "cpx -s foo symlink" || fail=1
cd ..
setup
cpx --symbolic-link=auto -f foo symlink 2>/dev/null || fail=1
check_foo "cpx -sf foo symlink" || fail=1
test -h symlink || fail=1
test "$(cat symlink)" = "$contents" || fail=1
cd ..

# --- 3. source is a symlink to the destination: cp symlink foo.
for opt in '' -f --remove-destination --backup=numbered; do
  setup
  if cpx $opt symlink foo 2>/dev/null; then
    echo "cpx $opt symlink foo: expected failure" >&2; fail=1
  fi
  check_foo "cpx $opt symlink foo" || fail=1
  cd ..
done
setup
cpx -l symlink foo 2>/dev/null || fail=1
check_foo "cpx -l symlink foo" || fail=1
cd ..

# --- 4. two symlinks to the same file: cp sl1 sl2.
setup
if cpx sl1 sl2 2>/dev/null; then fail=1; fi
check_foo "cpx sl1 sl2" || fail=1
test -h sl2 || fail=1
cd ..
setup
cpx --remove-destination sl1 sl2 2>/dev/null || fail=1
check_foo "cpx --rem sl1 sl2" || fail=1
test -f sl2 && ! test -h sl2 || fail=1
test "$(cat sl2)" = "$contents" || fail=1
cd ..
setup
cpx --backup=numbered sl1 sl2 2>/dev/null || fail=1
check_foo "cpx -b sl1 sl2" || fail=1
test -h sl2.~1~ || fail=1
cd ..
setup
cpx --symbolic-link=auto -f sl1 sl2 2>/dev/null || fail=1
check_foo "cpx -sf sl1 sl2" || fail=1
test "$(readlink sl2)" = sl1 || fail=1
cd ..

# --- 5. destination is a hard link of the source: cp foo hardlink.
for opt in '' -f --symbolic-link=auto; do
  setup
  if cpx $opt foo hardlink 2>/dev/null; then
    echo "cpx $opt foo hardlink: expected failure" >&2; fail=1
  fi
  check_foo "cpx $opt foo hardlink" || fail=1
  test "$(stat -c %i foo)" = "$(stat -c %i hardlink)" || fail=1
  cd ..
done
setup
cpx --remove-destination foo hardlink 2>/dev/null || fail=1
check_foo "cpx --rem foo hardlink" || fail=1
test "$(cat hardlink)" = "$contents" || fail=1
test "$(stat -c %i foo)" != "$(stat -c %i hardlink)" || fail=1
cd ..
setup
cpx --backup=numbered foo hardlink 2>/dev/null || fail=1
check_foo "cpx -b foo hardlink" || fail=1
test -f hardlink.~1~ || fail=1
cd ..
setup
cpx -l foo hardlink 2>/dev/null || fail=1
check_foo "cpx -l foo hardlink" || fail=1
test "$(stat -c %i foo)" = "$(stat -c %i hardlink)" || fail=1
cd ..

# --- 6. explicit --remove-destination with a differently spelled path.
setup
if cpx --remove-destination foo ./foo 2>/dev/null; then fail=1; fi
check_foo "cpx --rem foo ./foo" || fail=1
cd ..

exit $fail
