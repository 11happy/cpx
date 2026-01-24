# Configuration Guide

`cpx` supports flexible configuration through TOML files, allowing you to set default behaviors without specifying command-line flags every time.

## Table of Contents

- [Configuration File Locations](#configuration-file-locations)
- [Configuration Priority](#configuration-priority)
- [Managing Configuration](#managing-configuration)
- [Configuration Options](#configuration-options)
  - [Exclude Patterns](#exclude-patterns)
  - [Copy Settings](#copy-settings)
  - [Preserve Attributes](#preserve-attributes)
  - [Symlink Handling](#symlink-handling)
  - [Backup Settings](#backup-settings)
  - [Reflink (Copy-on-Write)](#reflink-copy-on-write)
  - [Progress Bar Customization](#progress-bar-customization)
- [Complete Configuration Example](#complete-configuration-example)
- [Use Cases](#use-cases)

## Configuration File Locations

`cpx` looks for configuration files in the following locations (in order of priority):

1. **Project-level**: `./cpxconfig.toml` (in the current directory)
2. **User-level**: `~/.config/cpx/cpxconfig.toml` (Linux/macOS)
3. **System-level**: `/etc/cpx/cpxconfig.toml` (Unix systems only)

## Configuration Priority

Settings are applied in the following order (later overrides earlier):
```
Defaults → System Config → User Config → Project Config → CLI Flags
```

**Example:**
- If `recursive = true` is set in user config
- But you run `cpx` without `-r` flag
- The user config setting will be used

**CLI flags always override config files.**

## Managing Configuration

### Initialize a New Config File

Create a config file with default settings:
```bash
cpx config init
```

This creates `~/.config/cpx/cpxconfig.toml` with sensible defaults.

To overwrite an existing config:
```bash
cpx config init --force
```

### View Current Configuration

See the effective configuration (merged from all sources):
```bash
cpx config show
```

### View Config File Location

See which config file is being used:
```bash
cpx config path
```

### Ignore All Config Files

Use the `--no-config` flag to ignore all configuration files:
```bash
cpx --no-config source.txt dest.txt
```

### Use Custom Config File

Specify a custom config file location:
```bash
cpx --config /path/to/custom.toml source.txt dest.txt
```

## Configuration Options

### Exclude Patterns

Exclude files and directories from being copied using glob patterns.
```toml
[exclude]
patterns = [
    "*.tmp",           # Exclude all .tmp files
    "*.log",           # Exclude all .log files
    "node_modules",    # Exclude node_modules directories
    ".git",            # Exclude .git directories
    "__pycache__",     # Exclude Python cache
    "*.pyc",           # Exclude Python bytecode
    "target/",         # Exclude Rust build directory
    ".DS_Store",       # Exclude macOS metadata
    "Thumbs.db",       # Exclude Windows thumbnails
]
```

**Pattern Syntax:**
- `*.ext` - Match files with extension
- `dirname` - Match directory by name (matches anywhere in path)
- `path/to/file` - Match relative path
- `/absolute/path` - Match absolute path
- `dir/` - Match directories (trailing slash)

**Multiple patterns per line:**
```toml
[exclude]
patterns = [
    "*.tmp, *.log, *.swp",  # Comma-separated patterns
]
```

**CLI Override:**
```bash
cpx -e "*.tmp" -e "node_modules" source/ dest/
```

### Copy Settings

Control default copy behavior.
```toml
[copy]
concurrency = 4              # Number of concurrent copy operations
recursive = false            # Copy directories recursively
parents = false              # Use full source path under destination
force = false                # Overwrite read-only destination files
interactive = false          # Prompt before overwrite
resume = false               # Resume interrupted transfers
attributes_only = false      # Copy only attributes, not file data
remove_destination = false   # Remove destination before copying
```

**Explanation:**

- **`concurrency`**: Number of files copied in parallel (default: 4)
  - Higher values = faster for many small files
  - Lower values = less resource usage

- **`recursive`**: Equivalent to `-r` flag
  - Set to `true` to always copy directories recursively

- **`parents`**: Equivalent to `--parents` flag
  - Preserves full source directory structure

- **`force`**: Equivalent to `-f` flag
  - Removes read-only files before copying

- **`interactive`**: Equivalent to `-i` flag
  - Prompts before overwriting existing files

- **`resume`**: Equivalent to `--resume` flag
  - Skips files that already exist and are identical

- **`attributes_only`**: Equivalent to `--attributes-only`
  - Useful for updating timestamps/permissions without copying data

- **`remove_destination`**: Equivalent to `--remove-destination`
  - Removes destination file before attempting to copy

**Example - Fast recursive copies by default:**
```toml
[copy]
recursive = true
concurrency = 8
```

### Preserve Attributes

Control which file attributes are preserved during copy.
```toml
[preserve]
mode = "default"
```

**Available modes:**

- `"none"` - Don't preserve any attributes (fastest)
- `"default"` - Preserve mode, ownership, and timestamps (recommended)
- `"all"` - Preserve everything: mode, ownership, timestamps, links, context, xattr
- Custom: `"mode,timestamps"` - Preserve specific attributes

**Custom attribute combinations:**
```toml
[preserve]
mode = "mode,timestamps,ownership"
```

**Available attributes:**
- `mode` - File permissions (rwxr-xr-x)
- `ownership` - User and group ownership (requires privileges)
- `timestamps` - Modification and access times
- `links` - Preserve hard link relationships
- `context` - SELinux security context (Linux only)
- `xattr` - Extended attributes (platform-dependent)

**CLI Override:**
```bash
cpx -p source.txt dest.txt                    # Default preservation
cpx -p=mode,timestamps source.txt dest.txt    # Custom attributes
cpx --attributes-only source.txt dest.txt     # Preserve all (no data copy)
```

### Symlink Handling

Configure how symbolic links are created and followed.
```toml
[symlink]
mode = "auto"       # How to create symlinks: "auto", "absolute", "relative"
follow = "never"    # When to follow symlinks: "never", "always", "command-line"
```

**Symlink Creation Mode (`mode`):**

- `"auto"` - Absolute if source is absolute, relative otherwise
- `"absolute"` - Always create absolute symlinks
- `"relative"` - Always create relative symlinks
- `""` (empty) - Don't create symlinks, copy normally

**Symlink Following (`follow`):**

- `"never"` - Never follow symlinks (equivalent to `-P`)
- `"always"` - Always follow symlinks (equivalent to `-L`)
- `"command-line"` - Follow only command-line symlinks (equivalent to `-H`)

**Examples:**
```toml
# Always create relative symlinks, never follow them
[symlink]
mode = "relative"
follow = "never"
```
```toml
# Follow symlinks and copy their targets
[symlink]
mode = ""
follow = "always"
```

**CLI Override:**
```bash
cpx -s source/ dest/              # Auto symlink mode
cpx -s=absolute source/ dest/     # Absolute symlinks
cpx -s=relative source/ dest/     # Relative symlinks
cpx -L source/ dest/              # Follow all symlinks
cpx -P source/ dest/              # Don't follow symlinks
cpx -H source/ dest/              # Follow command-line symlinks only
```

### Backup Settings

Automatically backup existing destination files.
```toml
[backup]
mode = "none"
```

**Available modes:**

- `"none"` - No backups (default)
- `"simple"` - Append `~` to filename (e.g., `file.txt~`)
- `"numbered"` - Use numbered backups (e.g., `file.txt.~1~`, `file.txt.~2~`)
- `"existing"` - Numbered if numbered backups exist, otherwise simple

**Examples:**
```toml
# Always create numbered backups
[backup]
mode = "numbered"
```

**Result:**
```
Original: important.txt
After copying:
- important.txt      (new file)
- important.txt.~1~  (old file backup)
```

**CLI Override:**
```bash
cpx -b source.txt dest.txt              # Use existing mode
cpx -b=numbered source.txt dest.txt     # Numbered backups
cpx -b=simple source.txt dest.txt       # Simple backups
```

### Reflink (Copy-on-Write)

Enable copy-on-write (CoW) copies on supporting filesystems (Btrfs, XFS, APFS).
```toml
[reflink]
mode = "auto"
```

**Available modes:**

- `"auto"` - Use reflink if supported, fall back to regular copy
- `"always"` - Require reflink, fail if not supported
- `"never"` - Never use reflink
- `""` (empty) - Same as never

**Supported filesystems:**
- Linux: Btrfs, XFS (with reflink support)
- macOS: APFS
- Windows: ReFS (limited support)

**Benefits:**
- Instant copies (no data duplication)
- Space-efficient until files diverge
- Perfect for snapshots and backups

**Example:**
```toml
# Try to use reflink, fall back if not available
[reflink]
mode = "auto"
```

**CLI Override:**
```bash
cpx --reflink source.txt dest.txt           # Auto mode
cpx --reflink=always source.txt dest.txt    # Require reflink
cpx --reflink=never source.txt dest.txt     # Disable reflink
```

### Progress Bar Customization

Customize the appearance and behavior of progress bars.
```toml
[progress]
style = "default"  # "default" or "detailed"

[progress.bar]
filled = "█"       # Character for filled portion
empty = "░"        # Character for empty portion
head = "▒"         # Character for progress head

[progress.color]
bar = "white"      # Progress bar color
message = "white"  # Message text color
```

**Progress Styles:**

- `"default"` - Simple progress: `Copying 45% ████░░░░ ETA:00:23`
- `"detailed"` - Detailed stats: `Copying: 42/100 ████░░░░ files 67% | 1.2GB/1.8GB | 45.3MB/s | Elapsed: 00:27 | ETA:00:16`

**Available Colors:**
`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`

**Custom Progress Bar Appearance:**
```toml
[progress]
style = "detailed"

[progress.bar]
filled = "="
empty = "·"
head = ">"

[progress.color]
bar = "cyan"
message = "green"
```

**Result:**
```
Copying: 67/100 [=========================>·········] files 67% | 1.2GB/1.8GB | 45.3MB/s | Elapsed: 00:27 | ETA:00:16
```

## Complete Configuration Example

Here's a fully documented configuration file with common settings:
```toml
# cpx configuration file
# For more information, see: https://github.com/yourusername/cpx/docs/configuration.md

# Exclude patterns (glob syntax supported)
# Example: patterns = ["*.tmp", "*.log", "node_modules", ".git"]
[exclude]
patterns = [
    "*.tmp",
    "*.log",
    "*.swp",
    ".git",
    ".svn",
    "node_modules",
    "__pycache__",
    "*.pyc",
    "target",
    ".DS_Store",
    "Thumbs.db",
]

# Copy operation settings
[copy]
concurrency = 4
recursive = false
parents = false
force = false
interactive = false
resume = false
attributes_only = false
remove_destination = false

# Preserve file attributes
# mode values: "none", "default", "all", or "mode,timestamps,ownership"
[preserve]
mode = "default"

# Symlink handling
# mode: "auto", "absolute", "relative"
# follow: "never" (-P), "always" (-L), "command-line" (-H)
[symlink]
mode = "auto"
follow = "never"

# Backup settings
# mode: "none", "simple" (~), "numbered" (~1~, ~2~), "existing"
[backup]
mode = "none"

# Copy-on-Write (reflink) settings
# mode: "auto", "always", "never"
[reflink]
mode = "auto"

# Progress bar settings
[progress]
style = "default"

# Progress bar characters
[progress.bar]
filled = "█"
empty = "░"
head = "▒"

# Supported progress bar colors: black, red, green, yellow, blue, magenta, cyan, white
[progress.color]
bar = "white"
message = "white"
```

## Use Cases

### 1. Development Environment

Skip common build artifacts and development files:
```toml
[exclude]
patterns = [
    "node_modules",
    "target",
    ".git",
    "*.pyc",
    "__pycache__",
    ".venv",
    "dist",
    "build",
]

[copy]
recursive = true
concurrency = 8
```

### 2. Backup Solution

Preserve everything and create numbered backups:
```toml
[preserve]
mode = "all"

[backup]
mode = "numbered"

[copy]
recursive = true
resume = true

[reflink]
mode = "auto"  # Fast snapshots on supporting filesystems
```

### 3. Production Deployment

Interactive mode with careful preservation:
```toml
[copy]
interactive = true
force = false

[preserve]
mode = "mode,timestamps"

[backup]
mode = "numbered"
```

### 4. Fast Local Copies

Maximum speed with minimal preservation:
```toml
[copy]
concurrency = 16
recursive = true

[preserve]
mode = "none"

[reflink]
mode = "always"  # Fail if reflink not available
```

### 5. Symlink Management

Work with symlink farms:
```toml
[symlink]
mode = "relative"
follow = "never"

[preserve]
mode = "all"
```

## Tips

1. **Start with defaults**: Run `cpx config init` and modify from there
2. **Project-specific settings**: Create `./cpxconfig.toml` in project roots
3. **Check effective config**: Use `cpx config show` to see merged settings
4. **CLI always wins**: Command-line flags override all config files
5. **Test before committing**: Use `--no-config` to test without your settings

## Troubleshooting

**Config not being used?**
- Check location: `cpx config path`
- Verify syntax: Ensure valid TOML
- Check permissions: File must be readable

**Unexpected behavior?**
- View effective config: `cpx config show`
- Disable config: Use `--no-config` flag
- Check priority: Project > User > System

**Need per-project settings?**
- Create `./cpxconfig.toml` in your project directory
- This overrides user and system configs

---

For more examples and usage patterns, see [Examples](examples.md).
