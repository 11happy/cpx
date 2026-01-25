# CPX vs GNU cp — cold cache benchmark

## Environment
- CPU cores: 8
- OS: Linux 6.17.0-8-generic
- Date: Sun Jan 25 18:05:56 IST 2026
- cp: cp (GNU coreutils) 9.5
- Cache mode: cold

## Dataset
- Size: 13G
- Files: 774044
- Directories: 71671
- Repositories: 11

## OpenImageIO
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/OpenImageIO /home/happy/cpx_multi_bench/dest_cpx` | 119.9 ± 1.6 | 118.7 | 123.0 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/OpenImageIO /home/happy/cpx_multi_bench/dest_cp` | 384.4 ± 6.3 | 374.3 | 392.8 | 3.21 ± 0.07 |

## chromium
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/chromium /home/happy/cpx_multi_bench/dest_cpx` | 14.768 ± 1.357 | 13.991 | 17.481 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/chromium /home/happy/cpx_multi_bench/dest_cp` | 47.748 ± 0.812 | 47.119 | 49.093 | 3.23 ± 0.30 |

## kubernetes
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/kubernetes /home/happy/cpx_multi_bench/dest_cpx` | 617.2 ± 19.8 | 607.0 | 657.1 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/kubernetes /home/happy/cpx_multi_bench/dest_cp` | 3097.1 ± 40.5 | 3039.0 | 3134.4 | 5.02 ± 0.17 |

## node
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/node /home/happy/cpx_multi_bench/dest_cpx` | 1.198 ± 0.017 | 1.174 | 1.224 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/node /home/happy/cpx_multi_bench/dest_cp` | 4.821 ± 0.074 | 4.743 | 4.957 | 4.02 ± 0.08 |

## openexr
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/openexr /home/happy/cpx_multi_bench/dest_cpx` | 267.0 ± 3.0 | 262.9 | 271.5 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/openexr /home/happy/cpx_multi_bench/dest_cp` | 588.3 ± 10.8 | 569.5 | 598.4 | 2.20 ± 0.05 |

## linux
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/linux /home/happy/cpx_multi_bench/dest_cpx` | 2.587 ± 0.098 | 2.428 | 2.684 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/linux /home/happy/cpx_multi_bench/dest_cp` | 8.868 ± 0.200 | 8.696 | 9.249 | 3.43 ± 0.15 |

## vscode
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/vscode /home/happy/cpx_multi_bench/dest_cpx` | 310.8 ± 57.4 | 270.4 | 423.1 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/vscode /home/happy/cpx_multi_bench/dest_cp` | 1189.6 ± 27.3 | 1151.8 | 1228.1 | 3.83 ± 0.71 |

## go
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/go /home/happy/cpx_multi_bench/dest_cpx` | 340.3 ± 10.1 | 331.1 | 359.3 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/go /home/happy/cpx_multi_bench/dest_cp` | 1529.4 ± 18.0 | 1497.8 | 1545.4 | 4.49 ± 0.14 |

## rust
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/rust /home/happy/cpx_multi_bench/dest_cpx` | 1.014 ± 0.073 | 0.930 | 1.090 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/rust /home/happy/cpx_multi_bench/dest_cp` | 4.604 ± 0.021 | 4.568 | 4.628 | 4.54 ± 0.33 |

## godot
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/godot /home/happy/cpx_multi_bench/dest_cpx` | 392.3 ± 9.3 | 378.5 | 402.3 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/godot /home/happy/cpx_multi_bench/dest_cp` | 987.5 ± 39.8 | 950.6 | 1044.1 | 2.52 ± 0.12 |

## tensorflow
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos/tensorflow /home/happy/cpx_multi_bench/dest_cpx` | 736.1 ± 16.5 | 716.7 | 763.5 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos/tensorflow /home/happy/cpx_multi_bench/dest_cp` | 3099.7 ± 39.4 | 3059.6 | 3171.0 | 4.21 ± 0.11 |

## Full Dataset
| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `/home/happy/cpx/cpx -r -j=16 /home/happy/cpx_multi_bench/repos /home/happy/cpx_multi_bench/dest_cpx` | 30.373 ± 0.825 | 29.205 | 31.243 | 1.00 |
| `rsync -r /home/happy/cpx_multi_bench/repos /home/happy/cpx_multi_bench/dest_cp` | 80.541 ± 2.567 | 78.208 | 84.340 | 2.65 ± 0.11 |
