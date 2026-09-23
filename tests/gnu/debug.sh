#!/bin/sh
# GNU cp --debug reports how each file was copied (offload, reflink,
# sparse detection) and whether it was skipped.  cpx has no --debug
# option, so there is nothing equivalent to test.
#
# Inspired by GNU coreutils test: tests/cp/debug.sh
# Independent reimplementation for CPX.

# cpx does not implement --debug.
exit 77
