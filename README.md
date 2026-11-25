# mazesort

[![Go Reference](https://pkg.go.dev/badge/github.com/Chiranjeevi819/mazesort/go.svg)](https://pkg.go.dev/github.com/Chiranjeevi819/mazesort/go)
[![Go CI](https://github.com/Chiranjeevi819/mazesort/actions/workflows/go.yml/badge.svg)](https://github.com/Chiranjeevi819/mazesort/actions/workflows/go.yml)
[![JS CI](https://github.com/Chiranjeevi819/mazesort/actions/workflows/js.yml/badge.svg)](https://github.com/Chiranjeevi819/mazesort/actions/workflows/js.yml)
[![Rust CI](https://github.com/Chiranjeevi819/mazesort/actions/workflows/rust.yml/badge.svg)](https://github.com/Chiranjeevi819/mazesort/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`mazesort` (**MazeSort**) is a multi-language, zero-dependency, **in-place** sorting library built around **In-Place Maze256** — a range-adaptive, in-place block-buffered MSD radix sort inspired by Ska Sort and IPS²Ra.

---

## Languages

| Language | Package | Directory | Highlights |
|---|---|---|---|
| **Go** | `github.com/Chiranjeevi819/mazesort/go` | [go/](go/) | Pure Go 1.21+, all 11 integer types, `0 B/op` single-threaded (`Maze256`) + multi-core `Maze256Parallel` (**7.5x–10x** vs. `slices.Sort`, up to **16x** on `int8`) |
| **JavaScript / TypeScript** | `mazesort` | [js/](js/) | Zero-dep ESM + `.d.ts`, zero per-call allocs on TypedArrays (`Int8`–`Uint32`, `Float32Array`), **1.7x–10.7x** faster than V8 `TypedArray.prototype.sort()` |
| **Rust** | `mazesort` | [rust/](rust/) | Zero-dep, `no_std`-compatible `0 B` heap single-threaded (`maze256`) + `std::thread::scope` multi-core `maze256_parallel`, all 10 integer types + `f32`/`f64` (`total_cmp`), beats `rayon::par_sort_unstable` & `voracious_mt_sort` on multi-core `i64`/`u32`/`i32`/`f32` (**10.4x** on `i16`, **12.1x** on `i8` at `0.59 ns/el`) |

---

## Algorithm: MazeSort (In-Place Maze256)

**MazeSort** (`Maze256`) is an in-place, range-adaptive 8-bit Most-Significant-Digit (MSD) radix sort inspired by Ska Sort and IPS²Ra, tailored to each host runtime's memory and cache model (Go's stack/escape analysis and bounds-check elimination; V8 TurboFan's monomorphic `Int32Array` loops; Rust/LLVM's `MaybeUninit` tiered stack frames, branchless sorting-network leaves, and barrier-synchronized `std::thread::scope` parallelism).

> For the full deep dive, problem breakdown, bounded spill-stack proof, and pseudocode, see **[ALGORITHM.md](ALGORITHM.md)**.

### How It Works

```text
Input array (len = n)
  │
  ├── Small n ────────────────► Fast comparison / insertion base case
  ├── Monotonic asc / desc ───► O(n) no-op or in-place reverse
  ├── Fits in L1 (<= 4096) ───► Hoisted ping-pong MSD radix sort
  └── Large n (> 4096) ───────► In-Place Maze256:
                                  1. Min/Max scan (or O(1) full-span probe) -> unsigned span
                                  2. Narrow span (<= 8 or 16 bits) -> direct 1-pass counting sort
                                  3. Pass 1: clean 256-bucket histogram
                                  4. Pass 2 (Adaptive In-Place Routing):
                                     ├─ Spread-out: cache-line block-buffered scatter
                                     │    + provably bounded spill stack (P <= 256(K - 1) + 1)
                                     └─ Skewed / sparse: zero-write cycle-leader permutation
                                  5. Recurse into occupied rooms
                                     (switching to L1 ping-pong once room <= 4096)
```

### Headline Benchmarks

Measured on **Intel Core i7-8565U @ 1.80GHz** (4 cores / 8 threads, `windows/amd64`):

| Runtime & Benchmark (`N = 1,000,000`) | `mazesort` | Stdlib / Built-in | Speedup |
|---|---:|---:|---:|
| Go `Maze256` (`[]int`, Uniform 64-bit, `0 B/op`) | **22.84 ns/el** | 81.12 ns/el (`slices.Sort`) | **3.55x** |
| Go `Maze256Parallel` (`[]int`, Uniform 64-bit, ~26 KB) | **10.86 ns/el** | 81.12 ns/el (`slices.Sort`) | **7.47x** |
| Go `Maze256` (`[]int8`, 100K, `0 B/op`) | **2.29 ns/el** | 37.30 ns/el (`slices.Sort`) | **16.29x** |
| Node v24 `maze256Float32` (`Float32Array`, 1M) | **78.21 ns/el** | 219.98 ns/el (`V8 .sort()`) | **2.81x** |
| Node v24 `maze256Uint32` (`Uint32Array`, 1M) | **59.97 ns/el** | 117.73 ns/el (`V8 .sort()`) | **1.96x** |
| Node v24 `maze256` (`Int16Array`, 1M) | **10.68 ns/el** | 114.13 ns/el (`V8 .sort()`) | **10.69x** |
| Node v24 `maze256` (`Int8Array`, 1M) | **5.56 ns/el** | 50.75 ns/el (`V8 .sort()`) | **9.13x** |
| Rust `maze256` (`i64`, Uniform 64-bit, `0 B` heap) | **16.43 ns/el** | 18.17 ns/el (`sort_unstable`) | **1.11x** |
| Rust `maze256` (`u64`, Uniform 64-bit, `0 B` heap) | **16.39 ns/el** | 28.61 ns/el (`sort_unstable`) | **1.75x** |
| Rust `maze256` (`i32`, Uniform 32-bit, `0 B` heap) | **14.61 ns/el** | 30.74 ns/el (`sort_unstable`) | **2.10x** |
| Rust `maze256_parallel` (`u32`, 1M, 8T) | **11.80 ns/el** | 17.29 ns/el (`sort_unstable`) | **1.47x** |
| Rust `maze256_f32_parallel` (`f32`, 1M, `total_cmp`) | **14.99 ns/el** | 31.75 ns/el (`sort_unstable_by`) | **2.12x** |
| Rust `maze256_parallel` (`i16`, 1M, 8T) | **1.64 ns/el** | 17.03 ns/el (`sort_unstable`) | **10.38x** |
| Rust `maze256_parallel` (`i8`, 1M, `0 B` heap) | **0.59 ns/el** | 7.07 ns/el (`sort_unstable`) | **12.06x** |

### Apples-to-Apples Ecosystem Comparisons (`N = 1,000,000`)

Measured head-to-head on the same machine (`Intel Core i7-8565U`, 8 logical CPUs) matching single-threaded vs. single-threaded, multi-threaded vs. multi-threaded, and auxiliary heap allocations:

| Ecosystem & Mode | `mazesort` | Top Ecosystem Competitors | Key Takeaway |
|---|---|---|---|
| **Go (1T, `int64` Uniform)** | **`22.64 ns/el`** (`0 B/op`) | `25.65 ns/el` (8-pass LSD Radix, `8 MB/op`) · `93.18 ns/el` (`slices.Sort`, `0 B/op`) | Beats even an `8 MB` out-of-place LSD radix sort with **`0 B/op`** |
| **Go (8T, `int64` Uniform)** | **`19.18 ns/el`** (`34 KB`) | `42.76 ns/el` (`twotwotwo/sorts`) · `49.01 ns/el` (`jfcg/sorty/v2`) | **2.2x–2.6x faster** than Go's parallel radix & quicksort libraries |
| **Rust (8T, Uniform)** | **`14.67 ns`** (`i64`) · **`12.04 ns`** (`u32`) · **`14.34 ns`** (`f32`) | `rayon::par_sort_unstable`: `18.46` / `13.59` / `44.82 ns` · `voracious_mt_sort`: `16.13` / `15.04` / `16.10 ns` | **#1 fastest** on `i64`, `u32`, `i32`, `f32`, Pipe Organ, and Few Unique with **0 external deps** |
| **Rust (1T, `i64` Heap & Speed)** | **`17.68 ns/el`** (`0 B`, `0` allocs, `no_std`) | `sort_unstable`: `22.58 ns` (`0 B`) · `voracious_sort`: `14.42 ns` (`57 KB`, `7,222` allocs) · `radsort`: `26.89 ns` (`8 MB`) | Fastest strictly `0 B`-heap / `no_std` sort; within ~22% of heap-allocating `voracious_sort` |
| **Node v24 (1T, `Int32Array`)** | **`71.24 ns/el`** (`0 B/call` in-place) | `V8 .sort()`: `131.33 ns` (`0 B`) · JS QuickSort: `155.53 ns` (`0 B`) · Out-of-place LSD: `30.53 ns` (`4 MB/call`) | Fastest `0 B/call` in-place TypedArray sort (**1.8x–10.7x** vs. V8 `.sort()`) |

See full benchmark tables and API documentation in [go/README.md](go/README.md), [js/README.md](js/README.md), and [rust/README.md](rust/README.md).

---

## Repository Structure

```text
.
├── go/                        # Go module (github.com/Chiranjeevi819/mazesort/go)
│   ├── go.mod
│   ├── README.md
│   ├── doc.go
│   ├── sort.go
│   ├── maze256.go
│   ├── maze256_parallel.go
│   ├── sort_test.go
│   └── example_test.go
├── js/                        # npm package (mazesort)
│   ├── package.json
│   ├── README.md
│   ├── src/index.mjs
│   ├── src/index.d.ts
│   ├── test/maze256.test.mjs
│   └── bench/bench.mjs
├── rust/                      # Rust crate (mazesort, zero deps, no_std-ready)
│   ├── Cargo.toml
│   ├── README.md
│   ├── src/lib.rs
│   ├── src/maze256.rs
│   ├── src/parallel.rs
│   ├── src/float.rs
│   ├── tests/sort_tests.rs
│   ├── tests/zero_alloc.rs
│   └── benches/bench.rs
├── .github/workflows/
│   ├── go.yml
│   ├── js.yml
│   └── rust.yml
├── docs/
│   └── maze256-example.jpg    # Step-by-step visual walkthrough diagram
├── LICENSE
├── ALGORITHM.md               # Algorithm deep dive, proof & pseudocode
└── README.md
```

---

## Versioning (Multi-Language Submodules)

Each language subdirectory is versioned and published independently:

- **Go:** tag `go/v0.1.0` → `go get github.com/Chiranjeevi819/mazesort/go@v0.1.0`
- **JS/TS:** tag `js/v0.1.0` → `cd js && npm publish --access public`
- **Rust:** tag `rust/v0.1.0` → `cd rust && cargo publish`

---

## License

MIT — see [LICENSE](LICENSE).
