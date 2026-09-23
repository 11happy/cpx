# CPX Performance Benchmarks

## v0.2.0 (September 2026)

Measured with [hyperfine](https://github.com/sharkdp/hyperfine) (`-N`, 1 warmup, 5 runs,
destination removed before each run) on a 24-core Linux machine, source and
destination on tmpfs, warm cache. `xcp` 0.24 and `cpz` (fuc) from crates.io
for comparison. Times are the mean.

| Tree | files | size | cp -r | cpx -j4 | cpx -j16 | xcp -r -w16 | cpz |
|------|-------|------|-------|---------|----------|-------------|-----|
| rust-lang/rust (depth 1) | 63,115 | 465 MB | 415 ms | 227 ms | **141 ms** | 193 ms | 61 ms |
| torvalds/linux (depth 1) | 95,966 | 2.1 GB | 962 ms | 430 ms | **240 ms** | 348 ms | 180 ms |
| synthetic, 200k files of 0-4 KiB | 200,000 | 400 MB | 942 ms | 520 ms | **262 ms** | 380 ms | n/a |

### Why v0.1.4 users should upgrade

v0.1.4 checked every planned file against all previously planned files, so
planning was O(n²) in the number of files:

| files | v0.1.4 | v0.2.0 (-j4) |
|-------|--------|--------------|
| 10,000 | 1.4 s | 47 ms |
| 20,000 | 5.0 s | 90 ms |
| 40,000 | 21.5 s | 158 ms |
| rust tree (63k) | 78 s | 227 ms |
| linux tree (96k) | 154 s | 430 ms |
| 200k files | > 10 min (killed) | 520 ms |

### What changed in v0.2.0

- Planning is linear: duplicate destinations across sources are found with one
  sort instead of a scan per file, and per-entry `stat` plus exclude matching run
  on the directory walker's thread pool (excluded directories are pruned, not
  walked).
- Files are copied largest-first by whichever worker is free (a shared counter),
  instead of rayon's static chunking, which used to hand one worker a contiguous
  run of the largest files while the others idled: linux tree 439 ms -> 243 ms.
- One `copy_file_range` per regular file (the previous loop made a second call
  just to observe EOF).

### Where the remaining gap to cpz is

`cpz` walks and copies in one pass with one task per directory and no progress
accounting. cpx still plans the whole tree first (about 60 ms of the 240 ms on
the linux tree), keeps a byte-accurate progress bar and total, and stats every
entry for its size. Those are the costs of resume, exclude rules and progress.

---

## v0.1.x results

## Methodology

- **Benchmarking Tool:** [hyperfine](https://github.com/sharkdp/hyperfine)
- **Runs per Test:** 6 iterations with (cache cleared before each test)
- **CPX Configuration:** `-r -j=16` (16 parallel threads)
- **GNU cp Configuration:** `-r` (standard recursive)
- **Dataset:** 11 major open-source repositories (773,996 files, 13 GB)
- **Hardware:** Intel Core i7-1165G7 (4C/8T), SSD storage
- **OS:** Ubuntu 25.10 (Kernel 6.x)
- **Consistency:** Results validated across multiple Intel/AMD platforms with <5% variance

---

## Overall Results

### Full Dataset (13 GB, 773,996 files)

cp:
| Tool | Mean Time | Speedup |
|------|-----------|---------|
| cpx | **28.72s ± 1.46s** | **2.81× faster** |
| cp | 80.56s ± 2.79s | baseline |

**Time Saved:** 51.84 seconds

rsync:
| Tool | Mean Time | Speedup |
|------|-----------|---------|
| cpx | **30.37s ± 0.82s** | **2.67× faster** |
| rsync | 80.54s ± 2.567s | baseline |

**Time Saved:** 50.54 seconds

---

## Per-Repository Results

cp:
| Repository | Files | cpx | cp | Speedup |
|------------|-------|----------|-------------|---------|
| **VSCode** | ~15k | 263ms | 1,084ms | **4.12×**  |
| **Rust** | ~65k | 1,091ms | 4,553ms | **4.17×**  |
| **Go** | ~12k | 336ms | 1,373ms | **4.09×**  |
| **TensorFlow** | ~50k | 736ms | 2,846ms | **3.87×**  |
| **Kubernetes** | ~35k | 744ms | 2,805ms | **3.77×**  |
| **Node.js** | ~45k | 1,351ms | 4,573ms | **3.39×**  |
| **Linux Kernel** | ~72k | 2,925ms | 9,314ms | **3.18×**  |
| **Chromium** | ~450k | 15.89s | 46.19s | **2.91×**  |
| **OpenImageIO** | ~5k | 125ms | 258ms | **2.07×**  |
| **Godot** | ~8k | 354ms | 550ms | **1.55×**  |
| **OpenEXR** | ~2.5k | 273ms | 286ms | **1.04×**  |

rsync:
| Repository | Files | cpx | rsync | Speedup |
|------------|-------|----------|-------------|---------|
| **Kubernetes** | ~35k | 617.2ms | 3,097ms | **5.02×**  |
| **Rust** | ~65k | 1,014ms | 4,604ms | **4.54×**  |
| **Go** | ~12k | 340ms | 1,529ms | **4.49×**  |
| **TensorFlow** | ~50k | 736ms | 3,100ms | **4.21×**  |
| **Node.js** | ~45k | 1,198ms | 4,821ms | **4.02×**  |
| **VSCode** | ~15k | 310ms | 1,190ms | **3.83×**  |
| **Linux Kernel** | ~72k | 2,587ms | 8,868ms | **3.43×**  |
| **Chromium** | ~450k | 14.76s | 47.74s | **3.23×**  |
| **OpenImageIO** | ~5k | 120ms | 385ms | **3.21×**  |
| **Godot** | ~8k | 392ms | 987ms | **2.52×**  |
| **OpenEXR** | ~2.5k | 267ms | 588ms | **2.20×**  |

---

See all hyperfine reports(cp,rsync,xcp) here https://github.com/11happy/cpx/tree/main/benchmarks

## Why faster ?

**CPX advantages:**
- Parallel I/O operations (16 threads ) [can be more depending on system]
- Adaptive buffer sizing (64KB-2MB)
- Parallel directory traversal


---

## Reproduction

```bash
# Clone repository
git clone https://github.com/11happy/cpx
cd cpx

# Run cold cache benchmark (requires sudo), adjust the paths in bench.sh accordingly
sudo ./benchmarks/bench.sh cold

# Results saved to ~/cpx_multi_bench/
```

**Note:** Performance varies with storage type (NVMe > SSD > HDD), CPU cores, and filesystem (ext4/btrfs/xfs).
