#!/bin/sh
# Check that -i prompts before overwriting and that the answer is honoured:
# answering "n" leaves the destination alone and yields a non-zero exit,
# answering "y" overwrites.  Also check how -i interacts with -n, -f, -u
# and -b (last of -i/-n wins; -b with -n is rejected; -u still prompts).
# cpx writes its prompt to stdout (GNU cp uses stderr), so the prompt text
# is stripped before the verbose output is compared.
#
# Inspired by GNU coreutils test: tests/cp/cp-i.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

# drop a leading "overwrite '...'? (y/n): " prompt from captured stdout
strip() { sed 's/^overwrite .*? (y\/n): //' "$1" > "$2"; }

mkdir -p a b/a/c || exit 1
touch a/c || exit 1

# A regular file a/c would land on the directory b/a/c: cp must prompt,
# and after "n" must exit non-zero, leaving the directory in place.
echo n | cpx -i -r a b >/dev/null 2>&1 && fail=1
test -d b/a/c || fail=1

touch c d || exit 1
echo "'c' -> 'd'" > out_copy || exit 1
: > out_empty || exit 1

# prompt, answer no: nothing copied, non-zero exit, no verbose line
echo n | cpx -vi c d 2>/dev/null > raw1 && fail=1
strip raw1 out1
cmp out1 out_empty >/dev/null || fail=1

# prompt, answer yes
echo y | cpx -vi c d 2>/dev/null > raw2 || fail=1
strip raw2 out2
cmp out2 out_copy >/dev/null || fail=1

# -i given after -n wins: prompt and copy
echo y | cpx -v -n -i c d 2>/dev/null > raw3 || fail=1
strip raw3 out3
cmp out3 out_copy >/dev/null || fail=1

# -n given after -i wins: no prompt, no copy, exit 0
echo y | cpx -v -i -n c d 2>/dev/null > out4 || fail=1
cmp out4 out_empty >/dev/null || fail=1

# same without -v: silent on stderr as well
echo y | cpx -i -n c d 2>err4 > out4 || fail=1
cmp /dev/null err4 || fail=1
cmp out4 out_empty >/dev/null || fail=1

# -f then -i: still prompts, answer yes copies
echo y | cpx -v -f -i c d 2>/dev/null > raw5 || fail=1
strip raw5 out5
cmp out5 out_copy >/dev/null || fail=1

# -f with -n: no prompt, no overwrite
echo n | cpx -v -f -n c d 2>/dev/null > out6 || fail=1
cmp out6 out_empty >/dev/null || fail=1
echo n | cpx -v -n -f c d 2>/dev/null > out7 || fail=1
cmp out7 out_empty >/dev/null || fail=1

# --backup and --no-clobber are mutually exclusive
cpx -b -n c d 2>/dev/null && fail=1

# -i combined with -u must still prompt when the source is newer
echo old > old || exit 1
touch -d yesterday old || exit 1
echo new > new || exit 1
echo n | cpx -vi -u new old 2>/dev/null > raw8 && fail=1
strip raw8 out8
cmp /dev/null out8 || fail=1
test "$(cat old)" = old || fail=1

exit $fail
