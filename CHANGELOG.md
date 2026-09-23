# Changelog

## 0.2.0 - 2026-09-23

### Performance
- Planning was O(n²) in the number of files in 0.1.4 (the rust tree took 78 s,
  the linux tree 154 s, 200k files never finished); it is linear now. Metadata lookups and exclude
  matching run on the walker's thread pool and excluded directories are pruned.
- Copies are scheduled largest-first across all workers: linux tree at -j16
  439 ms -> 240 ms, rust tree 163 ms -> 141 ms (tmpfs, warm cache).
- One `copy_file_range` per regular file; sparse files are copied hole-for-hole.

### New options
- `-v/--verbose`, `-u/--update`, `-a/--archive`, `-n/--no-clobber`, `-R` as an
  alias of `-r`. `-t DIR` now works with a single source (#16).
- Files of 64 MiB and more get their own progress bar under the overall one (#6).
- `-p`, `-s`, `-b` and `--reflink` take their optional value only with `=`
  (`-p=all`, `-s=relative`), so `cpx -p a b` no longer swallows a path.

### Fixes
- Copying a file onto itself, onto a symlink to itself or onto a hard link of
  itself truncated the source; refused now. `--backup=simple a~ a` destroyed
  the source; refused now. `--backup=numbered` failed on bare file names.
- Copying through a dangling destination symlink created its target; `cpx -r dir dir`
  copied a directory into itself; a FIFO inside a recursive copy hung forever;
  non-UTF-8 file names panicked; `--link` on a symlink source created nothing.
- `--preserve=links` raced against the parallel copy; links are now resolved on
  the plan and created after the copy phase.
- Directory modes and timestamps are preserved (deepest first), including for
  the directories `--parents` creates; POSIX ACLs travel with `--preserve=mode`;
  new files get the source mode & ~umask like GNU cp; xattrs are applied before
  the mode is narrowed.
- Existing destination symlinks are replaced; `--attributes-only` creates a
  missing destination and never replaces a file with a symlink; procfs/sysfs
  files that report size 0 are copied with their content; `cpx -r dir/. dest`
  copies the contents; declining an `-i` prompt exits 1.
- `preserve = "none"` / `"default"` in the config were not recognised.
- Builds on macOS (#1); macOS binaries are part of the release.
- Integration tests no longer pick up the developer's own config (#14) and no
  longer race on the process working directory.

### Testing
- All 68 GNU coreutils `cp` tests are ported under `tests/gnu` and run in CI
  (49 pass, 10 documented differences, 9 skipped). See docs/gnu-compat.md.

## 0.1.4 and earlier

See the GitHub releases page.
