#!/bin/sh
# Run every ported GNU coreutils cp test against the cpx binary on PATH.
# Exit codes per test: 0 = pass, 77 = skipped (feature/privilege missing), other = fail.
# Usage: PATH=target/release:$PATH tests/gnu/run.sh

dir="$(cd "$(dirname "$0")" && pwd)"
pass=0; fail=0; skip=0; failed=""

for t in "$dir"/*.sh; do
    name="$(basename "$t")"
    [ "$name" = "run.sh" ] && continue
    if sh "$t" >/dev/null 2>&1; then
        pass=$((pass + 1)); printf 'PASS %s\n' "$name"
    else
        rc=$?
        if [ "$rc" -eq 77 ]; then
            skip=$((skip + 1)); printf 'SKIP %s\n' "$name"
        else
            fail=$((fail + 1)); failed="$failed $name"; printf 'FAIL %s (exit %s)\n' "$name" "$rc"
        fi
    fi
done

printf '\n%s passed, %s failed, %s skipped\n' "$pass" "$fail" "$skip"
[ -n "$failed" ] && printf 'failed:%s\n' "$failed"
[ "$fail" -eq 0 ]
