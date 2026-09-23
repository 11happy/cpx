#!/bin/sh
# Run every ported GNU coreutils cp test against the cpx binary on PATH.
# Exit codes per test: 0 = pass, 77 = skipped (feature/privilege missing),
# other = fail. Names listed in expected-failures are known, documented
# differences from GNU cp (docs/gnu-compat.md) and are reported as XFAIL;
# a listed test that passes is reported as XPASS so the list stays honest.
# Usage: PATH=target/release:$PATH tests/gnu/run.sh

dir="$(cd "$(dirname "$0")" && pwd)"
pass=0; fail=0; skip=0; xfail=0; xpass=0; failed=""; xpassed=""

expected() {
    grep -qx "$1" "$dir/expected-failures" 2>/dev/null
}

for t in "$dir"/*.sh; do
    name="$(basename "$t")"
    [ "$name" = "run.sh" ] && continue
    if sh "$t" >/dev/null 2>&1; then
        if expected "$name"; then
            xpass=$((xpass + 1)); xpassed="$xpassed $name"; printf 'XPASS %s\n' "$name"
        else
            pass=$((pass + 1)); printf 'PASS  %s\n' "$name"
        fi
    else
        rc=$?
        if [ "$rc" -eq 77 ]; then
            skip=$((skip + 1)); printf 'SKIP  %s\n' "$name"
        elif expected "$name"; then
            xfail=$((xfail + 1)); printf 'XFAIL %s\n' "$name"
        else
            fail=$((fail + 1)); failed="$failed $name"; printf 'FAIL  %s (exit %s)\n' "$name" "$rc"
        fi
    fi
done

printf '\n%s passed, %s failed, %s skipped, %s expected failures, %s unexpected passes\n' \
    "$pass" "$fail" "$skip" "$xfail" "$xpass"
[ -n "$failed" ] && printf 'failed:%s\n' "$failed"
[ -n "$xpassed" ] && printf 'unexpected passes (remove from expected-failures):%s\n' "$xpassed"
[ "$fail" -eq 0 ] && [ "$xpass" -eq 0 ]
