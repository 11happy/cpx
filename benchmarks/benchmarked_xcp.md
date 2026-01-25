# CPX vs xcp — cold cache benchmark

## Environment
- CPU cores: 8
- OS: Linux 6.17.0-8-generic
- Date: Sun Jan 25 21:23:15 IST 2026
- cp: cp (GNU coreutils) 9.5
- Cache mode: cold

## Dataset
- Size: 13G
- Files: 774050
- Directories: 71671
- Repositories: 11

## OpenImageIO
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/OpenImageIO /home/happy/cpx_multi_bench/dest_cpx` | 122.7 ± 5.1 | 119.0 | 132.9 | 1.04 ± 0.04 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/OpenImageIO /home/happy/cpx_multi_bench/dest_cp` | 118.5 ± 0.8 | 117.1 | 119.3 | 1.00 |

## chromium
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/chromium /home/happy/cpx_multi_bench/dest_cpx` | 15.280 ± 0.789 | 14.160 | 16.156 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/chromium /home/happy/cpx_multi_bench/dest_cp` | 21.745 ± 0.962 | 20.462 | 23.034 | 1.42 ± 0.10 |

## kubernetes
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/kubernetes /home/happy/cpx_multi_bench/dest_cpx` | 656.0 ± 49.4 | 608.4 | 743.1 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/kubernetes /home/happy/cpx_multi_bench/dest_cp` | 1063.0 ± 20.4 | 1047.5 | 1101.9 | 1.62 ± 0.13 |

## node
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/node /home/happy/cpx_multi_bench/dest_cpx` | 1.313 ± 0.028 | 1.275 | 1.342 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/node /home/happy/cpx_multi_bench/dest_cp` | 1.982 ± 0.107 | 1.896 | 2.161 | 1.51 ± 0.09 |

## openexr
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/openexr /home/happy/cpx_multi_bench/dest_cpx` | 281.2 ± 7.1 | 274.0 | 292.4 | 1.14 ± 0.03 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/openexr /home/happy/cpx_multi_bench/dest_cp` | 246.4 ± 0.7 | 245.4 | 247.4 | 1.00 |

## linux
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/linux /home/happy/cpx_multi_bench/dest_cpx` | 2.943 ± 0.054 | 2.879 | 3.043 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/linux /home/happy/cpx_multi_bench/dest_cp` | 4.188 ± 0.052 | 4.126 | 4.277 | 1.42 ± 0.03 |

## vscode
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/vscode /home/happy/cpx_multi_bench/dest_cpx` | 282.1 ± 7.2 | 274.1 | 292.8 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/vscode /home/happy/cpx_multi_bench/dest_cp` | 442.0 ± 8.1 | 435.2 | 456.8 | 1.57 ± 0.05 |

## go
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/go /home/happy/cpx_multi_bench/dest_cpx` | 350.4 ± 4.9 | 345.4 | 359.6 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/go /home/happy/cpx_multi_bench/dest_cp` | 566.7 ± 3.6 | 562.1 | 571.1 | 1.62 ± 0.02 |

## rust
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/rust /home/happy/cpx_multi_bench/dest_cpx` | 1.128 ± 0.101 | 1.061 | 1.330 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/rust /home/happy/cpx_multi_bench/dest_cp` | 2.019 ± 0.079 | 1.936 | 2.118 | 1.79 ± 0.17 |

## godot
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/godot /home/happy/cpx_multi_bench/dest_cpx` | 362.3 ± 4.6 | 356.9 | 369.8 | 1.24 ± 0.03 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/godot /home/happy/cpx_multi_bench/dest_cp` | 291.5 ± 4.8 | 285.3 | 298.5 | 1.00 |

## tensorflow
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/tensorflow /home/happy/cpx_multi_bench/dest_cpx` | 777.8 ± 9.7 | 766.1 | 793.4 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos/tensorflow /home/happy/cpx_multi_bench/dest_cp` | 1200.1 ± 17.1 | 1174.5 | 1220.1 | 1.54 ± 0.03 |

## Full Dataset
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos /home/happy/cpx_multi_bench/dest_cpx` | 29.279 ± 1.646 | 27.425 | 31.437 | 1.00 |
| `/home/happy/.cargo/bin/xcp -r /home/happy/cpx_multi_bench/repos /home/happy/cpx_multi_bench/dest_cp` | 38.073 ± 1.748 | 35.820 | 39.610 | 1.30 ± 0.09 |
