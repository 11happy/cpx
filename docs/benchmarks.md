# CPX Performance Benchmarks


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

| Tool | Mean Time | Speedup |
|------|-----------|---------|
| cpx | **28.72s ± 1.46s** | **2.81× faster** |
| cp | 80.56s ± 2.79s | baseline |

**Time Saved:** 51.84 seconds

---

## Per-Repository Results

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

---



## Why faster ?

**CPX advantages:**
- Parallel I/O operations (16 threads ) [can be more depending on system]
- Linux `copy_file_range()` syscall
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
