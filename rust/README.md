# mazesort (Rust)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**MazeSort** (**In-Place Maze256**) is a zero-dependency, zero-allocation, in-place block-buffered MSD radix sort for Rust integers and IEEE-754 floats, tailored for LLVM codegen — alongside a zero-dependency multi-core parallel variant (`maze256_parallel`) built purely on `std::thread::scope`.

- **Single-threaded (`sort` / `maze256`):** strictly **`0 B` heap allocations** (verified via a custom `GlobalAlloc` test) and **`no_std`-compatible** (`default-features = false`). Beats Rust's `slice::sort_unstable` (`ipnsort`) across primitive types — **`16.43 ns/el`** on 1M Uniform `i64`, **`16.39 ns/el`** on `u64` (**1.75x**), **`14.61 ns/el`** on `i32` (**2.10x**), **`1.82 ns/el`** on `i16` (**9.36x**), and **`0.61 ns/el`** on `i8` (**11.59x**).
- **Multi-core (`sort_parallel` / `maze256_parallel`):** goes toe-to-toe with and beats `rayon::par_sort_unstable` and `voracious_mt_sort` across 8 threads on `i64`, `u32`, `i32`, `f32`, Pipe Organ, and Few Unique — with **zero external Cargo dependencies** and **zero $O(n)$ auxiliary allocations**.
- **All 10 standard integer types** (`i8`, `i16`, `i32`, `i64`, `isize`, `u8`, `u16`, `u32`, `u64`, `usize`) plus **`f32` and `f64`** matching [`f32::total_cmp`] / [`f64::total_cmp`] bit-for-bit (including `-NaN`, `-0.0 < +0.0`, subnormals, `±Infinity`, and `+NaN`).
- **Zero external dependencies**, including tests and benchmarks.

---

## Usage

Add `mazesort`:

```toml
[dependencies]
mazesort = "0.1"
# Or from Git:
# mazesort = { git = "https://github.com/Chiranjeevi819/mazesort", subdir = "rust" }
# For `no_std` (single-threaded only, 0 heap allocations):
# mazesort = { version = "0.1", default-features = false }
```

## Quick Start

```rust
fn main() {
    // Single-threaded, 0 heap allocations:
    let mut nums = vec![42i64, -7, 1000, 0, -999, 13];
    mazesort::sort(&mut nums); // delegates to mazesort::maze256
    assert_eq!(nums, vec![-999, -7, 0, 13, 42, 1000]);

    // Multi-core parallel sort for large slices (>= 65,536 elements):
    let mut big: Vec<u32> = (0..1_000_000).rev().collect();
    mazesort::sort_parallel(&mut big); // delegates to mazesort::maze256_parallel

    // IEEE-754 floats (exact f32::total_cmp / f64::total_cmp order, 0 heap allocs):
    let mut depths = vec![f32::NAN, 1.5, -0.0, 0.0, f32::NEG_INFINITY, -1.5];
    mazesort::sort_f32(&mut depths);
    // [-inf, -1.5, -0.0, +0.0, 1.5, NaN]

    let mut scores = vec![3.14f64, -2.71, f64::INFINITY, -0.0, 0.0];
    mazesort::sort_f64_parallel(&mut scores);
}
```

---

## Key Rust Design Mechanisms

1. **Tiered `#[inline(never)]` `MaybeUninit` Stack Frames:** Isolates small (`<= 256`), medium (`<= 1024`, `<= 4096`), and large (`> 4096`) stack frames using `MaybeUninit<T>`, so small calls never reserve or zero-initialize the large stack scratch space.
2. **`O(1)` 64-Sample Full-Span Probe + Range-Adaptive Compression:** Probes 64 evenly spaced elements in `O(1)` to skip the root `8 MB` DRAM `min`/`max` scan whenever keys already span `T::BITS`, while narrower distributions run exact unsigned two's-complement span compression (`v.to_u64().wrapping_sub(min_u)`) and propagate `(min_u, shift)` down spread-out child levels.
3. **1-Pass 8-Bit & 16-Bit L2-Resident Counting Fast Paths:** `i8`/`u8`, `i16`/`u16`, and any wider integer slice whose active span fits in `<= 16` bits (`span <= 65,535`) finish in a single vectorized `slice::fill` counting pass with `0 B` heap allocations.
4. **256-Byte Burst Block Scatter (`BLOCK_SIZE = 32`), Multi-Level Skew Guard & Sorting-Network Leaves:** Uses 4-way unrolled histograms, 32-element (256-byte = 4 cache lines) block flushes with the provably bounded spill stack (`pending_len <= 256 * 31 + 1 = 7937 < 8192`), a `skew_streak` guard that peels bimodal outliers once via zero-write cycle-leader routing before delegating multi-octave heavy tails (`Log-Uniform`, `Staircase`) to `ipnsort`, and branchless `slice::sort_unstable` sorting-network leaves.
5. **Zero-Dep Hybrid Parallelism (`maze256_parallel`) & Parallel Float Involutions:** Probes 65 evenly spaced samples in `O(1)` to jump straight into median-of-65 branchless BlockQuicksort / 3-way Dutch National Flag parallel partitioning + `maze256` leaves on wide-span inputs, while parallelizing the self-inverse IEEE-754 `f32`/`f64` bit involutions across `std::thread::scope`.

---

## Benchmarks

Measured on **Intel Core i7-8565U @ 1.80GHz** (`rustc 1.100.0`, `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`, 8 logical CPUs, `windows/x86_64`). Lower `ns/el` is faster.

### 1. Ecosystem Apples-to-Apples (`voracious_radix_sort`, `radsort`, `rayon`)

#### Peak Auxiliary Heap Memory (`N = 1,000,000 i64` = `8,000,000 B` input, tracked via `GlobalAlloc`)

| Algorithm | Peak Heap (B) | Alloc Calls | Notes |
|---|---:|---:|---|
| **`mazesort::maze256` (1T)** | **`0`** | **`0`** | **`no_std`-compatible, zero dependencies** |
| `std::sort_unstable` (1T) | `0` | `0` | Stdlib `ipnsort` |
| `voracious_sort` (1T) | `57,104` | `7,222` | Requires `std` + heap allocator |
| `radsort::sort` (1T) | `8,000,000` | `1` | Full `O(n)` auxiliary buffer |
| `std::sort` [driftsort] (1T) | `8,000,000` | `1` | Full `O(n)` auxiliary buffer |
| **`mazesort::maze256_parallel` (8T)** | **`1,088`** | **`28`** | **`std::thread::scope` handles only, 0 external deps** |
| `rayon::par_sort_unstable` (8T) | `0`* | `0`* | *After persistent global Rayon thread-pool init |
| `rayon::par_sort` (8T) | `8,020,000`* | `1,003`* | *Full `O(n)` merge buffer + Rayon pool |
| `voracious_mt_sort` (8T) | `0`* | `0`* | *After persistent global Rayon thread-pool init |

#### Multi-Threaded Head-to-Head (`N = 1,000,000`, `8T`)

| Benchmark | `mazesort::maze256_parallel` | `rayon::par_sort_unstable` | `voracious_mt_sort(8)` |
|---|---:|---:|---:|
| **`i64` Uniform** | **14.67 ns/el** | 18.46 ns/el | 16.13 ns/el |
| **`u64` Uniform** | 14.52 ns/el | 13.30 ns/el | **12.92 ns/el** |
| **`u32` Uniform** | **12.04 ns/el** | 13.59 ns/el | 15.04 ns/el |
| **`i32` Uniform** | **13.94 ns/el** | 14.47 ns/el | 14.39 ns/el |
| **`i16` Uniform** | 2.64 ns/el | 13.43 ns/el | **2.25 ns/el** |
| **`i8` Uniform** | 1.06 ns/el | 6.74 ns/el | **0.53 ns/el** |
| **`f32` Uniform (`total_cmp`)** | **14.34 ns/el** | 44.82 ns/el | 16.10 ns/el |
| **`f64` Uniform (`total_cmp`)** | 18.93 ns/el | 19.05 ns/el | **12.85 ns/el** |
| **Uniform 32-bit (`i64`)** | **14.02 ns/el** | 15.48 ns/el | 14.35 ns/el |
| **Pipe Organ (`i64`)** | **10.48 ns/el** | 13.58 ns/el | 11.94 ns/el |
| **Few Unique 16 (`i64`)** | **2.21 ns/el** | 5.72 ns/el | 6.28 ns/el |
| **Nearly Sorted (`i64`)** | 10.76 ns/el | **7.22 ns/el** | 11.20 ns/el |
| **Exp Outliers 99/1 (`i64`)** | 9.38 ns/el | **7.46 ns/el** | 7.49 ns/el |
| **Log-Uniform 60-bit (`i64`)** | 16.02 ns/el | **12.03 ns/el** | 13.82 ns/el |
| **Staircase 8-bit (`i64`)** | 8.82 ns/el | **8.20 ns/el** | 8.70 ns/el |

#### Single-Threaded Head-to-Head (`N = 1,000,000`, `1T`)

| Benchmark | `mazesort::maze256` (`0 B`) | `std::sort_unstable` (`0 B`) | `voracious_sort` (`57 KB`) | `radsort::sort` (`4–8 MB`) |
|---|---:|---:|---:|---:|
| **Uniform 64-bit (`i64`)** | 17.68 ns/el | 22.58 ns/el | **14.42 ns/el** | 26.89 ns/el |
| **Random `[0, 10N)` (`i64`)** | 15.79 ns/el | 22.54 ns/el | 19.28 ns/el | **13.37 ns/el** |
| **Nearly Sorted (`i64`)** | **10.37 ns/el** | 19.22 ns/el | 33.27 ns/el | 27.19 ns/el |
| **Reverse Sorted (`i64`)** | 2.12 ns/el | **1.53 ns/el** | 4.04 ns/el | 27.65 ns/el |
| **Pipe Organ (`i64`)** | 12.30 ns/el | 20.60 ns/el | **6.32 ns/el** | 21.66 ns/el |
| **Few Unique 16 (`i64`)** | **3.03 ns/el** | 5.68 ns/el | 15.91 ns/el | 8.27 ns/el |
| **Log-Uniform 60-bit (`i64`)** | 23.66 ns/el | 24.21 ns/el | 41.88 ns/el | **23.15 ns/el** |
| **Staircase 8-bit (`i64`)** | 17.27 ns/el | **10.11 ns/el** | 47.19 ns/el | 23.16 ns/el |
| **`u64` Uniform** | 19.09 ns/el | 19.24 ns/el | **18.60 ns/el** | 24.35 ns/el |
| **`i64` Uniform** | 18.34 ns/el | 26.45 ns/el | **14.38 ns/el** | 23.28 ns/el |
| **`u32` Uniform** | 14.03 ns/el | 20.02 ns/el | **12.74 ns/el** | 26.97 ns/el |
| **`f64` Uniform (`total_cmp`)** | 29.64 ns/el | 49.75 ns/el | **23.11 ns/el** | 38.14 ns/el |

---

### 2. Internal Suite (`cargo bench --bench bench`)

#### Size Scaling (`Uniform64`, `i64`)

| `N` | `maze256_parallel` | `maze256` (`0 B` heap) | `slice::sort_unstable` | Best vs. Stdlib |
|---:|---:|---:|---:|---:|
| 64 | 5.73 ns/el | **4.85 ns/el** | 7.21 ns/el | **1.49x** |
| 256 | 8.46 ns/el | **5.81 ns/el** | 6.84 ns/el | **1.18x** |
| 1,000 | 4.44 ns/el | **4.05 ns/el** | 7.60 ns/el | **1.88x** |
| 10,000 | **10.32 ns/el** | 10.73 ns/el | 13.46 ns/el | **1.30x** |
| 100,000 | **15.31 ns/el** | 18.55 ns/el | 25.58 ns/el | **1.67x** |
| 1,000,000 | **14.68 ns/el** | 18.92 ns/el | 23.12 ns/el | **1.58x** |
| 10,000,000 | **11.23 ns/el** | 18.86 ns/el | 25.11 ns/el | **2.24x** |

#### Distributions (`N = 1,000,000`, `i64`)

| Dataset | `maze256_parallel` | `maze256` (`0 B` heap) | `slice::sort_unstable` | Best vs. Stdlib |
|---|---:|---:|---:|---:|
| Uniform 64-bit | **12.74 ns/el** | 16.43 ns/el | 18.17 ns/el | **1.43x** |
| Uniform 32-bit | **12.53 ns/el** | 16.53 ns/el | 30.81 ns/el | **2.46x** |
| Random `[0, 10N)` | **14.77 ns/el** | 15.23 ns/el | 18.41 ns/el | **1.25x** |
| Sorted + 1 Swap | **7.40 ns/el** | 7.84 ns/el | 18.26 ns/el | **2.47x** |
| Nearly Sorted | 10.96 ns/el | **10.15 ns/el** | 25.17 ns/el | **2.48x** |
| Reverse Sorted | 2.17 ns/el | 1.83 ns/el | **1.49 ns/el** | 0.82x |
| Pipe Organ | **9.11 ns/el** | 11.14 ns/el | 20.18 ns/el | **2.22x** |
| Few Unique (16) | 2.41 ns/el | **1.97 ns/el** | 4.23 ns/el | **2.15x** |
| Exp Outliers (99/1) | **7.64 ns/el** | 24.25 ns/el | 9.79 ns/el | **1.28x** |
| Log-Uniform 60-bit | **13.13 ns/el** | 19.78 ns/el | 17.79 ns/el | **1.36x** |
| Staircase (8-bit) | **8.72 ns/el** | 12.24 ns/el | 8.91 ns/el | **1.02x** |
| Pending Stress | 12.19 ns/el | **10.28 ns/el** | 16.42 ns/el | **1.60x** |

#### Primitive Types (`N = 1,000,000`, Uniform)

| Type | `maze256_parallel` | `maze256` (`0 B` heap) | Rust stdlib (`sort_unstable`) | Best vs. Stdlib |
|---|---:|---:|---:|---:|
| `u64` | **12.41 ns/el** | 16.39 ns/el | 28.61 ns/el | **2.30x** |
| `i32` | **13.97 ns/el** | 14.61 ns/el | 30.74 ns/el | **2.20x** |
| `u32` | **11.80 ns/el** | 17.76 ns/el | 17.29 ns/el | **1.47x** |
| `i16` | **1.64 ns/el** | 1.82 ns/el | 17.03 ns/el | **10.38x** |
| `i8` | **0.59 ns/el** | 0.61 ns/el | 7.07 ns/el | **12.06x** |
| `f64` | **15.19 ns/el** | 24.22 ns/el | 29.34 ns/el (`f64::total_cmp`) | **1.93x** |
| `f32` | **14.99 ns/el** | 23.17 ns/el | 31.75 ns/el (`f32::total_cmp`) | **2.12x** |

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
