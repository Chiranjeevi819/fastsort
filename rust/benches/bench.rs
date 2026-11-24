use std::hint::black_box;
use std::time::{Duration, Instant};

use fastsort::{
    maze256, maze256_f32, maze256_f32_parallel, maze256_f64, maze256_f64_parallel,
    maze256_parallel,
};

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn next_usize(&mut self, bound: usize) -> usize {
        (self.next_u64() as usize) % bound
    }

    fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.next_usize(i + 1);
            slice.swap(i, j);
        }
    }
}

fn bench_ns_per_elem<T: Copy>(template: &[T], min_time: Duration, mut f: impl FnMut(&mut [T])) -> f64 {
    let n = template.len();
    let mut work = template.to_vec();

    // Warmup
    let warmups = if n >= 1_000_000 { 2 } else { 5 };
    for _ in 0..warmups {
        work.copy_from_slice(template);
        f(black_box(&mut work));
    }

    let mut total_ns = 0u128;
    let mut iters = 0u64;
    let min_ns = min_time.as_nanos();
    let min_iters = if n >= 10_000_000 { 3 } else if n >= 1_000_000 { 8 } else { 20 };

    while iters < min_iters || total_ns < min_ns {
        work.copy_from_slice(template);
        let t0 = Instant::now();
        f(black_box(&mut work));
        total_ns += t0.elapsed().as_nanos();
        iters += 1;
    }

    (total_ns as f64) / ((iters as f64) * (n as f64))
}

fn make_uniform64(n: usize, rng: &mut SplitMix64) -> Vec<i64> {
    (0..n).map(|_| rng.next_u64() as i64).collect()
}

fn make_datasets(n: usize) -> Vec<(&'static str, Vec<i64>)> {
    let mut rng = SplitMix64::new(108);

    let u64_data = make_uniform64(n, &mut rng);
    let u32_data: Vec<i64> = (0..n).map(|_| (rng.next_u64() & 0x7fff_ffff) as i64).collect();
    let u10n: Vec<i64> = (0..n)
        .map(|_| (rng.next_u64() % ((n as u64) * 10)) as i64)
        .collect();

    let mut sorted_1_swap: Vec<i64> = (0..n as i64).collect();
    if n >= 4 {
        sorted_1_swap.swap(n / 2, n / 2 + 1);
    }

    let mut nearly_sorted: Vec<i64> = (0..n as i64).collect();
    if n > 108 {
        for _ in 0..(n / 50) {
            let idx = rng.next_usize(n - 108);
            let j = idx + rng.next_usize(108);
            nearly_sorted.swap(idx, j);
        }
    }

    let reversed: Vec<i64> = (0..n as i64).rev().collect();

    let mut pipe_organ = Vec::with_capacity(n);
    for i in 0..(n / 2) {
        pipe_organ.push(i as i64);
    }
    for i in (n / 2)..n {
        pipe_organ.push((n - i) as i64);
    }

    let few_unique: Vec<i64> = (0..n).map(|_| (rng.next_u64() % 16) as i64).collect();

    let exp_outliers: Vec<i64> = (0..n)
        .map(|_| {
            if rng.next_u64() % 100 < 1 {
                ((rng.next_u64() % 1_000_000) * 1_000_000) as i64
            } else {
                (rng.next_u64() % 1000) as i64
            }
        })
        .collect();

    let log_uniform: Vec<i64> = (0..n)
        .map(|_| {
            let b = (rng.next_u64() % 60) as u32;
            let mask = (1u64 << b) - 1;
            ((1u64 << b) | (rng.next_u64() & mask)) as i64
        })
        .collect();

    let mut staircase: Vec<i64> = (0..n).map(|_| (rng.next_u64() % 1000) as i64).collect();
    let outliers = [
        1i64 << 61,
        1i64 << 53,
        1i64 << 45,
        1i64 << 37,
        1i64 << 29,
        1i64 << 21,
        1i64 << 13,
    ];
    for (i, &v) in outliers.iter().enumerate() {
        if i < n {
            staircase[i] = v;
        }
    }
    rng.shuffle(&mut staircase);

    let pending: Vec<i64> = (0..n)
        .map(|i| {
            let bucket = 255 - (i % 256);
            ((bucket as i64) << 20) | ((i as i64) & 0xF_FFFF)
        })
        .collect();

    vec![
        ("Uniform 64-bit", u64_data),
        ("Uniform 32-bit", u32_data),
        ("Random [0, 10N)", u10n),
        ("Sorted + 1 Swap", sorted_1_swap),
        ("Nearly Sorted", nearly_sorted),
        ("Reverse Sorted", reversed),
        ("Pipe Organ", pipe_organ),
        ("Few Unique (16)", few_unique),
        ("Exp Outliers (99/1)", exp_outliers),
        ("Log-Uniform 60-bit", log_uniform),
        ("Staircase (8-bit)", staircase),
        ("Pending Stress", pending),
    ]
}

fn main() {
    let quick = std::env::args().any(|a| a == "--quick");
    let min_time = if quick {
        Duration::from_millis(80)
    } else {
        Duration::from_millis(300)
    };

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!(
        "Rust fastsort benchmarks ({threads} logical CPUs, ns/elem — lower is faster)\n"
    );

    // 1. Size scaling (Uniform64, i64)
    println!("=== Size Scaling (Uniform64, i64) ===");
    println!(
        "{:<12} | {:>14} | {:>14} | {:>14} | {:>10}",
        "N", "maze256_par", "maze256", "sort_unstable", "Best Speed"
    );
    println!("{}", "-".repeat(76));
    let mut rng = SplitMix64::new(999);
    let sizes: &[usize] = if quick {
        &[64, 256, 1_000, 10_000, 100_000]
    } else {
        &[64, 256, 1_000, 10_000, 100_000, 1_000_000, 10_000_000]
    };
    for &n in sizes {
        let data = make_uniform64(n, &mut rng);
        let ns_par = bench_ns_per_elem(&data, min_time, maze256_parallel);
        let ns_seq = bench_ns_per_elem(&data, min_time, maze256);
        let ns_std = bench_ns_per_elem(&data, min_time, |s| s.sort_unstable());
        let best = ns_par.min(ns_seq);
        println!(
            "{:<12} | {:>11.2} ns | {:>11.2} ns | {:>11.2} ns | {:>9.2}x",
            n,
            ns_par,
            ns_seq,
            ns_std,
            ns_std / best
        );
    }

    if quick {
        return;
    }

    // 2. Distributions (N = 1,000,000, i64)
    println!("\n=== Distributions (N = 1,000,000, i64) ===");
    println!(
        "{:<20} | {:>14} | {:>14} | {:>14} | {:>10}",
        "Dataset", "maze256_par", "maze256", "sort_unstable", "Best Speed"
    );
    println!("{}", "-".repeat(84));
    for (name, data) in make_datasets(1_000_000) {
        let ns_par = bench_ns_per_elem(&data, min_time, maze256_parallel);
        let ns_seq = bench_ns_per_elem(&data, min_time, maze256);
        let ns_std = bench_ns_per_elem(&data, min_time, |s| s.sort_unstable());
        let best = ns_par.min(ns_seq);
        println!(
            "{:<20} | {:>11.2} ns | {:>11.2} ns | {:>11.2} ns | {:>9.2}x",
            name,
            ns_par,
            ns_seq,
            ns_std,
            ns_std / best
        );
    }

    // 3. Primitive Types (N = 1,000,000, Uniform)
    println!("\n=== Primitive Types (N = 1,000,000, Uniform) ===");
    println!(
        "{:<10} | {:>14} | {:>14} | {:>14} | {:>10}",
        "Type", "maze256_par", "maze256", "std unstable", "Best Speed"
    );
    println!("{}", "-".repeat(74));
    let n = 1_000_000usize;
    let mut rng = SplitMix64::new(123);

    macro_rules! bench_int_type {
        ($label:expr, $t:ty) => {{
            let data: Vec<$t> = (0..n).map(|_| rng.next_u64() as $t).collect();
            let ns_par = bench_ns_per_elem(&data, min_time, |s| maze256_parallel(s));
            let ns_seq = bench_ns_per_elem(&data, min_time, |s| maze256(s));
            let ns_std = bench_ns_per_elem(&data, min_time, |s| s.sort_unstable());
            let best = ns_par.min(ns_seq);
            println!(
                "{:<10} | {:>11.2} ns | {:>11.2} ns | {:>11.2} ns | {:>9.2}x",
                $label,
                ns_par,
                ns_seq,
                ns_std,
                ns_std / best
            );
        }};
    }

    bench_int_type!("u64", u64);
    bench_int_type!("i32", i32);
    bench_int_type!("u32", u32);
    bench_int_type!("i16", i16);
    bench_int_type!("i8", i8);

    {
        let data: Vec<f64> = (0..n)
            .map(|_| ((rng.next_u64() as i64) as f64) / (i64::MAX as f64) * 1e6)
            .collect();
        let ns_par = bench_ns_per_elem(&data, min_time, maze256_f64_parallel);
        let ns_seq = bench_ns_per_elem(&data, min_time, maze256_f64);
        let ns_std = bench_ns_per_elem(&data, min_time, |s| s.sort_unstable_by(f64::total_cmp));
        let best = ns_par.min(ns_seq);
        println!(
            "{:<10} | {:>11.2} ns | {:>11.2} ns | {:>11.2} ns | {:>9.2}x",
            "f64",
            ns_par,
            ns_seq,
            ns_std,
            ns_std / best
        );
    }

    {
        let data: Vec<f32> = (0..n)
            .map(|_| ((rng.next_u64() as i32) as f32) / (i32::MAX as f32) * 1e6)
            .collect();
        let ns_par = bench_ns_per_elem(&data, min_time, maze256_f32_parallel);
        let ns_seq = bench_ns_per_elem(&data, min_time, maze256_f32);
        let ns_std = bench_ns_per_elem(&data, min_time, |s| s.sort_unstable_by(f32::total_cmp));
        let best = ns_par.min(ns_seq);
        println!(
            "{:<10} | {:>11.2} ns | {:>11.2} ns | {:>11.2} ns | {:>9.2}x",
            "f32",
            ns_par,
            ns_seq,
            ns_std,
            ns_std / best
        );
    }
}
