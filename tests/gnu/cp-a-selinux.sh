#!/bin/sh
# SELinux contexts: -a, --preserve=context and --preserve=all must carry
# the source context to the copy (and -a must do so silently); when
# copying into existing directories their context is reset to the source's;
# with --parents the intermediate directories created get the source
# context.  Needs root and an SELinux-enabled kernel.  The GNU -Z /
# --context and fixed-context mount cases are not ported: cpx has neither
# option.
#
# Inspired by GNU coreutils test: tests/cp/cp-a-selinux.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
# Setting arbitrary contexts requires root.
[ "$(id -u)" -eq 0 ] || exit 77
# Needs an SELinux-enabled kernel and the chcon tool.
test -d /sys/fs/selinux || exit 77
grep -q selinuxfs /proc/filesystems 2>/dev/null || exit 77
command -v chcon >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# A context that works without mcstransd.
ctx='root:object_r:tmp_t'
if test -r /sys/fs/selinux/mls && test "$(cat /sys/fs/selinux/mls)" = 1; then
  ctx="$ctx:s0"
fi

touch c || exit 1
chcon $ctx c || exit 77   # cannot set the test context here

cpx -a c d 2>err || exit 1
cpx --preserve=context c e || exit 1
cpx --preserve=all c f || exit 1
ls -Z d | grep $ctx || fail=1
test -s err && { cat err >&2; fail=1; }
ls -Z e | grep $ctx || fail=1
ls -Z f | grep $ctx || fail=1
rm -f f

# Existing destination directories must have their context updated.
mkdir -p backup/existing_dir/ || exit 1
ls -Zd backup/existing_dir > ed_ctx || fail=1
grep $ctx ed_ctx && exit 1
touch backup/existing_dir/file || exit 1
chcon $ctx backup/existing_dir/file || exit 1
mkdir -p restore/existing_dir || exit 1
chcon $ctx restore/existing_dir || exit 1
cpx -a backup/. restore/ || fail=1
ls -Zd restore/existing_dir > ed_ctx || fail=1
grep $ctx ed_ctx && { ls -lZd restore/existing_dir; fail=1; }

# --parents: created directories get the source context, existing ones
# are updated to match.
mkdir -p parents/a/b || exit 1
ls -Zd parents/a/b > ed_ctx || fail=1
grep $ctx ed_ctx && exit 1
touch parents/a/b/file || exit 1
chcon $ctx parents/a/b || exit 1
mkdir -p parents_dest/parents/a || exit 1
chcon $ctx parents_dest/parents/a || exit 1
cpx -r --parents --preserve=context parents/a/b/file parents_dest || fail=1
ls -Zd parents_dest/parents/a/b > ed_ctx || fail=1
grep $ctx ed_ctx || { ls -lZd parents_dest/parents/a/b; fail=1; }
ls -Zd parents_dest/parents/a > ed_ctx || fail=1
grep $ctx ed_ctx && { ls -lZd parents_dest/parents/a; fail=1; }

exit $fail
