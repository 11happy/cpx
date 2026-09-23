#!/bin/sh
# Exercise --link with symlink sources under every dereference option
# (default, -L, -H, -P) both with and without -R.  Without -P a symlink
# to a file must yield a hard link to the target, a symlink to a
# directory is refused without -r and copied as a fresh directory with
# -R, and a dangling symlink is an error.  With -P the destination must
# be a hard link to the symlink itself.
#
# Inspired by GNU coreutils test: tests/cp/link-deref.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# Can this file system hard-link a symlink itself?
ln -s testtarget test_sl || exit 1
can_hardlink_symlink=0
if ln -P test_sl test_hl_sl 2>/dev/null &&
   [ "$(stat -c %i test_sl)" = "$(stat -c %i test_hl_sl)" ]; then
  can_hardlink_symlink=1
fi

mkdir dir || exit 1
: > file || exit 1
ln -s dir dirlink || exit 1
ln -s file filelink || exit 1
ln -s nowhere danglink || exit 1

check() {
  # $1 = description, $2 = expected exit (0/1), $3 = expected inode or '',
  # $4 = expected file type or ''
  desc=$1; exp_rc=$2; exp_ino=$3; exp_typ=$4
  ino_dst="$(stat -c %i dst 2>/dev/null)" || ino_dst=
  typ_dst="$(stat -c %F dst 2>/dev/null)" || typ_dst=
  if [ "$rc" != "$exp_rc" ] || [ "$ino_dst" != "$exp_ino" ] ||
     [ "$typ_dst" != "$exp_typ" ]; then
    echo "FAIL: $desc: rc=$rc (want $exp_rc)" \
         "inode=$ino_dst (want $exp_ino) type=$typ_dst (want $exp_typ)" >&2
    fail=1
  fi
}

for src in dirlink filelink danglink; do
  tgt=$(readlink $src) || exit 1
  ino_src="$(stat -c %i $src)" || exit 1
  typ_src="$(stat -c %F $src)" || exit 1
  ino_tgt="$(stat -c %i $tgt 2>/dev/null)" || ino_tgt=
  typ_tgt="$(stat -c %F $tgt 2>/dev/null)" || typ_tgt=

  for o in '' -L -H -P; do
    [ "$o" = -P ] && [ $can_hardlink_symlink -eq 0 ] && continue
    for r in '' -r; do
      rc=0
      # shellcheck disable=SC2086
      cpx --link $o $r $src dst 2>/dev/null || rc=$?
      [ "$rc" -ne 0 ] && rc=1
      desc="cpx --link $o $r $src dst"
      if [ "$o" = -P ]; then
        check "$desc" 0 "$ino_src" "$typ_src"
      elif [ $src = danglink ]; then
        check "$desc" 1 '' ''
      elif [ $src = dirlink ] && [ "$r" != -r ]; then
        check "$desc" 1 '' ''
      elif [ $src = dirlink ]; then
        # a new directory is created; only its type matters
        ino_new="$(stat -c %i dst 2>/dev/null)" || ino_new=
        check "$desc" 0 "$ino_new" 'directory'
      else
        check "$desc" 0 "$ino_tgt" "$typ_tgt"
      fi
      rm -rf dst
    done
  done
done

exit $fail
