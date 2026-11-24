use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingAlloc;

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(new_size, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[test]
fn test_sort_zero_heap_allocations() {
    let mut seed = 7u64;
    let mut next_u64 = || {
        seed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };

    for n in [100usize, 1_000, 4_096, 100_000] {
        let mut data: Vec<i64> = (0..n).map(|_| next_u64() as i64).collect();
        let before_count = ALLOC_COUNT.load(Ordering::SeqCst);
        let before_bytes = ALLOC_BYTES.load(Ordering::SeqCst);

        fastsort::sort(&mut data);

        let after_count = ALLOC_COUNT.load(Ordering::SeqCst);
        let after_bytes = ALLOC_BYTES.load(Ordering::SeqCst);

        assert_eq!(
            after_count - before_count,
            0,
            "n={n}: expected 0 heap allocs, got {}",
            after_count - before_count
        );
        assert_eq!(
            after_bytes - before_bytes,
            0,
            "n={n}: expected 0 heap bytes, got {}",
            after_bytes - before_bytes
        );
        assert!(data.windows(2).all(|w| w[0] <= w[1]));
    }

    // Verify float sorts also perform 0 heap allocations.
    let mut floats: Vec<f64> = (0..50_000).map(|_| f64::from_bits(next_u64())).collect();
    let before = ALLOC_COUNT.load(Ordering::SeqCst);
    fastsort::sort_f64(&mut floats);
    let after = ALLOC_COUNT.load(Ordering::SeqCst);
    assert_eq!(after - before, 0, "sort_f64 allocated on the heap");
}
