#!/bin/sh
# Verify that --preserve keeps the owner and group of the source when the
# copying user is allowed to, and that a plain copy gets the copier's ids.
# Needs root (to chown to arbitrary ids and to run as an unprivileged user),
# python3 (to find unused uid/gid values) and a chroot that supports --user.
#
# Inspired by GNU coreutils test: tests/cp/preserve-gid.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
# Changing file ownership to arbitrary ids requires root.
[ "$(id -u)" -eq 0 ] || exit 77
command -v python3 >/dev/null 2>&1 || exit 77
# GNU chroot --user is needed to rerun cpx as an unprivileged id.
chroot --skip-chdir --user=+0:+0 / true >/dev/null 2>&1 || exit 77

# The checks assume the umask leaves group/other read access.
case $(umask) in
  *[0-3][0-3]) ;;
  *) exit 77 ;;
esac

# mktemp under /tmp so the unprivileged ids can reach it.
tmp="$(TMPDIR=/tmp mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

preserve='--preserve=mode,ownership,timestamps'
primary_group_num=$(id -g)

create() {
  echo "$1" > "$1" || exit 1
  chown "+$2:+$3" "$1" || exit 1
}

# t0 FILE UID GID CMD...: copy FILE to b with CMD and check b's owner/group.
t0() {
  f=$1; shift
  u=$1; shift
  g=$1; shift
  rm -f b || exit 1
  "$@" "$f" b || exit 1
  s=$(stat -c '%u %g' b)
  if test "x$s" != "x$u $g"; then
    # A group inherited from the parent directory is also acceptable.
    if test "x$s" = "x$u $primary_group_num"; then
      :
    else
      echo "$* $f b: $u $g != $s" >&2
      fail=1
    fi
  fi
}

nameless_uid=$(python3 -c '
import pwd
for i in range(1000, 16*1024):
    try: pwd.getpwuid(i)
    except KeyError: print(i); break
')
nameless_gid1=$(python3 -c '
import grp
for i in range(1000, 16*1024+1):
    try: grp.getgrgid(i)
    except KeyError: print(i); break
')
nameless_gid2=$(python3 -c '
import grp
for i in range('"$nameless_gid1"'+1, 16*1024+1):
    try: grp.getgrgid(i)
    except KeyError: print(i); break
')

# No unused ids available: nothing to test with.
if test -z "$nameless_uid" || test -z "$nameless_gid1" || test -z "$nameless_gid2"; then
  exit 77
fi

chown "+$nameless_uid:+0" .

create a0 0 0
create b0 "$nameless_uid" "$nameless_gid1"
create b1 "$nameless_uid" "$nameless_gid2"
create c0 0 "$nameless_gid1"
create c1 0 "$nameless_gid2"

# Root without --preserve: everything becomes root-owned.
t0 a0 0 0 cpx
t0 b0 0 0 cpx
t0 b1 0 0 cpx
t0 c0 0 0 cpx
t0 c1 0 0 cpx

# Root with --preserve: ids are kept.
t0 a0 0 0 cpx $preserve
t0 b0 "$nameless_uid" "$nameless_gid1" cpx $preserve
t0 b1 "$nameless_uid" "$nameless_gid2" cpx $preserve
t0 c0 0 "$nameless_gid1" cpx $preserve
t0 c1 0 "$nameless_gid2" cpx $preserve

# Put a copy of cpx where the nameless user can execute it.
tmp_path=$(TMPDIR=/tmp mktemp -d) || exit 1
cp "$(command -v cpx)" "$tmp_path/cpx" || exit 1
chmod -R a+rx "$tmp_path"

t1() {
  f=$1; shift
  u=$1; shift
  g=$1; shift
  t0 "$f" "$u" "$g" \
      chroot --skip-chdir \
             --user=+$nameless_uid:+$nameless_gid1 \
             --groups="+$nameless_gid1,+$nameless_gid2" \
        / env PATH="$tmp_path" HOME=/nonexistent "$@"
}

# Unprivileged copier without --preserve: gets its own uid and primary gid.
t1 a0 "$nameless_uid" "$nameless_gid1" cpx
t1 b0 "$nameless_uid" "$nameless_gid1" cpx
t1 b1 "$nameless_uid" "$nameless_gid1" cpx
t1 c0 "$nameless_uid" "$nameless_gid1" cpx
t1 c1 "$nameless_uid" "$nameless_gid1" cpx

# Unprivileged copier with --preserve: may keep a gid it is a member of,
# but never a foreign uid.
t1 a0 "$nameless_uid" "$nameless_gid1" cpx $preserve
t1 b0 "$nameless_uid" "$nameless_gid1" cpx $preserve
t1 b1 "$nameless_uid" "$nameless_gid2" cpx $preserve
t1 c0 "$nameless_uid" "$nameless_gid1" cpx $preserve
t1 c1 "$nameless_uid" "$nameless_gid2" cpx $preserve

rm -rf "$tmp_path"

exit $fail
