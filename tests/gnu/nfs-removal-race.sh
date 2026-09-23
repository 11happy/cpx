#!/bin/sh
# Simulate a stale-NFS-attribute race: stat() on the destination claims
# it exists while open() finds nothing there.  cp must simply create the
# destination and copy the data.  A tiny LD_PRELOAD shim redirects every
# stat-family call on "d" to an existing file "d2".
#
# Inspired by GNU coreutils test: tests/cp/nfs-removal-race.sh
# Independent reimplementation for CPX.

set -eu
fail=0

command -v cpx >/dev/null 2>&1 || exit 77
command -v gcc >/dev/null 2>&1 || exit 77  # gcc needed to build the shim

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"

cat > k.c <<'CEOF' || exit 1
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <sys/stat.h>

static const char *
redirect_path (const char *path)
{
  if (path && path[0] == 'd' && path[1] == 0)
    {
      fclose (fopen ("preloaded", "w"));
      return "d2";
    }
  return path;
}

int
__xstat (int ver, const char *path, struct stat *st)
{
  static int (*real) (int, const char *, struct stat *);
  if (!real)
    real = dlsym (RTLD_NEXT, "__xstat");
  if (!real)
    return -1;
  return real (ver, redirect_path (path), st);
}

int
stat (const char *path, struct stat *st)
{
  static int (*real) (const char *, struct stat *);
  if (!real)
    real = dlsym (RTLD_NEXT, "stat");
  return real (redirect_path (path), st);
}

int
lstat (const char *path, struct stat *st)
{
  static int (*real) (const char *, struct stat *);
  if (!real)
    real = dlsym (RTLD_NEXT, "lstat");
  return real (redirect_path (path), st);
}

int
fstatat (int dirfd, const char *path, struct stat *st, int flags)
{
  static int (*real) (int, const char *, struct stat *, int);
  if (!real)
    real = dlsym (RTLD_NEXT, "fstatat");
  return real (dirfd, redirect_path (path), st, flags);
}

int
stat64 (const char *path, struct stat64 *st)
{
  static int (*real) (const char *, struct stat64 *);
  if (!real)
    real = dlsym (RTLD_NEXT, "stat64");
  return real (redirect_path (path), st);
}

int
lstat64 (const char *path, struct stat64 *st)
{
  static int (*real) (const char *, struct stat64 *);
  if (!real)
    real = dlsym (RTLD_NEXT, "lstat64");
  return real (redirect_path (path), st);
}

int
fstatat64 (int dirfd, const char *path, struct stat64 *st, int flags)
{
  static int (*real) (int, const char *, struct stat64 *, int);
  if (!real)
    real = dlsym (RTLD_NEXT, "fstatat64");
  return real (dirfd, redirect_path (path), st, flags);
}

int
statx (int dirfd, const char *path, int flags, unsigned int mask,
       struct statx *buf)
{
  static int (*real) (int, const char *, int, unsigned int, struct statx *);
  if (!real)
    real = dlsym (RTLD_NEXT, "statx");
  return real (dirfd, redirect_path (path), flags, mask, buf);
}
CEOF

gcc -shared -fPIC -o k.so k.c -ldl 2>/dev/null || exit 77  # cannot build shared library

touch d2 || exit 1
echo xyz > src || exit 1

LD_PRELOAD="${LD_PRELOAD:+$LD_PRELOAD:}./k.so" cpx src d || fail=1
test -f preloaded || exit 77  # the shim was never consulted
cmp src d || fail=1

exit $fail
