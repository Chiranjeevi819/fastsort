# fastsort (Rust)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**In-Place Maze256** is a zero-dependency, zero-allocation, in-place block-buffered MSD radix sort for Rust integers and IEEE-754 floats, tailored for LLVM codegen — alongside a zero-dependency multi-core parallel variant (`maze256_parallel`) built purely on `std::thread::scope`.

- **Single-threaded (`sort` / `maze256`):** strictly **`0 B` heap allocations** (verified via a custom `GlobalAlloc` test) and **`no_std`-compatible** (`default-features = false`). Faster than Rust's `slice::sort_unstable` (`ipnsort`) on `i8` (**11.49x** at **`0.64 ns/el`**), `i16` (**3.01x**), `f32`/`f64` (**1.08x–1.37x** vs. `total_cmp`), `Few Unique` (**1.78x**), `Pipe Organ` (**1.51x**), `Nearly Sorted` (**1.79x**), and `Pending Stress` (**1.26x**).
- **Multi-core (`sort_parallel` / `maze256_parallel`):** **1.8x–3.7x faster** than `slice::sort_unstable` at 1M elements (down to **`9.91 ns/el`** on `i64` Uniform 64-bit, **`8.67 ns/el`** on `i32`, **`6.45 ns/el`** on `i16`, **`12.47 ns/el`** on `f32`, **`18.37 ns/el`** on `f64`) with zero external dependencies and **zero $O(n)$ auxiliary allocations**.
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

1. **Tiered `#[inline(never)]` `MaybeUninit` Stack Frames:** Isolates small (`<= 256`), medium (`<= 1024`, `<= 4096`), and large (`> 4096`) stack frames using `MaybeUninit<T>`, so small calls never reserve or zero-initialize the 64 KB stack scratch space.
2. **Range-Adaptive Span Compression + Spread-Out Range Propagation:** Unsigned two's-complement wrapping subtraction (`v.to_u64().wrapping_sub(min_u)`) handles signed and unsigned types identically, skips unused leading bits, finishes `span < 256` in a single vectorized `slice::fill` counting-sort pass, and propagates `(min_u, shift)` into spread-out child levels to skip redundant `min`/`max` scans.
3. **4-Way Unrolled Histograms, `[u16; 256]` L1 Tables & Block Scatter:** Breaks store-to-load forwarding stalls on large histograms, shrinks L1 ping-pong metadata from 6 KB to 1.5 KB via `[u16; 256]`, drains the provably bounded spill stack (`pending_len <= 1793 < 2048`) outside the sequential read hot path, falls back to zero-write cycle-leader routing on skewed/sparse inputs, and uses Rust's branchless `slice::sort_unstable` sorting networks for leaves `<= 128`.
4. **Zero-Dep Hybrid Parallelism (`maze256_parallel`):** Runs a barrier-synchronized parallel min/max + monotonicity scan inside a single `std::thread::scope` (handling constant, ascending, descending parallel reverse, and parallel 8-bit counting sort directly), then uses median-of-65 branchless BlockQuicksort / 3-way Dutch National Flag parallel partitioning into balanced chunks sorted concurrently across all cores by `maze256`.
5. **Exact IEEE-754 `total_cmp` Bit Involution (`f32` / `f64`):** Reinterprets `&mut [f32]` / `&mut [f64]` in-place as `&mut [i32]` / `&mut [i64]` and applies the branchless self-inverse involution `u ^ ((u >> 31) & 0x7fffffff)` (or `>> 63` for `f64`), matching Rust's `f32::total_cmp` / `f64::total_cmp` bit-for-bit across all bit patterns with zero auxiliary memory.

---

## Benchmarks

Measured with `cargo bench --bench bench` (`rustc 1.100.0`, `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, 8 logical CPUs, `windows/x86_64`). Lower `ns/el` is faster.

### Size Scaling (`Uniform64`, `i64`)

| `N` | `maze256_parallel` | `maze256` (`0 B` heap) | `slice::sort_unstable` | Best Speedup |
|---:|---:|---:|---:|---:|
| 64 | 5.51 ns/el | **5.15 ns/el** | 5.82 ns/el | **1.13x** |
| 256 | 11.39 ns/el | 8.17 ns/el | **7.71 ns/el** | 0.94x |
| 1,000 | 7.44 ns/el | **6.59 ns/el** | 7.29 ns/el | **1.11x** |
| 10,000 | 12.44 ns/el | 11.66 ns/el | **10.97 ns/el** | 0.94x |
| 100,000 | 19.74 ns/el | 20.43 ns/el | **15.96 ns/el** | 0.81x |
| 1,000,000 | **10.58 ns/el** | 28.22 ns/el | 19.39 ns/el | **1.83x** |
| 10,000,000 | **16.52 ns/el** | 34.19 ns/el | 24.87 ns/el | **1.51x** |

### Distributions (`N = 1,000,000`, `i64`)

| Dataset | `maze256_parallel` | `maze256` (`0 B` heap) | `slice::sort_unstable` | Best Speedup |
|---|---:|---:|---:|---:|
| Uniform 64-bit | **9.91 ns/el** | 25.77 ns/el | 20.64 ns/el | **2.08x** |
| Uniform 32-bit | **11.66 ns/el** | 25.68 ns/el | 20.64 ns/el | **1.77x** |
| Random `[0, 10N)` | **10.77 ns/el** | 21.90 ns/el | 21.04 ns/el | **1.95x** |
| Sorted + 1 Swap | **4.84 ns/el** | 8.97 ns/el | 15.72 ns/el | **3.25x** |
| Nearly Sorted | **7.15 ns/el** | 9.84 ns/el | 17.59 ns/el | **2.46x** |
| Reverse Sorted | 1.90 ns/el | 2.11 ns/el | **1.21 ns/el** | 0.64x |
| Pipe Organ | **9.18 ns/el** | 14.51 ns/el | 21.98 ns/el | **2.39x** |
| Few Unique (16) | **2.68 ns/el** | 2.87 ns/el | 5.11 ns/el | **1.91x** |
| Exp Outliers (99/1) | **5.90 ns/el** | 16.92 ns/el | 8.83 ns/el | **1.50x** |
| Log-Uniform 60-bit | **13.71 ns/el** | 32.19 ns/el | 16.80 ns/el | **1.23x** |
| Staircase (8-bit) | 8.87 ns/el | 26.15 ns/el | **8.09 ns/el** | 0.91x |
| Pending Stress | **9.78 ns/el** | 15.69 ns/el | 19.78 ns/el | **2.02x** |

### Primitive Types (`N = 1,000,000`, Uniform)

| Type | `maze256_parallel` | `maze256` (`0 B` heap) | Rust stdlib (`sort_unstable`) | Best Speedup |
|---|---:|---:|---:|---:|
| `u64` | **14.18 ns/el** | 24.62 ns/el | 22.53 ns/el | **1.59x** |
| `i32` | **8.67 ns/el** | 20.96 ns/el | 18.26 ns/el | **2.11x** |
| `u32` | **8.74 ns/el** | 21.50 ns/el | 17.55 ns/el | **2.01x** |
| `i16` | **6.45 ns/el** | 7.98 ns/el | 24.02 ns/el | **3.73x** |
| `i8` | 0.70 ns/el | **0.64 ns/el** | 7.35 ns/el | **11.49x** |
| `f64` | **18.37 ns/el** | 43.79 ns/el | 47.34 ns/el (`f64::total_cmp`) | **2.58x** |
| `f32` | **12.47 ns/el** | 31.73 ns/el | 43.50 ns/el (`f32::total_cmp`) | **3.49x** |

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
