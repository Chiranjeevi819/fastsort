use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Barrier, OnceLock};
use std::thread;

use crate::maze256::maze256;
use crate::Integer;

const PARALLEL_CUTOFF: usize = 65_536;
const PARALLEL_MAX_PROCS: usize = 32;
const PARALLEL_LEAF_LIMIT: usize = 16_384;

const ACTION1_DONE: u8 = 0;
const ACTION1_REVERSE: u8 = 1;
const ACTION1_COUNTING_8BIT: u8 = 2;
const ACTION1_PARALLEL_SPLIT: u8 = 3;

#[derive(Clone, Copy)]
struct ChunkStat<T: Integer> {
    min_v: T,
    max_v: T,
    asc: bool,
    desc: bool,
}

/// Wrapper for sharing a raw mutable pointer across scoped worker threads that
/// operate on strictly disjoint sub-ranges synchronized by barriers.
#[derive(Clone, Copy)]
struct SendMutPtr<T>(*mut T);

// SAFETY: Accesses through `SendMutPtr` are strictly disjoint across threads
// and synchronized via `Barrier` happens-before edges.
unsafe impl<T: Send> Send for SendMutPtr<T> {}
unsafe impl<T: Send> Sync for SendMutPtr<T> {}

/// Fixed-size shared container synchronized strictly by barrier phases.
struct BarrierCell<T>(UnsafeCell<T>);

// SAFETY: Every write completes before a `Barrier::wait()` and readers only
// access after the matching `Barrier::wait()`.
unsafe impl<T: Send> Sync for BarrierCell<T> {}

impl<T> BarrierCell<T> {
    #[inline(always)]
    fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }

    #[inline(always)]
    fn get(&self) -> *mut T {
        self.0.get()
    }
}

struct CountingMeta {
    offsets: [usize; 256],
    ends: [usize; 256],
    active_list: [u8; 256],
}

fn available_procs() -> usize {
    static PROCS: OnceLock<usize> = OnceLock::new();
    *PROCS.get_or_init(|| {
        thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .clamp(1, PARALLEL_MAX_PROCS)
    })
}

/// Sorts `data` in ascending order in-place across multiple CPU cores using a
/// hybrid parallel **In-Place Maze256** algorithm with zero external
/// dependencies and zero $O(n)$ auxiliary allocations.
///
/// - For small slices (`< 65,536` elements, single-core machines, or 8-bit
///   types `< 2M` elements), delegates directly to [`maze256`] with 0 heap
///   allocations and 0 threads.
/// - For larger slices, runs a barrier-synchronized parallel min/max +
///   monotonicity scan inside a single [`std::thread::scope`]:
///   - Constant or ascending: returns immediately.
///   - Descending: parallel in-place reverse.
///   - Narrow span (`span < 256`): parallel 256-bucket histogram + lock-free
///     parallel counting-sort fill.
///   - General keys (`span >= 256`): median-of-65 branchless BlockQuicksort /
///     3-way Dutch National Flag parallel partitioning into balanced chunks
///     sorted concurrently across all CPU cores by [`maze256`].
pub fn maze256_parallel<T: Integer>(data: &mut [T]) {
    let n = data.len();
    let procs = available_procs();
    if procs <= 1 || n < PARALLEL_CUTOFF || (T::BITS == 8 && n < 2_000_000) {
        maze256(data);
        return;
    }

    let data_ptr = SendMutPtr(data.as_mut_ptr());
    let barrier = Barrier::new(procs);

    let stats: [BarrierCell<ChunkStat<T>>; PARALLEL_MAX_PROCS] = core::array::from_fn(|_| {
        BarrierCell::new(ChunkStat {
            min_v: T::ZERO,
            max_v: T::ZERO,
            asc: true,
            desc: true,
        })
    });
    let local_counts: [BarrierCell<[usize; 256]>; PARALLEL_MAX_PROCS] =
        core::array::from_fn(|_| BarrierCell::new([0usize; 256]));
    let counting_meta = BarrierCell::new(CountingMeta {
        offsets: [0usize; 256],
        ends: [0usize; 256],
        active_list: [0u8; 256],
    });

    let action1 = AtomicU8::new(ACTION1_DONE);
    let min_u_shared = AtomicU64::new(0);
    let active_rooms_shared = AtomicUsize::new(0);
    let next_idx = AtomicUsize::new(0);

    let worker = |p: usize| {
        let ptr = data_ptr;
        let lo = (n * p) / procs;
        let hi = (n * (p + 1)) / procs;

        // --- Phase 1: Parallel min/max + monotonicity scan ---
        {
            // SAFETY: `[lo..hi)` is strictly disjoint across `p in 0..procs`
            // and no thread mutates `data` during Phase 1.
            let slice = unsafe { core::slice::from_raw_parts(ptr.0.add(lo), hi - lo) };
            let mut mn = slice[0];
            let mut mx = mn;
            let mut asc = true;
            let mut desc = true;
            let mut prev = mn;
            for &v in &slice[1..] {
                if v < mn {
                    mn = v;
                } else if v > mx {
                    mx = v;
                }
                if v < prev {
                    asc = false;
                } else if v > prev {
                    desc = false;
                }
                prev = v;
            }
            // SAFETY: Only worker `p` writes `stats[p]` before [B1].
            unsafe {
                *stats[p].get() = ChunkStat {
                    min_v: mn,
                    max_v: mx,
                    asc,
                    desc,
                };
            }
        }

        barrier.wait(); // [B1]

        if p == 0 {
            // SAFETY: All workers finished writing `stats[0..procs]` before [B1].
            let s0 = unsafe { *stats[0].get() };
            let mut min_val = s0.min_v;
            let mut max_val = s0.max_v;
            let mut asc = s0.asc;
            let mut desc = s0.desc;

            // SAFETY: `data` is read-only during boundary inspection.
            let full = unsafe { core::slice::from_raw_parts(ptr.0, n) };
            for (id, slot) in stats[..procs].iter().enumerate().skip(1) {
                let st = unsafe { *slot.get() };
                if st.min_v < min_val {
                    min_val = st.min_v;
                }
                if st.max_v > max_val {
                    max_val = st.max_v;
                }
                let boundary = (n * id) / procs;
                if full[boundary] < full[boundary - 1] {
                    asc = false;
                } else if full[boundary] > full[boundary - 1] {
                    desc = false;
                }
                asc = asc && st.asc;
                desc = desc && st.desc;
            }

            if min_val == max_val || asc {
                action1.store(ACTION1_DONE, Ordering::Relaxed);
            } else if desc {
                action1.store(ACTION1_REVERSE, Ordering::Relaxed);
            } else {
                let min_u = min_val.to_u64();
                let span = max_val.to_u64().wrapping_sub(min_u);
                let bit_len = 64 - span.leading_zeros();

                if bit_len <= 8 && n < 250_000 {
                    // Single-threaded counting sort beats thread-barrier cost for small 8-bit spans.
                    // SAFETY: Only thread 0 mutates `data` between [B1] and [B2].
                    let full_mut = unsafe { core::slice::from_raw_parts_mut(ptr.0, n) };
                    let mut counts = [0usize; 256];
                    for &v in full_mut.iter() {
                        let idx = v.to_u64().wrapping_sub(min_u) as u8;
                        counts[idx as usize] += 1;
                    }
                    let mut out = 0usize;
                    let limit = span as usize;
                    for (r, &c) in counts[..=limit].iter().enumerate() {
                        if c > 0 {
                            let val = T::from_u64(min_u.wrapping_add(r as u64));
                            full_mut[out..out + c].fill(val);
                            out += c;
                        }
                    }
                    action1.store(ACTION1_DONE, Ordering::Relaxed);
                } else if bit_len <= 8 {
                    min_u_shared.store(min_u, Ordering::Relaxed);
                    action1.store(ACTION1_COUNTING_8BIT, Ordering::Relaxed);
                } else {
                    action1.store(ACTION1_PARALLEL_SPLIT, Ordering::Relaxed);
                }
            }
        }

        barrier.wait(); // [B2]

        match action1.load(Ordering::Relaxed) {
            ACTION1_DONE | ACTION1_PARALLEL_SPLIT => return,
            ACTION1_REVERSE => {
                let half = n / 2;
                let r_lo = (half * p) / procs;
                let r_hi = (half * (p + 1)) / procs;
                // SAFETY: Indices `i` in `[r_lo..r_hi)` and `n - 1 - i` are
                // strictly disjoint across all `p in 0..procs`.
                unsafe {
                    for i in r_lo..r_hi {
                        let j = n - 1 - i;
                        core::ptr::swap(ptr.0.add(i), ptr.0.add(j));
                    }
                }
                return;
            }
            _ => {}
        }

        // --- Parallel 8-bit counting sort ---
        let min_u = min_u_shared.load(Ordering::Relaxed);
        {
            // SAFETY: `[lo..hi)` is read-only; only worker `p` writes `local_counts[p]`.
            let slice = unsafe { core::slice::from_raw_parts(ptr.0.add(lo), hi - lo) };
            let lc = unsafe { &mut *local_counts[p].get() };
            for &v in slice {
                let idx = v.to_u64().wrapping_sub(min_u) as u8;
                lc[idx as usize] += 1;
            }
        }

        barrier.wait(); // [B3]

        if p == 0 {
            let meta = unsafe { &mut *counting_meta.get() };
            let mut total = 0usize;
            let mut active_rooms = 0usize;
            for r in 0..256 {
                let mut c = 0usize;
                for slot in &local_counts[..procs] {
                    // SAFETY: All workers finished writing `local_counts` before [B3].
                    c += unsafe { (*slot.get())[r] };
                }
                if c > 0 {
                    meta.offsets[r] = total;
                    total += c;
                    meta.ends[r] = total;
                    meta.active_list[active_rooms] = r as u8;
                    active_rooms += 1;
                }
            }
            active_rooms_shared.store(active_rooms, Ordering::Relaxed);
        }

        barrier.wait(); // [B4]

        let active_rooms = active_rooms_shared.load(Ordering::Relaxed);
        // SAFETY: `counting_meta` is read-only after [B4].
        let meta = unsafe { &*counting_meta.get() };
        loop {
            let idx = next_idx.fetch_add(1, Ordering::Relaxed);
            if idx >= active_rooms {
                break;
            }
            let r = meta.active_list[idx] as usize;
            let val = T::from_u64(min_u.wrapping_add(r as u64));
            let st = meta.offsets[r];
            let len = meta.ends[r] - st;
            // SAFETY: Each `idx` maps to a unique `r` with strictly disjoint `[st..st+len)`.
            let sub = unsafe { core::slice::from_raw_parts_mut(ptr.0.add(st), len) };
            sub.fill(val);
        }
    };

    thread::scope(|s| {
        for p in 1..procs {
            s.spawn(move || worker(p));
        }
        worker(0);
    });

    if action1.load(Ordering::Relaxed) == ACTION1_PARALLEL_SPLIT {
        let depth = (usize::BITS - (procs - 1).leading_zeros()).max(1);
        parallel_split_and_maze(data, depth);
    }
}

fn parallel_split_and_maze<T: Integer>(data: &mut [T], depth: u32) {
    let n = data.len();
    if depth == 0 || n < PARALLEL_LEAF_LIMIT {
        maze256(data);
        return;
    }

    let mut samples = [T::ZERO; 65];
    let step = n / 65;
    for (i, slot) in samples.iter_mut().enumerate() {
        *slot = data[i * step];
    }
    samples.sort_unstable();
    let pivot = samples[32];

    // Heavy duplicates around the median: 3-way partition resolves equal middle immediately.
    if samples[16] == samples[48] {
        let (lt, gt) = partition_3way(data, pivot);
        let (left, rest) = data.split_at_mut(lt);
        let right = &mut rest[gt - lt..];
        if left.len() > 1 && right.len() > 1 {
            thread::scope(|s| {
                s.spawn(|| parallel_split_and_maze(left, depth - 1));
                parallel_split_and_maze(right, depth - 1);
            });
        } else if left.len() > 1 {
            parallel_split_and_maze(left, depth - 1);
        } else if right.len() > 1 {
            parallel_split_and_maze(right, depth - 1);
        }
        return;
    }

    let mid = partition_block_branchless(data, pivot);
    if mid == 0 || mid == n {
        maze256(data);
        return;
    }

    let (left, right) = data.split_at_mut(mid);
    thread::scope(|s| {
        s.spawn(|| parallel_split_and_maze(left, depth - 1));
        parallel_split_and_maze(right, depth - 1);
    });
}

/// BlockQuicksort-style branchless 2-way partition: moves all elements `< pivot`
/// before elements `>= pivot`.
fn partition_block_branchless<T: Integer>(a: &mut [T], pivot: T) -> usize {
    const BLK: usize = 64;
    let mut off_l = [0u8; BLK];
    let mut off_r = [0u8; BLK];

    let mut l = 0usize;
    let mut r = a.len();
    let mut start_l = 0usize;
    let mut num_l = 0usize;
    let mut start_r = 0usize;
    let mut num_r = 0usize;

    while r - l >= 2 * BLK {
        if num_l == 0 {
            start_l = 0;
            let s = &a[l..l + BLK];
            for (i, &v) in s.iter().enumerate() {
                off_l[num_l] = i as u8;
                num_l += usize::from(v >= pivot);
            }
        }
        if num_r == 0 {
            start_r = 0;
            let base = r - BLK;
            let s = &a[base..r];
            for i in 0..BLK {
                let idx = BLK - 1 - i;
                off_r[num_r] = idx as u8;
                num_r += usize::from(s[idx] < pivot);
            }
        }

        let cnt = num_l.min(num_r);
        if cnt > 0 {
            let base_r = r - BLK;
            for k in 0..cnt {
                let li = l + off_l[start_l + k] as usize;
                let ri = base_r + off_r[start_r + k] as usize;
                a.swap(li, ri);
            }
            start_l += cnt;
            start_r += cnt;
            num_l -= cnt;
            num_r -= cnt;
        }

        if num_l == 0 {
            l += BLK;
        }
        if num_r == 0 {
            r -= BLK;
        }
    }

    let mut i = l;
    let mut j = r;
    while i < j {
        while i < j && a[i] < pivot {
            i += 1;
        }
        while i < j && a[j - 1] >= pivot {
            j -= 1;
        }
        if i + 1 < j {
            j -= 1;
            a.swap(i, j);
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn partition_3way<T: Integer>(a: &mut [T], pivot: T) -> (usize, usize) {
    let mut lt = 0usize;
    let mut i = 0usize;
    let mut gt = a.len();
    while i < gt {
        let v = a[i];
        if v < pivot {
            a.swap(lt, i);
            lt += 1;
            i += 1;
        } else if v > pivot {
            gt -= 1;
            a.swap(i, gt);
        } else {
            i += 1;
        }
    }
    (lt, gt)
}
