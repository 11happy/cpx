#!/bin/sh
# Copying with -a from a filesystem that returns no SELinux context must
# not crash and must succeed (new and existing destination), while an
# explicit --preserve=context must fail when no context is available.
# GNU achieves the "no context" condition by LD_PRELOADing a stub
# getfilecon into cp; that cannot be applied to a statically linked Rust
# binary, so this port only runs the -a copies on an SELinux system.
#
# Inspired by GNU coreutils test: tests/cp/no-ctx.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
# Needs an SELinux-enabled kernel.
test -d /sys/fs/selinux || exit 77
grep -q selinuxfs /proc/filesystems 2>/dev/null || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

touch file_src || exit 1

# New destination.
cpx -a file_src file_dst || fail=1
# Existing destination.
cpx -a file_src file_dst || fail=1

exit $fail
