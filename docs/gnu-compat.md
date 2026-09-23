# GNU cp compatibility

`tests/gnu/` contains one script per test in the GNU coreutils `cp` test
suite (68 scripts). Each is an independent reimplementation of the GNU test
against `cpx`; run them with:

```bash
cargo build --release
PATH=target/release:$PATH tests/gnu/run.sh
```

A test exits 0 on pass, 77 when it must be skipped (needs root, SELinux, a
second filesystem, or an option cpx does not have), and 1 on failure. The
names in `tests/gnu/expected-failures` are known differences from GNU cp
listed below; the runner reports them as `XFAIL` and fails CI only on an
unexpected failure or an unexpected pass.

Current state (Linux, non-root): 48 pass, 11 expected failures, 9 skipped.

## Intentional differences

| Test | Difference |
|------|------------|
| `link-heap.sh`, `link-preserve.sh`, `symlink-slash.sh` | `cpx -r src dest` always creates `dest/src`, also when `dest` does not exist yet. GNU cp makes `dest` itself the copy in that case. This is documented cpx behaviour ([#13](https://github.com/11happy/cpx/issues/13)); `cpx -r src/. dest` copies the contents into `dest` like GNU. |
| `perm.sh`, `preserve-mode.sh`, `existing-perm-dir.sh`, `acl.sh` | With the default configuration (`[preserve] mode = "default"`) cpx preserves mode, ownership and timestamps of every copied file and directory, including ACLs and the modes of destinations that already existed. GNU cp only does that with `-p`/`-a`. Set `mode = "none"` in the config for GNU behaviour. |
| `cp-parents.sh` | A symlink given on the command line of a non-recursive copy is copied as a symlink (cpx defaults to `-P`); GNU cp follows it. Use `-L` or `-H` to follow. |
| `cpx-i.sh` | `-n` always wins over `-i`; GNU cp lets the last of the two on the command line win. |
| `preserve-link.sh` | With `-u`/`-n`, a file that is skipped because the destination is up to date is not used as the link target for its other hard links; GNU cp still links them to the existing destination. |
| `same-file.sh` | `cpx --backup foo symlink-to-foo` is refused as "the same file"; GNU cp backs the symlink up and copies. Every other case in that test (which used to truncate or delete the source) now matches GNU. |

## Skipped tests

| Test | Reason |
|------|--------|
| `capability.sh`, `preserve-gid.sh`, `special-bits.sh`, `cp-mv-enotsup-xattr.sh` | need root |
| `cp-a-selinux.sh`, `no-ctx.sh` | need SELinux |
| `debug.sh` | `--debug` is not implemented |
| `keep-directory-symlink.sh` | `--keep-directory-symlink`, `-T` and `--copy-contents` are not implemented |
| `trailing-slash.sh` | the GNU file is an empty placeholder |

## Options cpx does not have

`-S`/`--suffix`, `-T`/`--no-target-directory`, `-d`, `-x`/`--one-file-system`,
`--sparse=WHEN` (holes are always preserved), `--no-preserve`,
`--copy-contents`, `--strip-trailing-slashes`, `--debug`, `-Z`/`--context`,
`--update=WHEN` (only `-u`), `--keep-directory-symlink`.
