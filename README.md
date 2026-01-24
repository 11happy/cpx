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
- 🎯 Exclude patterns (gitignore-style)
- ⚙️ Flexible configuration
- 🛑 Graceful Ctrl+C handling with resume hints


## Installation

### Prerequisites

- **Linux** (kernel 4.5+ recommended for fast copy)
- **Rust** 1.70 or later


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

### From Source
```bash
git clone https://github.com/11happy/cpx.git
cd cpx
cargo install --path .
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

# Copy with progress bar
cpx -r large_dir/ /backup/
```

### Common Use Cases
```bash
# Simple copy
cpx source.txt destination.txt

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

Arguments:
  <SOURCE>...       Source file(s) or directory(ies)
  <DESTINATION>     Destination file or directory

Input/Output Options:
  -t, --target-directory <DIRECTORY>
                           Copy all SOURCE arguments into DIRECTORY
  -e, --exclude <PATTERN>  Exclude files matching pattern (supports globs, comma-separated)

Copy Behavior:
  -r, --recursive          Copy directories recursively
  -j <N>                   Number of parallel operations [default: 4]
      --resume             Resume interrupted transfers (checksum verified)
  -f, --force              Remove and retry if destination cannot be opened
  -i, --interactive        Prompt before overwrite
      --parents            Use full source file name under DIRECTORY
      --attributes-only    Copy only attributes, not file data
      --remove-destination Remove destination file before copying

Link and Symlink Options:
  -s, --symbolic-link [MODE]
                           Create symlinks instead of copying [auto|absolute|relative]
  -l, --link               Create hard links instead of copying
  -P, --no-dereference     Never follow symbolic links in SOURCE
  -L, --dereference        Always follow symbolic links in SOURCE
  -H, --dereference-command-line
                           Follow symbolic links only on command line

Preservation:
  -p, --preserve [ATTRS]   Preserve attributes [default|all|mode,timestamps,ownership,...]
                           Available: mode, ownership, timestamps, links, context, xattr

Backup and Reflink:
  -b, --backup [MODE]      Backup existing files [none|simple|numbered|existing]
      --reflink [WHEN]     CoW copy if supported [auto|always|never]

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

`cpx` is built for speed. Quick comparison:

| Task | cp | cpx | speedup |
|------|-----|-------|-----|
| VsCode (~15k files) | 1084ms | 263ms | 4.12x |
| rust (~65k files) | 4.553s | 1.091s  |  4.17x |

**See [benchmarks.md](docs/benchmarks.md) for detailed methodology and more comparisons.**

## Documentation

- **[Examples](docs/examples.md)** - Real-world usage patterns and workflows
- **[Configuration Guide](docs/configuration.md)** - Complete config reference
- **[Benchmarks](docs/benchmarks.md)** - Performance analysis and comparisons
- **[Contributing](CONTRIBUTING.md)** - How to contribute

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| **Linux** | ✅ Supported | Fast copy supported for (kernel 4.5+) |
| macOS | 🔄 Planned | Basic support coming soon |
| Windows | 🔄 Planned | Future release |


**Linux-specific optimizations:**
- `copy_file_range` syscall (kernel 4.5+)
- SELinux context preservation
- Extended attributes support

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Quick Start for Developers
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

## Roadmap

**Current (v0.1)**
- [x] Core copy functionality
- [x] Progress bars
- [x] Resume capability
- [x] Exclude patterns
- [x] Configuration system
- [x] Reflink support
- [x] Hard link preservation

**Upcoming (v0.2)**
- [ ] macOS support
- [ ] Windows support


## License

- MIT [LICENSE](https://github.com/11happy/cpx/blob/main/LICENSE)


## Acknowledgments

Inspired by `ripgrep`, `fd`, and the modern Rust CLI ecosystem.

Built with: [clap](https://github.com/clap-rs/clap), [indicatif](https://github.com/console-rs/indicatif), [rayon](https://github.com/rayon-rs/rayon), [jwalk](https://github.com/Byron/jwalk), and more.

---
