# fastsort

[![Go Reference](https://pkg.go.dev/badge/github.com/Chiranjeevi819/fastsort/go.svg)](https://pkg.go.dev/github.com/Chiranjeevi819/fastsort/go)
[![Go CI](https://github.com/Chiranjeevi819/fastsort/actions/workflows/go.yml/badge.svg)](https://github.com/Chiranjeevi819/fastsort/actions/workflows/go.yml)
[![JS CI](https://github.com/Chiranjeevi819/fastsort/actions/workflows/js.yml/badge.svg)](https://github.com/Chiranjeevi819/fastsort/actions/workflows/js.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`fastsort` is a multi-language, zero-dependency, **in-place** sorting library built around **In-Place Maze256** — a range-adaptive, in-place block-buffered MSD radix sort inspired by Ska Sort and IPS²Ra.

---

## Languages

| Language | Package | Directory | Highlights |
|---|---|---|---|
| **Go** | `github.com/Chiranjeevi819/fastsort/go` | [go/](go/) | Pure Go 1.21+, all 11 integer types, `0 B/op` single-threaded (`Maze256`) + multi-core `Maze256Parallel` (**7.5x–10x** vs. `slices.Sort`, up to **16x** on `int8`) |
| **JavaScript / TypeScript** | `@chiranjeevi819/fastsort` | [js/](js/) | Zero-dep ESM + `.d.ts`, zero per-call allocs on TypedArrays (`Int8`–`Uint32`, `Float32Array`), **1.7x–10.7x** faster than V8 `TypedArray.prototype.sort()` |

---

## Algorithm: In-Place Maze256

**In-Place Maze256** is an in-place, range-adaptive 8-bit Most-Significant-Digit (MSD) radix sort inspired by Ska Sort and IPS²Ra, tailored to each host runtime's memory and cache model (Go's stack/escape analysis and bounds-check elimination; V8 TurboFan's monomorphic `Int32Array` loops).

### How It Works

```text
Input array (len = n)
  │
  ├── Small n ────────────────► Fast comparison / insertion base case
  ├── Monotonic asc / desc ───► O(n) no-op or in-place reverse
  ├── Fits in L1 (<= 4096) ───► Hoisted ping-pong MSD radix sort
  └── Large n (> 4096) ───────► In-Place Maze256:
                                  1. Min/Max scan -> unsigned two's-complement span
                                  2. If span fits in 8 bits -> direct in-place counting sort
                                  3. Pass 1: clean 256-bucket histogram
                                  4. Pass 2 (Adaptive In-Place Routing):
                                     ├─ Spread-out (>= 32 active rooms, max < 75% n):
                                     │    64-byte cache-line block-buffered scatter
                                     │    + provably bounded spill stack (pending <= 1793 < 2048)
                                     └─ Skewed / sparse:
                                          zero-write cycle-leader permutation
                                  5. Recurse into occupied rooms
                                     (switching to L1 ping-pong once room <= 4096)
```

### Headline Benchmarks

Measured on **Intel Core i7-8565U @ 1.80GHz** (4 cores / 8 threads, `windows/amd64`):

| Runtime & Benchmark (`N = 1,000,000`) | `fastsort` | Stdlib / Built-in | Speedup |
|---|---:|---:|---:|
| Go `Maze256` (`[]int`, Uniform 64-bit, `0 B/op`) | **22.84 ns/el** | 81.12 ns/el (`slices.Sort`) | **3.55x** |
| Go `Maze256Parallel` (`[]int`, Uniform 64-bit, ~26 KB) | **10.86 ns/el** | 81.12 ns/el (`slices.Sort`) | **7.47x** |
| Go `Maze256` (`[]int8`, 100K, `0 B/op`) | **2.29 ns/el** | 37.30 ns/el (`slices.Sort`) | **16.29x** |
| Node v24 `maze256Float32` (`Float32Array`, 1M) | **78.21 ns/el** | 219.98 ns/el (`V8 .sort()`) | **2.81x** |
| Node v24 `maze256Uint32` (`Uint32Array`, 1M) | **59.97 ns/el** | 117.73 ns/el (`V8 .sort()`) | **1.96x** |
| Node v24 `maze256` (`Int16Array`, 1M) | **10.68 ns/el** | 114.13 ns/el (`V8 .sort()`) | **10.69x** |
| Node v24 `maze256` (`Int8Array`, 1M) | **5.56 ns/el** | 50.75 ns/el (`V8 .sort()`) | **9.13x** |

See full benchmark tables and API documentation in [go/README.md](go/README.md) and [js/README.md](js/README.md).

---

## Repository Structure

```text
.
├── go/                        # Go module (github.com/Chiranjeevi819/fastsort/go)
│   ├── go.mod
│   ├── README.md
│   ├── doc.go
│   ├── sort.go
│   ├── maze256.go
│   ├── maze256_parallel.go
│   ├── sort_test.go
│   └── example_test.go
├── js/                        # npm package (@chiranjeevi819/fastsort)
│   ├── package.json
│   ├── README.md
│   ├── src/index.mjs
│   ├── src/index.d.ts
│   ├── test/maze256.test.mjs
│   └── bench/bench.mjs
├── .github/workflows/
│   ├── go.yml
│   └── js.yml
├── LICENSE
└── README.md
```

---

## Versioning (Multi-Language Submodules)

Each language subdirectory is versioned and published independently:

- **Go:** tag `go/v0.1.0` → `go get github.com/Chiranjeevi819/fastsort/go@v0.1.0`
- **JS/TS:** tag `js/v0.1.0` → `cd js && npm publish --access public`

---

## License

MIT — see [LICENSE](LICENSE).
