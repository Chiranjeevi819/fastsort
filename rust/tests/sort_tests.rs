use fastsort::{
    maze256, maze256_f32, maze256_f32_parallel, maze256_f64, maze256_f64_parallel,
    maze256_parallel, sort, sort_f32, sort_f64, sort_parallel, Integer,
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

fn verify_type<T: Integer + core::fmt::Debug>(name: &str, mut gen: impl FnMut(&mut SplitMix64, usize) -> Vec<T>) {
    let mut rng = SplitMix64::new(42);
    let sizes = [0, 1, 2, 64, 65, 256, 257, 1024, 1025, 4096, 4097, 10_000, 50_000];
    for n in sizes {
        let data = gen(&mut rng, n);
        let mut expected = data.clone();
        expected.sort_unstable();

        let mut got = data.clone();
        sort(&mut got);
        assert_eq!(got, expected, "sort/{name} mismatch at n={n}");

        let mut got_par = data;
        sort_parallel(&mut got_par);
        assert_eq!(got_par, expected, "sort_parallel/{name} mismatch at n={n}");
    }
}

#[test]
fn test_sort_all_integer_types() {
    verify_type("i8", |r, n| (0..n).map(|_| r.next_u64() as i8).collect());
    verify_type("i16", |r, n| (0..n).map(|_| r.next_u64() as i16).collect());
    verify_type("i32", |r, n| (0..n).map(|_| r.next_u64() as i32).collect());
    verify_type("i64", |r, n| (0..n).map(|_| r.next_u64() as i64).collect());
    verify_type("isize", |r, n| (0..n).map(|_| r.next_u64() as isize).collect());
    verify_type("u8", |r, n| (0..n).map(|_| r.next_u64() as u8).collect());
    verify_type("u16", |r, n| (0..n).map(|_| r.next_u64() as u16).collect());
    verify_type("u32", |r, n| (0..n).map(|_| r.next_u64() as u32).collect());
    verify_type("u64", |r, n| (0..n).map(|_| r.next_u64()).collect());
    verify_type("usize", |r, n| (0..n).map(|_| r.next_u64() as usize).collect());
}

fn make_pending_stress(n: usize) -> Vec<i64> {
    (0..n)
        .map(|i| {
            let bucket = 255 - (i % 256);
            ((bucket as i64) << 20) | ((i as i64) & 0xF_FFFF)
        })
        .collect()
}

fn make_reverse_buckets(n: usize) -> Vec<i64> {
    let per = (n / 256).max(1);
    (0..n)
        .map(|i| {
            let b = 255i64.saturating_sub((i / per) as i64).max(0);
            (b << 24) | (((n - i) as i64) & 0xFF_FFFF)
        })
        .collect()
}

fn make_staircase8(n: usize, rng: &mut SplitMix64) -> Vec<i64> {
    let mut a: Vec<i64> = (0..n).map(|_| (rng.next_u64() % 1000) as i64).collect();
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
            a[i] = v;
        }
    }
    rng.shuffle(&mut a);
    a
}

#[test]
fn test_sort_edge_cases() {
    let mut rng = SplitMix64::new(108);

    let extreme_vals = [
        i64::MIN,
        i64::MIN + 1,
        i64::MIN + 255,
        -1,
        0,
        1,
        i64::MAX - 255,
        i64::MAX - 1,
        i64::MAX,
    ];

    let cases: Vec<(&str, Vec<i64>)> = vec![
        ("empty", vec![]),
        ("single", vec![42]),
        ("all_min_i64", vec![i64::MIN; 50_000]),
        ("all_max_i64", vec![i64::MAX; 50_000]),
        (
            "extreme_mix",
            (0..50_000)
                .map(|_| extreme_vals[rng.next_usize(extreme_vals.len())])
                .collect(),
        ),
        (
            "span_255_cross0",
            (0..50_000)
                .map(|_| -128 + (rng.next_u64() % 256) as i64)
                .collect(),
        ),
        (
            "span_256_cross0",
            (0..50_000)
                .map(|_| -128 + (rng.next_u64() % 257) as i64)
                .collect(),
        ),
        ("pending_stress_65536", make_pending_stress(65_536)),
        ("reverse_buckets_50000", make_reverse_buckets(50_000)),
        ("staircase_8b", make_staircase8(50_000, &mut rng)),
        ("already_sorted", (0..50_000i64).collect()),
        ("reverse_sorted", (0..50_000i64).rev().collect()),
        (
            "large_8bit_parallel_300k",
            (0..300_000)
                .map(|_| -50 + (rng.next_u64() % 100) as i64)
                .collect(),
        ),
        (
            "few_unique_16_100k",
            (0..100_000).map(|_| (rng.next_u64() % 16) as i64).collect(),
        ),
        (
            "exp_outliers_100k",
            (0..100_000)
                .map(|_| {
                    if rng.next_u64() % 100 < 1 {
                        ((rng.next_u64() % 1_000_000) * 1_000_000) as i64
                    } else {
                        (rng.next_u64() % 1000) as i64
                    }
                })
                .collect(),
        ),
        (
            "log_uniform_60_100k",
            (0..100_000)
                .map(|_| {
                    let b = (rng.next_u64() % 60) as u32;
                    let mask = (1u64 << b) - 1;
                    ((1u64 << b) | (rng.next_u64() & mask)) as i64
                })
                .collect(),
        ),
    ];

    for (name, data) in cases {
        let mut expected = data.clone();
        expected.sort_unstable();

        let mut got = data.clone();
        maze256(&mut got);
        assert_eq!(got, expected, "maze256 mismatch on {name}");

        let mut got_par = data;
        maze256_parallel(&mut got_par);
        assert_eq!(got_par, expected, "maze256_parallel mismatch on {name}");
    }
}

#[test]
fn test_sort_f32_total_order() {
    let mut rng = SplitMix64::new(777);
    let specials: Vec<f32> = vec![
        f32::from_bits(0xffff_ffff), // -qNaN
        f32::from_bits(0xff80_0001), // -sNaN
        f32::NEG_INFINITY,
        f32::MIN,
        -1.5,
        -f32::MIN_POSITIVE,
        f32::from_bits(0x8000_0001), // negative subnormal
        -0.0,
        0.0,
        f32::from_bits(0x0000_0001), // positive subnormal
        f32::MIN_POSITIVE,
        1.5,
        f32::MAX,
        f32::INFINITY,
        f32::from_bits(0x7f80_0001), // +sNaN
        f32::from_bits(0x7fff_ffff), // +qNaN
    ];

    for n in [0, 1, 2, 16, 100, 1000, 5000, 60_000] {
        let mut data = Vec::with_capacity(n);
        for i in 0..n {
            if i < specials.len() {
                data.push(specials[i]);
            } else {
                let bits = rng.next_u64() as u32;
                data.push(f32::from_bits(bits));
            }
        }
        rng.shuffle(&mut data);

        let mut expected = data.clone();
        expected.sort_unstable_by(f32::total_cmp);
        let expected_bits: Vec<u32> = expected.iter().map(|v| v.to_bits()).collect();

        let mut got = data.clone();
        sort_f32(&mut got);
        let got_bits: Vec<u32> = got.iter().map(|v| v.to_bits()).collect();
        assert_eq!(got_bits, expected_bits, "sort_f32 mismatch at n={n}");

        let mut got2 = data.clone();
        maze256_f32(&mut got2);
        let got2_bits: Vec<u32> = got2.iter().map(|v| v.to_bits()).collect();
        assert_eq!(got2_bits, expected_bits, "maze256_f32 mismatch at n={n}");

        let mut got_par = data;
        maze256_f32_parallel(&mut got_par);
        let got_par_bits: Vec<u32> = got_par.iter().map(|v| v.to_bits()).collect();
        assert_eq!(
            got_par_bits, expected_bits,
            "maze256_f32_parallel mismatch at n={n}"
        );
    }
}

#[test]
fn test_sort_f64_total_order() {
    let mut rng = SplitMix64::new(888);
    let specials: Vec<f64> = vec![
        f64::from_bits(0xffff_ffff_ffff_ffff), // -qNaN
        f64::from_bits(0xfff0_0000_0000_0001), // -sNaN
        f64::NEG_INFINITY,
        f64::MIN,
        -1.5,
        -f64::MIN_POSITIVE,
        f64::from_bits(0x8000_0000_0000_0001), // negative subnormal
        -0.0,
        0.0,
        f64::from_bits(0x0000_0000_0000_0001), // positive subnormal
        f64::MIN_POSITIVE,
        1.5,
        f64::MAX,
        f64::INFINITY,
        f64::from_bits(0x7ff0_0000_0000_0001), // +sNaN
        f64::from_bits(0x7fff_ffff_ffff_ffff), // +qNaN
    ];

    for n in [0, 1, 2, 16, 100, 1000, 5000, 60_000] {
        let mut data = Vec::with_capacity(n);
        for i in 0..n {
            if i < specials.len() {
                data.push(specials[i]);
            } else {
                let bits = rng.next_u64();
                data.push(f64::from_bits(bits));
            }
        }
        rng.shuffle(&mut data);

        let mut expected = data.clone();
        expected.sort_unstable_by(f64::total_cmp);
        let expected_bits: Vec<u64> = expected.iter().map(|v| v.to_bits()).collect();

        let mut got = data.clone();
        sort_f64(&mut got);
        let got_bits: Vec<u64> = got.iter().map(|v| v.to_bits()).collect();
        assert_eq!(got_bits, expected_bits, "sort_f64 mismatch at n={n}");

        let mut got2 = data.clone();
        maze256_f64(&mut got2);
        let got2_bits: Vec<u64> = got2.iter().map(|v| v.to_bits()).collect();
        assert_eq!(got2_bits, expected_bits, "maze256_f64 mismatch at n={n}");

        let mut got_par = data;
        maze256_f64_parallel(&mut got_par);
        let got_par_bits: Vec<u64> = got_par.iter().map(|v| v.to_bits()).collect();
        assert_eq!(
            got_par_bits, expected_bits,
            "maze256_f64_parallel mismatch at n={n}"
        );
    }
}
