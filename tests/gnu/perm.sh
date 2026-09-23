#!/bin/sh
# Exercise the permission handling of cpx when creating a new destination
# and when overwriting an existing one, with and without --preserve and -f,
# under several umasks.  Expected results follow GNU cp:
#   - new destination, no --preserve: source mode with the umask applied
#   - new destination, --preserve:    source mode exactly
#   - existing destination, no --preserve: destination mode left untouched
#   - existing destination, --preserve:    source mode exactly
# Only the cp cases of the GNU test are ported (mv/ln are out of scope).
#
# Inspired by GNU coreutils test: tests/cp/perm.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# cpx has no bare -p; spell out the GNU -p attribute set.
preserve='--preserve=mode,ownership,timestamps'

for u in 31 37 2; do
  umask $u
  for cmd in preserve plain; do
    for force in '' -f; do
      for existing_dest in yes no; do
        for g_perm in r w x rwx; do
          for o_perm in r w x rwx; do
            touch src || exit 1
            chmod u=r,g=rx,o= src || exit 1
            expected_perms=$(stat --format=%A src)
            rm -f dest
            if [ $existing_dest = yes ]; then
              touch dest || exit 1
              chmod u=rw,g=$g_perm,o=$o_perm dest || exit 1
            fi
            if [ $cmd = preserve ]; then
              cpx $preserve $force src dest || exit 1
            else
              cpx $force src dest || exit 1
            fi
            test -f src || exit 1
            actual_perms=$(stat --format=%A dest)

            case "$cmd:$existing_dest" in
              plain:yes)
                _g_perm=$(echo rwx | sed 's/[^'$g_perm']/-/g')
                _o_perm=$(echo rwx | sed 's/[^'$o_perm']/-/g')
                expected_perms=-rw-$_g_perm$_o_perm
                ;;
              plain:no)
                test $u = 37 &&
                  expected_perms=$(echo $expected_perms | sed 's/.....$/-----/')
                test $u = 31 &&
                  expected_perms=$(echo $expected_perms | sed 's/..\(..\).$/--\1-/')
                ;;
            esac
            if [ "_$actual_perms" != "_$expected_perms" ]; then
              echo "umask=$u cmd=$cmd force='$force' existing=$existing_dest" \
                   "g=$g_perm o=$o_perm: got $actual_perms, want $expected_perms" >&2
              fail=1
            fi
            # A missing destination does not depend on g/o perms: one pass is enough.
            test $existing_dest = no && break 2
          done
        done
      done
    done
  done
done

exit $fail
