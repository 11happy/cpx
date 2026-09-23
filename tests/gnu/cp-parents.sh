#!/bin/sh
# --parents: a trailing slash on the source is accepted, the full source
# path is recreated under the destination, a non-directory path component
# is rejected without creating stray directories, and with -a the modes of
# the recreated parent directories are restored from the source (including
# through a command-line symlink).  Finally --parents combined with -t and
# an absolute source path must work.
# (--no-preserve=mode is not available in cpx, so that GNU check is omitted.)
#
# Inspired by GNU coreutils test: tests/cp/cp-parents.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

umask 022
# a setgid temp directory would change the expected modes
test -g . && exit 77

mkdir foo bar || exit 1
mkdir -p a/b/c d e g || exit 1
ln -s d/a sym || exit 1
touch f || exit 1

# trailing slash on the source
cpx -r --parents foo/ bar || fail=1
test -d bar/foo || fail=1

cpx --verbose -a --parents a/b/c d > /dev/null 2>&1 || fail=1
test -d d/a/b/c || fail=1

# f is a regular file: must fail and must not create d/f
cpx --parents f/g d 2>/dev/null && fail=1
test -d d/f && fail=1

# modes of recreated parents
chmod go=w d/a || exit 1
cpx -a --parents d/a/b/c e || fail=1
cpx -a --parents sym/b/c g || fail=1
p=$(ls -ld e/d | cut -b-10);       case $p in drwxr-xr-x) ;; *) fail=1 ;; esac
p=$(ls -ld e/d/a | cut -b-10);     case $p in drwx-w--w-) ;; *) fail=1 ;; esac
p=$(ls -ld g/sym | cut -b-10);     case $p in drwx-w--w-) ;; *) fail=1 ;; esac
p=$(ls -ld e/d/a/b/c | cut -b-10); case $p in drwxr-xr-x) ;; *) fail=1 ;; esac
p=$(ls -ld g/sym/b/c | cut -b-10); case $p in drwxr-xr-x) ;; *) fail=1 ;; esac

# --parents with -t and an absolute source
mkdir dest || exit 1
if test -f /bin/ls; then
  cpx -t dest --parents --preserve=mode,ownership,timestamps /bin/ls || fail=1
  test -f dest/bin/ls || fail=1
fi

exit $fail
