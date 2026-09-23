#!/bin/sh
# --parents together with --preserve must give the intermediate directories
# it creates the same permissions as the originals, and must also fix up
# the permissions of intermediate directories that already existed.
#
# Inspired by GNU coreutils test: tests/cp/parent-perm.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# The checks assume the umask leaves group/other read access.
case $(umask) in
  *[0-3][0-3]) ;;
  *) exit 77 ;;
esac

preserve='--preserve=mode,ownership,timestamps'

mkdir -p a/b/c a/b/d e || exit 1
touch a/b/c/foo a/b/d/foo || exit 1
cpx $preserve --parents a/b/c/foo e || exit 1
test -f e/a/b/c/foo || fail=1

# Make e/a and e/a/b differ from a and a/b so the second copy has to
# bring the pre-existing destination directories back in line.
chmod g-rx e/a e/a/b || exit 1

cpx $preserve --parents a/b/d/foo e || fail=1
test -f e/a/b/d/foo || fail=1

# Map an inherited set-gid 's' to 'x' so a set-gid build tree cannot
# cause a spurious mismatch.
for dir in a a/b a/b/d; do
  want=$(stat --printf %A $dir | sed s/s/x/g)
  got=$(stat --printf %A e/$dir | sed s/s/x/g)
  if [ "$want" != "$got" ]; then
    echo "e/$dir: got $got, want $want" >&2
    fail=1
  fi
done

exit $fail
