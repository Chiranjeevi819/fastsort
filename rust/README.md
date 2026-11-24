# fastsort (Rust)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**In-Place Maze256** is a zero-dependency, zero-allocation, in-place block-buffered MSD radix sort for Rust integers and IEEE-754 floats, tailored for LLVM codegen — alongside a zero-dependency multi-core parallel variant (`maze256_parallel`) built purely on `std::thread::scope`.

- **Single-threaded (`sort` / `maze256`):** strictly **`0 B` heap allocations** (verified via a custom `GlobalAlloc` test) and **`no_std`-compatible** (`default-features = false`). Beats Rust's `slice::sort_unstable` (`ipnsort`) from `N = 1,000` through `N = 10,000,000` on `i64` — **`20.01 ns/el`** on 1M Uniform 64-bit (**1.17x**), **`6.28 ns/el`** on 1M `i16` (**2.60x**), **`23.50 ns/el`** on 1M `f32` (**1.77x**), and **`0.64 ns/el`** on 1M `i8` (**11.05x**).
- **Multi-core (`sort_parallel` / `maze256_parallel`):** **2.3x–4.6x faster** than `slice::sort_unstable` at 1M elements across all 12 distributions (down to **`8.02 ns/el`** on `i64` Uniform 64-bit, **`7.21 ns/el`** on `u32`, **`4.94 ns/el`** on `i16`, **`9.49 ns/el`** on `f32`, **`10.39 ns/el`** on `f64`, **`0.62 ns/el`** on `i8` for **11.47x**) with zero external dependencies and **zero $O(n)$ auxiliary allocations**.
- **All 10 standard integer types** (`i8`, `i16`, `i32`, `i64`, `isize`, `u8`, `u16`, `u32`, `u64`, `usize`) plus **`f32` and `f64`** matching [`f32::total_cmp`] / [`f64::total_cmp`] bit-for-bit (including `-NaN`, `-0.0 < +0.0`, subnormals, `±Infinity`, and `+NaN`).
- **Zero external dependencies**, including tests and benchmarks.

---

## Usage

Add `fastsort` from this repository (or path dependency):

```toml
[dependencies]
fastsort = { git = "https://github.com/Chiranjeevi819/fastsort", subdir = "rust" }
# For `no_std` (single-threaded only, 0 heap allocations):
# fastsort = { git = "https://github.com/Chiranjeevi819/fastsort", default-features = false }
```

## Quick Start

```rust
fn main() {
    // Single-threaded, 0 heap allocations:
    let mut nums = vec![42i64, -7, 1000, 0, -999, 13];
    fastsort::sort(&mut nums); // delegates to fastsort::maze256
    assert_eq!(nums, vec![-999, -7, 0, 13, 42, 1000]);

    // Multi-core parallel sort for large slices (>= 65,536 elements):
    let mut big: Vec<u32> = (0..1_000_000).rev().collect();
    fastsort::sort_parallel(&mut big); // delegates to fastsort::maze256_parallel

    // IEEE-754 floats (exact f32::total_cmp / f64::total_cmp order, 0 heap allocs):
    let mut depths = vec![f32::NAN, 1.5, -0.0, 0.0, f32::NEG_INFINITY, -1.5];
    fastsort::sort_f32(&mut depths);
    // [-inf, -1.5, -0.0, +0.0, 1.5, NaN]

    let mut scores = vec![3.14f64, -2.71, f64::INFINITY, -0.0, 0.0];
    fastsort::sort_f64_parallel(&mut scores);
}
```

---

## Key Rust Design Mechanisms

1. **Tiered `#[inline(never)]` `MaybeUninit` Stack Frames:** Isolates small (`<= 256`), medium (`<= 1024`, `<= 4096`), and large (`> 4096`) stack frames using `MaybeUninit<T>`, so small calls never reserve or zero-initialize the large stack scratch space.
2. **Range-Adaptive Span Compression + Spread-Out Range Propagation:** Unsigned two's-complement wrapping subtraction (`v.to_u64().wrapping_sub(min_u)`) handles signed and unsigned types identically, skips unused leading bits, finishes `span < 256` in a single vectorized `slice::fill` counting-sort pass, and propagates `(min_u, shift)` into spread-out child levels to skip redundant `min`/`max` scans.
3. **256-Byte Burst Block Scatter (`BLOCK_SIZE = 32`), `[u16; 256]` L1 Tables & Non-Recursive Leaf Finish:** Uses 4-way unrolled histograms, 32-element (256-byte = 4 cache lines) block flushes with the provably bounded spill stack (`pending_len <= 256 * 31 + 1 = 7937 < 8192`), compact `[u16; 256]` L1 ping-pong metadata (512 bytes per table), and an inline non-recursive leaf pass with a single bulk copy back when all Level-2 buckets fit in `<= 128` elements (delegating leaves to Rust's branchless `slice::sort_unstable` sorting networks).
4. **Zero-Dep Hybrid Parallelism (`maze256_parallel`):** Probes 65 evenly spaced samples in $O(1)$ to jump straight into median-of-65 branchless BlockQuicksort / 3-way Dutch National Flag parallel partitioning + `maze256` leaves on unsorted wide-span inputs, while routing monotonic or narrow-span (`span < 256`) inputs through a single `std::thread::scope` + `std::sync::Barrier` parallel scan, reverse, or 8-bit counting fill.
5. **Exact IEEE-754 `total_cmp` Bit Involution (`f32` / `f64`):** Reinterprets `&mut [f32]` / `&mut [f64]` in-place as `&mut [i32]` / `&mut [i64]` and applies the branchless self-inverse involution `u ^ ((u >> 31) & 0x7fffffff)` (or `>> 63` for `f64`), matching Rust's `f32::total_cmp` / `f64::total_cmp` bit-for-bit across all bit patterns with zero auxiliary memory.

---

## Benchmarks

Measured with `cargo bench --bench bench` (`rustc 1.100.0`, `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, Intel Core i7-8565U @ 1.80GHz, 8 logical CPUs, `windows/x86_64`). Lower `ns/el` is faster.

> **Note on speedup ratios vs. Go & JS:** Rust 1.81+ uses `ipnsort` (branchless LLVM-compiled quicksort + sorting networks) for `slice::sort_unstable`, which runs at `~20–25 ns/el` on 1M `i64`s — ~4x–5x faster than Go's `slices.Sort` (`~81–105 ns/el`) and V8's `TypedArray.sort()` (`~110–220 ns/el`). In absolute `ns/el`, Rust `fastsort` is the fastest implementation in this repository (`8.02 ns/el` parallel on 1M `i64`, `0.62 ns/el` on 1M `i8`).

### Size Scaling (`Uniform64`, `i64`)

| `N` | `maze256_parallel` | `maze256` (`0 B` heap) | `slice::sort_unstable` | Best Speedup |
|---:|---:|---:|---:|---:|
| 64 | 7.88 ns/el | 7.39 ns/el | **6.81 ns/el** | 0.92x |
| 256 | 8.54 ns/el | 7.16 ns/el | **6.77 ns/el** | 0.95x |
| 1,000 | **4.56 ns/el** | 4.67 ns/el | 8.14 ns/el | **1.79x** |
| 10,000 | **10.32 ns/el** | 10.73 ns/el | 13.46 ns/el | **1.30x** |
| 100,000 | **15.31 ns/el** | 18.55 ns/el | 25.58 ns/el | **1.67x** |
| 1,000,000 | **10.44 ns/el** | 21.98 ns/el | 25.05 ns/el | **2.40x** |
| 10,000,000 | **12.58 ns/el** | 23.84 ns/el | 25.55 ns/el | **2.03x** |

### Distributions (`N = 1,000,000`, `i64`)

| Dataset | `maze256_parallel` | `maze256` (`0 B` heap) | `slice::sort_unstable` | Best Speedup |
|---|---:|---:|---:|---:|
| Uniform 64-bit | **8.02 ns/el** | 20.01 ns/el | 23.48 ns/el | **2.93x** |
| Uniform 32-bit | **9.08 ns/el** | 22.42 ns/el | 23.62 ns/el | **2.60x** |
| Random `[0, 10N)` | **8.74 ns/el** | 24.55 ns/el | 26.75 ns/el | **3.06x** |
| Sorted + 1 Swap | **4.51 ns/el** | 8.88 ns/el | 18.52 ns/el | **4.11x** |
| Nearly Sorted | **5.80 ns/el** | 10.07 ns/el | 20.12 ns/el | **3.47x** |
| Reverse Sorted | **1.21 ns/el** | 1.95 ns/el | 1.27 ns/el | **1.04x** |
| Pipe Organ | **5.22 ns/el** | 13.00 ns/el | 23.78 ns/el | **4.56x** |
| Few Unique (16) | **1.43 ns/el** | 2.43 ns/el | 4.59 ns/el | **3.21x** |
| Exp Outliers (99/1) | **4.63 ns/el** | 12.94 ns/el | 14.60 ns/el | **3.15x** |
| Log-Uniform 60-bit | **9.04 ns/el** | 34.35 ns/el | 19.77 ns/el | **2.19x** |
| Staircase (8-bit) | **7.09 ns/el** | 27.51 ns/el | 11.37 ns/el | **1.60x** |
| Pending Stress | **6.88 ns/el** | 12.81 ns/el | 22.78 ns/el | **3.31x** |

### Primitive Types (`N = 1,000,000`, Uniform)

| Type | `maze256_parallel` | `maze256` (`0 B` heap) | Rust stdlib (`sort_unstable`) | Best Speedup |
|---|---:|---:|---:|---:|
| `u64` | **7.85 ns/el** | 21.05 ns/el | 18.56 ns/el | **2.36x** |
| `i32` | **7.70 ns/el** | 20.20 ns/el | 17.44 ns/el | **2.26x** |
| `u32` | **7.21 ns/el** | 20.95 ns/el | 18.28 ns/el | **2.54x** |
| `i16` | **4.94 ns/el** | 6.28 ns/el | 16.34 ns/el | **3.31x** |
| `i8` | **0.62 ns/el** | 0.64 ns/el | 7.07 ns/el | **11.47x** |
| `f64` | **10.39 ns/el** | 29.06 ns/el | 35.18 ns/el (`f64::total_cmp`) | **3.39x** |
| `f32` | **9.49 ns/el** | 23.50 ns/el | 41.62 ns/el (`f32::total_cmp`) | **4.39x** |

---

## Development

```bash
cargo test --release
cargo check --no-default-features
cargo clippy --all-targets -- -D warnings
cargo bench --bench bench
```

## License

MIT — see [LICENSE](LICENSE).
