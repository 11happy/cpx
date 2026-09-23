#!/bin/sh
# --keep-directory-symlink: when a destination directory entry is a
# symlink to a directory, keep it and copy into its target instead of
# replacing it.  cpx has neither --keep-directory-symlink, -T nor
# --copy-contents, so this test is skipped.
#
# Inspired by GNU coreutils test: tests/cp/keep-directory-symlink.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

# cpx does not implement --keep-directory-symlink (nor -T / --copy-contents)
exit 77
