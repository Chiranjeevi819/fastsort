# mazesort (Go)

[![Go Reference](https://pkg.go.dev/badge/github.com/Chiranjeevi819/mazesort/go.svg)](https://pkg.go.dev/github.com/Chiranjeevi819/mazesort/go)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**MazeSort** (**In-Place Maze256**) is a pure-Go, zero-allocation, in-place block-buffered MSD radix sort inspired by Ska Sort and IPS²Ra, tailored for Go's stack and escape analysis — alongside a hybrid multi-core parallel variant (`Maze256Parallel`).

- **Single-threaded (`Sort` / `Maze256`):** strictly **`0 B/op`, `0 allocs/op`**, **3x–4.5x faster** than `slices.Sort` on random 32-bit/64-bit integers (up to **16x** on `int8`).
- **Multi-core (`SortParallel` / `Maze256Parallel`):** **5.5x–10x faster** than `slices.Sort` at 1M elements with constant `O(GOMAXPROCS)` (~26 KB) worker metadata and **zero $O(n)$ auxiliary slice allocations**.
- **Generic over all 11 Go integer types** (`int`, `int8`, `int16`, `int32`, `int64`, `uint`, `uint8`, `uint16`, `uint32`, `uint64`, `uintptr`) plus user-defined types with those underlying types.
- **Zero external dependencies**, Go 1.21+.

---

## Installation

```bash
go get github.com/Chiranjeevi819/mazesort/go
```

## Quick Start

```go
package main

import (
	"fmt"
	"github.com/Chiranjeevi819/mazesort/go"
)

func main() {
	nums := []int64{42, -7, 1000, 0, -999, 13}
	mazesort.Sort(nums) // delegates to mazesort.Maze256 (0 B/op, 0 allocs/op)
	fmt.Println(nums)   // [-999 -7 0 13 42 1000]

	type DocID uint32
	ids := []DocID{500, 12, 99999, 1, 42}
	mazesort.Maze256(ids)

	// Multi-core parallel sort for large slices (>= 32,768 elements):
	mazesort.SortParallel(nums) // delegates to mazesort.Maze256Parallel
}
```

---

## Key Design Mechanisms

1. **Tiered Stack-Frame Dispatch (`//go:noinline`):** Isolates small (`<= 256`), medium (`<= 1024`, `<= 4096`), and large (`> 4096`) stack frames so small calls never reserve or zero the 64 KB stack scratch space.
2. **Range-Adaptive Span Compression:** Unsigned two's-complement subtraction (`uint64(v) - uint64(minVal)`) handles signed and unsigned types uniformly without overflow, skips unused leading bits, and runs a direct 1-pass counting sort when `span < 256`.
3. **Adaptive In-Place Routing:** Switches between 64-byte cache-line block-buffered scatter (with a provably bounded spill stack `pendingLen <= 1793 < 2048`) on spread-out data and zero-write cycle-leader routing on skewed/sparse data.
4. **TZCNT Checkpoint Bitmasks:** Tracks occupied rooms in `[4]uint64` and jumps directly to active rooms via `bits.TrailingZeros64`.
5. **Hybrid Parallel Gate (`Maze256Parallel`):** Uses parallel min/max + parallel 256-bucket histogram to route spread-out inputs to parallel work-stealing room sorting and skewed/staircase/few-cluster inputs to median-of-65 branchless BlockQuicksort parallel partitioning into `Maze256` leaves.

---

## Who Should Use It (and When Not To)

- **Great fit:** Telemetry/metrics (`int64` timestamps, latencies, percentiles), search/inverted-index posting lists & doc IDs (`uint32`/`uint64`), GC-sensitive services where $O(n)$ scratch buffers hurt tail latency, and narrow integer slices (`int8`/`int16`).
- **Not a fit:** Structs, strings, floats, custom comparators, stable key-value sorts, or already-sorted slices with 1–2 inversions (use `slices.Sort`).

---

## How Maze256 Differs From Other Algorithms

| Algorithm | Heap Allocs | Complexity | Key Differences |
|---|---|---|---|
| **Go `slices.Sort` (pdqsort)** | `0 B/op` | $O(n \log n)$ | General-purpose comparison sort. Faster on 1–2 swaps, but 3x–10x slower on large random integers and up to 16x slower on `int8`. |
| **Classic LSD Radix Sort** | $O(n)$ (`8 B/el`) | $O(n \cdot d)$ | Allocates a full auxiliary copy on the heap (80 MB for 10M `int64`s). |
| **American Flag Sort (1993)** | `0 B/op` | $O(n \cdot d)$ | Element-by-element swaps everywhere; Maze256 adds span compression, 64-byte block-buffered scatter, and L1 stack ping-pong leaves. |
| **IPS²Ra / Ska Sort (C++)** | `0 B/op` | $O(n \cdot d)$ | Maze256 shares the block-buffered in-place MSD philosophy, replacing IPS²Ra's block-cycle phase with a bounded `pending` spill stack tuned for pure Go. |

---

## Benchmarks

Measured with `go test -run=^$ -bench . -benchmem -benchtime=300ms ./...` on **Intel Core i7-8565U @ 1.80GHz** (4C/8T, `GOMAXPROCS=8`, Go 1.24.2, `windows/amd64`).

### Size Scaling (`Uniform64`, `[]int`)

| `N` | `Maze256Parallel` | `Maze256` (0 B/op) | `slices.Sort` (0 B/op) | Best Speedup |
|---:|---:|---:|---:|---:|
| 64 | 18.11 ns/el *(0 B)* | **17.07 ns/el** | 18.30 ns/el | 1.07x |
| 256 | 28.54 ns/el *(0 B)* | 29.92 ns/el | **26.04 ns/el** | 0.91x |
| 1,000 | 15.73 ns/el *(0 B)* | **14.13 ns/el** | 43.24 ns/el | **3.06x** |
| 10,000 | 25.58 ns/el *(0 B)* | **23.03 ns/el** | 63.64 ns/el | **2.76x** |
| 100,000 | **17.03 ns/el** *(26 KB)* | 28.77 ns/el | 75.06 ns/el | **4.41x** |
| 1,000,000 | **10.86 ns/el** *(26 KB)* | 22.84 ns/el | 81.12 ns/el | **7.47x** |
| 10,000,000 | **30.18 ns/el** *(26 KB)* | 82.74 ns/el | 139.30 ns/el | **4.62x** |

### Distributions (`N = 1,000,000`, `[]int`)

| Dataset | `Maze256Parallel` (~26 KB) | `Maze256` (0 B/op) | `slices.Sort` (0 B/op) | Best Speedup |
|---|---:|---:|---:|---:|
| Uniform 64-bit | **13.83 ns/el** | 25.51 ns/el | 119.20 ns/el | **8.62x** |
| Uniform 32-bit | **12.88 ns/el** | 25.69 ns/el | 129.60 ns/el | **10.06x** |
| Random `[0, 10N)` | **14.95 ns/el** | 34.98 ns/el | 86.21 ns/el | **5.77x** |
| Sorted + 1 Swap | 16.82 ns/el | 15.55 ns/el | **2.77 ns/el** | 0.18x |
| Nearly Sorted | **10.35 ns/el** | 22.38 ns/el | 33.44 ns/el | **3.23x** |
| Reverse Sorted | **1.37 ns/el** | 2.80 ns/el | 2.55 ns/el | **1.86x** |
| Pipe Organ | **15.64 ns/el** | 33.11 ns/el | 70.60 ns/el | **4.51x** |
| Few Unique (16 vals) | **3.10 ns/el** | 3.81 ns/el | 22.26 ns/el | **7.18x** |
| Exp Outliers (99/1) | **14.60 ns/el** | 24.40 ns/el | 45.62 ns/el | **3.12x** |
| Log-Uniform 60-bit | **21.69 ns/el** | 81.17 ns/el | 85.35 ns/el | **3.94x** |
| Staircase (8-bit) | **14.55 ns/el** | 32.43 ns/el | 46.33 ns/el | **3.18x** |
| Pending Stress | **12.70 ns/el** | 21.70 ns/el | 70.29 ns/el | **5.53x** |

### Generic Integer Types (`N = 100,000`, Uniform)

| Type | `Maze256Parallel` | `Maze256` (0 B/op) | `slices.Sort` (0 B/op) | Best Speedup |
|---|---:|---:|---:|---:|
| `uint64` | **18.65 ns/el** | 25.27 ns/el | 107.70 ns/el | **5.77x** |
| `int32` | **14.67 ns/el** | 26.63 ns/el | 82.15 ns/el | **5.60x** |
| `int16` | **12.45 ns/el** | 14.99 ns/el | 77.60 ns/el | **6.23x** |
| `int8` | 3.25 ns/el | **2.29 ns/el** | 37.30 ns/el | **16.29x** |

### Ecosystem Apples-to-Apples (`N = 1,000,000 int64`, `GOMAXPROCS=8`)

Head-to-head comparison against an 8-pass out-of-place LSD Radix Sort (`8 MB` auxiliary slice), `github.com/twotwotwo/sorts` (parallel MSD radix sort), and `github.com/jfcg/sorty/v2` (concurrent quicksort):

#### Single-Threaded (`1T`)

| Dataset | `mazesort.Maze256` (`0 B/op`) | `slices.Sort` (`0 B/op`) | Out-of-Place LSD Radix (`8 MB/op`) |
|---|---:|---:|---:|
| Uniform 64-bit | **22.64 ns/el** | 93.18 ns/el | 25.65 ns/el |
| Uniform 32-bit | **24.74 ns/el** | 93.18 ns/el | 35.32 ns/el |
| Nearly Sorted | **16.15 ns/el** | 20.87 ns/el | 50.46 ns/el |
| Reverse Sorted | **1.70 ns/el** | 2.34 ns/el | 60.10 ns/el |
| Few Unique (16) | **3.10 ns/el** | 20.74 ns/el | 90.14 ns/el |
| Exp Outliers (99/1) | **17.50 ns/el** | 35.80 ns/el | 30.09 ns/el |
| Log-Uniform 60-bit | 61.95 ns/el | 80.67 ns/el | **22.47 ns/el** |

#### Multi-Threaded (`8 GOMAXPROCS`)

| Dataset | `mazesort.Maze256Parallel` (`34 KB/op`) | `twotwotwo/sorts` (`9 KB/op`) | `jfcg/sorty/v2` (`18 KB/op`) |
|---|---:|---:|---:|
| Uniform 64-bit | **19.18 ns/el** | 42.76 ns/el | 49.01 ns/el |
| Uniform 32-bit | **17.13 ns/el** | 47.42 ns/el | 46.98 ns/el |
| Nearly Sorted | 12.23 ns/el | 26.78 ns/el | **11.96 ns/el** |
| Reverse Sorted | **1.25 ns/el** | 34.76 ns/el | 6.77 ns/el |
| Few Unique (16) | **3.27 ns/el** | 15.18 ns/el | 15.20 ns/el |
| Exp Outliers (99/1) | **25.33 ns/el** | 64.04 ns/el | 26.42 ns/el |
| Log-Uniform 60-bit | **28.75 ns/el** | 101.80 ns/el | 36.38 ns/el |

```bash
go test -v ./...
go test -run=^$ -bench . -benchmem -benchtime=300ms ./...
```
