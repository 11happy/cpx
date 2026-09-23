# cpx

<div align="center">

**A modern, fast file copy tool for Linux with progress bars, resume capability, and more.**

[![Crates.io](https://img.shields.io/crates/v/cpx.svg)](https://crates.io/crates/cpx)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![CI](https://github.com/11happy/cpx/actions/workflows/ci.yml/badge.svg)](https://github.com/11happy/cpx/actions/workflows/ci.yml)


[Features](#features) •
[Installation](#installation) •
[Quick Start](#quick-start) •
[Documentation](#documentation)

</div>

---

## Why cpx?

`cpx` is a modern replacement for the traditional `cp` command, built with Rust for maximum performance and safety on Linux systems.

![one](https://github.com/user-attachments/assets/85fdbe39-2635-41b0-a00a-27ba7d2e8e60)

## Features
- 🚀 Fast parallel copying (upto 5x faster than cp [benchmarks](docs/benchmarks.md))
- 📊 Beautiful progress bars (customizable)
- ⏸️ Resume interrupted transfers
- 🛑 Graceful Ctrl+C handling with resume hints
  ![four](https://github.com/user-attachments/assets/11c9ecb8-ea57-4162-9772-bdf071f61848)
- 🎯 Exclude patterns (gitignore-style)
- 🕳️ Sparse files stay sparse (SEEK_DATA/SEEK_HOLE), reflink/CoW copies where supported
- 🧪 Tested against the GNU coreutils `cp` test suite ([details](docs/gnu-compat.md))
- ⚙️ Flexible configuration





## Installation

### Quick Install
```bash
curl -fsSL https://raw.githubusercontent.com/11happy/cpx/main/install.sh | bash
```

Or with wget:
```bash
wget -qO- https://raw.githubusercontent.com/11happy/cpx/main/install.sh | bash
```

### From Crates.io
```bash
cargo install cpx
```

### Arch Linux (AUR)
> Added by community
```bash
yay -S cpx-copy
```


### Nix / NixOS
> Added by community
```bash
nix-shell -p cpx
```


### From Source
```bash
cargo install --git https://github.com/11happy/cpx
cpx --version
```

### Pre-built Binaries

Download from [Releases](https://github.com/11happy/cpx/releases)

## Quick Start

### Basic Usage
```bash
# Copy a file
cpx source.txt dest.txt

# Copy directory recursively
cpx -r source_dir/ dest_dir/

# exclude build artifacts
cpx -r -e "node_modules" -e ".git" -e "target" my-project/ /backup/

# Resume interrupted transfer
cpx -r --resume large_dataset/ /backup/

# Copy with full attribute preservation
cpx -r -p=all photos/ /backup/photos/
```

**See [examples.md](docs/examples.md) for detailed workflows and real-world scenarios.**

## Key Options
```
cpx [OPTIONS] <SOURCE>... <DESTINATION>
cpx [OPTIONS] -t <DIRECTORY> <SOURCE>...

Arguments:
  <SOURCE>...       Source file(s) or directory(ies)
  <DESTINATION>     Destination file or directory (omitted with -t)

Input/Output Options:
  -t, --target-directory <DIRECTORY>
                           Copy all SOURCE arguments into DIRECTORY
  -e, --exclude <PATTERN>  Exclude files matching pattern (supports globs, comma-separated)

Copy Behavior:
  -r, -R, --recursive      Copy directories recursively
  -a, --archive            Same as -r --no-dereference --preserve=all
  -j <N>                   Number of parallel operations [default: 4]
  -v, --verbose            Print each 'src' -> 'dst' as it is copied
  -u, --update             Copy only when SOURCE is newer than DEST or DEST is missing
  -n, --no-clobber         Never overwrite an existing file
      --resume             Resume interrupted transfers (checksum verified)
  -f, --force              Remove and retry if destination cannot be opened
  -i, --interactive        Prompt before overwrite
      --parents            Use full source file name under DIRECTORY
      --attributes-only    Copy only attributes, not file data
      --remove-destination Remove destination file before copying

Link and Symlink Options:
  -s, --symbolic-link[=MODE]
                           Create symlinks instead of copying [auto|absolute|relative]
  -l, --link               Create hard links instead of copying
  -P, --no-dereference     Never follow symbolic links in SOURCE
  -L, --dereference        Always follow symbolic links in SOURCE
  -H, --dereference-command-line
                           Follow symbolic links only on command line

Preservation:
  -p, --preserve[=ATTRS]   Preserve attributes [default|all|mode,timestamps,ownership,...]
                           Available: mode, ownership, timestamps, links, context, xattr

Backup and Reflink:
  -b, --backup[=MODE]      Backup existing files [none|simple|numbered|existing]
      --reflink[=WHEN]     CoW copy if supported [auto|always|never]

Configuration:
      --config <PATH>      Use custom config file
      --no-config          Ignore all config files

Other:
  -h, --help               Print help information
  -V, --version            Print version information
```


For complete usage examples, see [examples.md](docs/examples.md)

For complete option reference, run `cpx --help`

## Configuration

Set defaults with configuration files:
```bash
# Create config with defaults
cpx config init

# View active configuration
cpx config show

# See config file location
cpx config path
```

**Config locations (in priority order):**
1. `./cpxconfig.toml` (project-level)
2. `~/.config/cpx/cpxconfig.toml` (user-level)
3. `/etc/cpx/cpxconfig.toml` (system-level, Unix only)

**Example config** (`~/.config/cpx/cpxconfig.toml`):
```toml
[exclude]
patterns = ["*.tmp", "*.log", "node_modules", ".git"]

[copy]
parallel = 8
recursive = false

[preserve]
mode = "default"

[progress]
style = "detailed"

[reflink]
mode = "auto"
```

**See [configuration.md](docs/configuration.md) for all options and use cases.**

## Performance

`cpx` is built for speed. Measured on v0.2.0 (24-core Linux box, tmpfs, warm
cache, 5 hyperfine runs; `cp` and `cpx -j4` are the defaults, `-j16` is what
the [benchmarks](docs/benchmarks.md) use):

| Tree | files | cp | cpx -j4 | cpx -j16 | xcp -w16 | cpz |
|------|-------|----|---------|----------|----------|-----|
| rust-lang/rust | 63k | 415 ms | 227 ms | **141 ms** | 193 ms | 61 ms |
| torvalds/linux (2.1 GB) | 96k | 962 ms | 430 ms | **240 ms** | 348 ms | 180 ms |
| 200k small files | 200k | 942 ms | 520 ms | **262 ms** | 380 ms | n/a |

v0.1.4 had a quadratic planning step: the rust tree took **78 s**, the linux
tree **154 s**, and the 200k-file tree did not finish in ten minutes. Upgrade
if you copy large trees.

**See [benchmarks.md](docs/benchmarks.md) for methodology and more comparisons.**

## Documentation

- **[Configuration Guide](docs/configuration.md)** - Complete config reference
- **[Benchmarks](docs/benchmarks.md)** - Performance analysis and comparisons
- **[GNU cp compatibility](docs/gnu-compat.md)** - The ported coreutils test suite and the known differences
- **[Contributing](CONTRIBUTING.md)** - How to contribute

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| **Linux** | ✅ Supported | copy_file_range fast path (kernel 4.5+), hole-preserving sparse copies |
| **macOS** | ✅ Supported | built and tested in CI, binaries on the releases page |
| Windows | 🔄 Planned | To be released |

## Quick Start for Developers
```bash
git clone https://github.com/11happy/cpx.git
cd cpx

# Run tests
cargo test

# Run clippy
cargo clippy

# Try it out
cargo run -- -r test_data/ test_dest/
```

## Tests

All 68 tests of the [GNU coreutils cp test suite](https://github.com/coreutils/coreutils/tree/master/tests/cp) are ported as independent reimplementations under [tests/gnu](tests/gnu) and run in CI: 49 pass, 10 are documented differences from GNU cp, 9 need root/SELinux or options cpx does not have. See [docs/gnu-compat.md](docs/gnu-compat.md).

```bash
cargo build --release
PATH=target/release:$PATH tests/gnu/run.sh
```

Found wrong behavior? [File an issue](https://github.com/11happy/cpx/issues), PRs for more tests are always welcome!

## License

- MIT [LICENSE](https://github.com/11happy/cpx/blob/main/LICENSE)


## Acknowledgments

Inspired by `ripgrep`, `fd`, and the modern Rust CLI ecosystem.

Built with: [clap](https://github.com/clap-rs/clap), [indicatif](https://github.com/console-rs/indicatif), [rayon](https://github.com/rayon-rs/rayon), [jwalk](https://github.com/Byron/jwalk), and more.

---
