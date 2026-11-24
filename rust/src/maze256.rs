use core::mem::MaybeUninit;
use core::ptr;

use crate::{handle_monotonic, Integer};

pub(crate) const SMALL_CUTOFF: usize = 256;
pub(crate) const LEAF_CUTOFF: usize = 128;
pub(crate) const SMALL_STACK_SIZE: usize = 1024;
pub(crate) const STACK_BUF_SIZE: usize = 4096;
pub(crate) const BLOCK_SIZE: usize = 8; // 8 x i64 = 64-byte cache line
pub(crate) const PENDING_CAP: usize = 2048; // Provably bounded: <= 256*(BLOCK_SIZE-1)+1 = 1793 < 2048

/// Sorts `data` in ascending order in-place using the **In-Place Maze256**
/// algorithm: a range-adaptive 8-bit MSD radix sort with tiered stack frames,
/// cache-line block-buffered scatter, cycle-leader fallback for skewed data,
/// and TZCNT checkpoint bitmasks.
///
/// Runs in $O(n)$ time (at most 8 radix levels for 64-bit integers) and uses
/// strictly $O(1)$ stack space with **0 heap allocations**.
pub fn maze256<T: Integer>(data: &mut [T]) {
    let n = data.len();
    if n <= SMALL_CUTOFF {
        data.sort_unstable();
        return;
    }

    // Specialized 1-pass counting sort for 8-bit integer types (i8 / u8):
    // avoids the min/max scan and fills via hardware-vectorized memset.
    if T::BITS == 8 {
        if handle_monotonic(data) {
            return;
        }
        sort_8bit(data);
        return;
    }

    if handle_monotonic(data) {
        return;
    }
    if n <= SMALL_STACK_SIZE {
        in_place_maze_small(data);
        return;
    }
    if n <= STACK_BUF_SIZE {
        in_place_maze_med(data);
        return;
    }
    in_place_maze_large(data);
}

#[inline(never)]
fn sort_8bit<T: Integer>(data: &mut [T]) {
    let mut c0 = [0usize; 256];
    let mut c1 = [0usize; 256];
    let mut c2 = [0usize; 256];
    let mut c3 = [0usize; 256];
    let offset: u64 = if T::IS_SIGNED { 128 } else { 0 };

    let mut i = 0usize;
    let n = data.len();
    while i + 4 <= n {
        c0[data[i].to_u64().wrapping_add(offset) as u8 as usize] += 1;
        c1[data[i + 1].to_u64().wrapping_add(offset) as u8 as usize] += 1;
        c2[data[i + 2].to_u64().wrapping_add(offset) as u8 as usize] += 1;
        c3[data[i + 3].to_u64().wrapping_add(offset) as u8 as usize] += 1;
        i += 4;
    }
    for &v in &data[i..] {
        c0[v.to_u64().wrapping_add(offset) as u8 as usize] += 1;
    }

    let mut out = 0usize;
    for r in 0..256 {
        let c = c0[r] + c1[r] + c2[r] + c3[r];
        if c > 0 {
            let val = T::from_u64((r as u64).wrapping_sub(offset));
            data[out..out + c].fill(val);
            out += c;
        }
    }
}

#[inline(never)]
fn in_place_maze_small<T: Integer>(data: &mut [T]) {
    let mut stack_aux = [MaybeUninit::<T>::uninit(); SMALL_STACK_SIZE];
    let n = data.len();
    stack_maze_pp(data, &mut stack_aux[..n], false, None);
}

#[inline(never)]
fn in_place_maze_med<T: Integer>(data: &mut [T]) {
    let mut stack_aux = [MaybeUninit::<T>::uninit(); STACK_BUF_SIZE];
    let n = data.len();
    stack_maze_pp(data, &mut stack_aux[..n], false, None);
}

#[inline(never)]
pub(crate) fn in_place_maze_large<T: Integer>(data: &mut [T]) {
    let mut stack_aux = [MaybeUninit::<T>::uninit(); STACK_BUF_SIZE];
    let mut buf = [[MaybeUninit::<T>::uninit(); BLOCK_SIZE]; 256];
    let mut pending = [MaybeUninit::<T>::uninit(); PENDING_CAP];
    in_place_maze(data, &mut stack_aux, &mut buf, &mut pending, None);
}

#[inline(always)]
fn scan_min_max<T: Integer>(src: &[T]) -> (T, T) {
    let mut min_val = src[0];
    let mut max_val = min_val;
    for &v in &src[1..] {
        if v < min_val {
            min_val = v;
        } else if v > max_val {
            max_val = v;
        }
    }
    (min_val, max_val)
}

#[inline(always)]
fn histogram_shift<T: Integer>(src: &[T], min_u: u64, shift: u32, counts: &mut [usize; 256]) {
    if src.len() >= 16_384 {
        let mut c1 = [0usize; 256];
        let mut c2 = [0usize; 256];
        let mut c3 = [0usize; 256];
        let mut i = 0usize;
        let n = src.len();
        while i + 4 <= n {
            let b0 = (src[i].to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
            let b1 = (src[i + 1].to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
            let b2 = (src[i + 2].to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
            let b3 = (src[i + 3].to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
            counts[b0] += 1;
            c1[b1] += 1;
            c2[b2] += 1;
            c3[b3] += 1;
            i += 4;
        }
        for &v in &src[i..] {
            let b = (v.to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
            counts[b] += 1;
        }
        for r in 0..256 {
            counts[r] += c1[r] + c2[r] + c3[r];
        }
    } else {
        for &v in src {
            let b = (v.to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
            counts[b] += 1;
        }
    }
}

fn in_place_maze<T: Integer>(
    src: &mut [T],
    stack_aux: &mut [MaybeUninit<T>; STACK_BUF_SIZE],
    buf: &mut [[MaybeUninit<T>; BLOCK_SIZE]; 256],
    pending: &mut [MaybeUninit<T>; PENDING_CAP],
    known_range: Option<(u64, u32)>,
) {
    let n = src.len();
    if n <= LEAF_CUTOFF {
        src.sort_unstable();
        return;
    }
    if n <= STACK_BUF_SIZE {
        stack_maze_pp(src, &mut stack_aux[..n], false, known_range);
        return;
    }

    let (min_u, bit_len, span_opt) = match known_range {
        Some((mu, rem_bits)) => (mu, rem_bits, None),
        None => {
            let (min_val, max_val) = scan_min_max(src);
            if min_val == max_val {
                return;
            }
            let min_u = min_val.to_u64();
            let span = max_val.to_u64().wrapping_sub(min_u);
            let bit_len = 64 - span.leading_zeros();
            (min_u, bit_len, Some(span))
        }
    };

    let mut counts = [0usize; 256];

    // Direct in-place counting sort when active span fits in 8 bits.
    if bit_len <= 8 {
        let eff_min = match span_opt {
            Some(_) => min_u,
            None => {
                let mask = (1u64 << bit_len) - 1;
                let prefix = src[0].to_u64().wrapping_sub(min_u) & !mask;
                min_u.wrapping_add(prefix)
            }
        };
        let limit = match span_opt {
            Some(s) => s as usize,
            None => ((1usize << bit_len) - 1).min(255),
        };
        for &v in src.iter() {
            let idx = v.to_u64().wrapping_sub(eff_min) as u8;
            counts[idx as usize] += 1;
        }
        let mut out = 0usize;
        for (r, &c) in counts[..=limit].iter().enumerate() {
            if c > 0 {
                let val = T::from_u64(eff_min.wrapping_add(r as u64));
                src[out..out + c].fill(val);
                out += c;
            }
        }
        return;
    }

    // Pass 1: Histogram on top 8 active bits (u8 index eliminates bounds checks).
    let shift = bit_len - 8;
    histogram_shift(src, min_u, shift, &mut counts);

    // Build prefix sums and 4-register TZCNT checkpoint bitmask in 256 iterations.
    let mut offsets = [0usize; 256];
    let mut write_pos = [0usize; 256];
    let mut ends = [0usize; 256];
    let mut room_mask = [0u64; 4];
    let mut total = 0usize;
    let mut active_rooms = 0usize;
    let mut last_room = 0u8;
    let mut max_count = 0usize;

    for r in 0..256 {
        let c = counts[r];
        if c > 0 {
            room_mask[r >> 6] |= 1u64 << (r & 63);
            offsets[r] = total;
            write_pos[r] = total;
            total += c;
            ends[r] = total;
            last_room = r as u8;
            active_rooms += 1;
            if c > max_count {
                max_count = c;
            }
        }
    }

    // Pass 2: Adaptive in-place routing.
    let spread_out = active_rooms >= 16 && max_count < (n * 3) / 4;
    if active_rooms > 1 {
        if spread_out {
            // Spread-out data: cache-line block-buffered in-place scatter.
            scatter_block_buffered_with_scratch(
                src,
                min_u,
                shift,
                &room_mask,
                &offsets,
                &mut write_pos,
                &ends,
                buf,
                pending,
            );
        } else {
            // Skewed or sparse data: zero-write cycle-leader routing.
            route_cycle_leader(
                src,
                min_u,
                shift,
                &room_mask,
                &mut write_pos,
                &ends,
                last_room,
            );
        }
    }

    // For spread-out passes, propagate `(min_u, shift)` down to skip redundant
    // min/max scans on the next level; for skewed passes, re-scan min/max to
    // compress unused bit gaps immediately.
    let next_known = if spread_out {
        Some((min_u, shift))
    } else {
        None
    };

    // Recurse into occupied rooms via TZCNT checkpoint bitmasks.
    for (z, &mask) in room_mask.iter().enumerate() {
        let mut rm = mask;
        while rm != 0 {
            let r = (z << 6) | (rm.trailing_zeros() as usize);
            rm &= rm - 1;
            let c = counts[r];
            if c > 1 {
                let st = offsets[r];
                in_place_maze(&mut src[st..st + c], stack_aux, buf, pending, next_known);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn route_cycle_leader<T: Integer>(
    src: &mut [T],
    min_u: u64,
    shift: u32,
    room_mask: &[u64; 4],
    write_pos: &mut [usize; 256],
    ends: &[usize; 256],
    last_room: u8,
) {
    let src_ptr = src.as_mut_ptr();
    'outer: for (z, &mask) in room_mask.iter().enumerate() {
        let mut rm = mask;
        while rm != 0 {
            let r = ((z << 6) | (rm.trailing_zeros() as usize)) as u8;
            rm &= rm - 1;
            if r == last_room {
                break 'outer;
            }
            let end = ends[r as usize];
            let mut wp = write_pos[r as usize];
            while wp < end {
                // SAFETY: `wp < end <= src.len()`, and `next_pos < ends[dest] <= src.len()`
                // because histogram counts exactly match the number of elements per bucket.
                unsafe {
                    let mut v = *src_ptr.add(wp);
                    let mut dest = (v.to_u64().wrapping_sub(min_u) >> shift) as u8;
                    if dest == r {
                        wp += 1;
                        continue;
                    }
                    loop {
                        let next_pos = *write_pos.get_unchecked(dest as usize);
                        *write_pos.get_unchecked_mut(dest as usize) = next_pos + 1;
                        let slot = src_ptr.add(next_pos);
                        ptr::swap(slot, &mut v);
                        dest = (v.to_u64().wrapping_sub(min_u) >> shift) as u8;
                        if dest == r {
                            *src_ptr.add(wp) = v;
                            wp += 1;
                            break;
                        }
                    }
                }
            }
            write_pos[r as usize] = wp;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn scatter_block_buffered_with_scratch<T: Integer>(
    src: &mut [T],
    min_u: u64,
    shift: u32,
    room_mask: &[u64; 4],
    offsets: &[usize; 256],
    write_pos: &mut [usize; 256],
    ends: &[usize; 256],
    buf: &mut [[MaybeUninit<T>; BLOCK_SIZE]; 256],
    pending: &mut [MaybeUninit<T>; PENDING_CAP],
) {
    let mut read_pos = *offsets;
    let mut buf_len = [0u8; 256];
    let mut pending_len = 0usize;

    let src_ptr = src.as_mut_ptr();
    let pending_ptr = pending.as_mut_ptr() as *mut T;

    // SAFETY invariants (proven in ALGORITHM.md §4.2):
    // - `pending_len <= 256 * (BLOCK_SIZE - 1) + 1 = 1793 < PENDING_CAP (2048)`
    // - `dest < 256`, `bl < BLOCK_SIZE (8)`
    // - `read_pos[r] < ends[r] <= src.len()` and `write_pos[dest] + BLOCK_SIZE <= ends[dest] <= src.len()`
    // - Every `pending[0..pending_len]` and `buf[dest][0..buf_len[dest]]` slot is initialized before read.
    unsafe {
        // Phase A: Scan all unread elements across active rooms. Whenever a
        // block flush evicts unread elements onto `pending`, drain `pending`
        // completely in a tight inner loop before reading from `src` again.
        for (z, &mask) in room_mask.iter().enumerate() {
            let mut rm = mask;
            while rm != 0 {
                let r = (z << 6) | (rm.trailing_zeros() as usize);
                rm &= rm - 1;
                let end = *ends.get_unchecked(r);

                while *read_pos.get_unchecked(r) < end {
                    let rp_cur = *read_pos.get_unchecked(r);
                    *read_pos.get_unchecked_mut(r) = rp_cur + 1;
                    let v = *src_ptr.add(rp_cur);

                    let dest = (v.to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
                    let bl = *buf_len.get_unchecked(dest) as usize;
                    let dest_buf = buf.get_unchecked_mut(dest).as_mut_ptr() as *mut T;
                    *dest_buf.add(bl) = v;
                    let next_bl = bl + 1;

                    if next_bl == BLOCK_SIZE {
                        let wp = *write_pos.get_unchecked(dest);
                        let new_wp = wp + BLOCK_SIZE;
                        let rp = *read_pos.get_unchecked(dest);
                        if new_wp > rp {
                            let evict = new_wp - rp;
                            ptr::copy_nonoverlapping(
                                src_ptr.add(rp),
                                pending_ptr.add(pending_len),
                                evict,
                            );
                            pending_len += evict;
                            *read_pos.get_unchecked_mut(dest) = new_wp;
                        }
                        ptr::copy_nonoverlapping(dest_buf, src_ptr.add(wp), BLOCK_SIZE);
                        *write_pos.get_unchecked_mut(dest) = new_wp;
                        *buf_len.get_unchecked_mut(dest) = 0;

                        // Drain all evacuated elements before reading any more from `src`.
                        while pending_len > 0 {
                            pending_len -= 1;
                            let pv = *pending_ptr.add(pending_len);
                            let pdest =
                                (pv.to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
                            let pbl = *buf_len.get_unchecked(pdest) as usize;
                            let pdest_buf = buf.get_unchecked_mut(pdest).as_mut_ptr() as *mut T;
                            *pdest_buf.add(pbl) = pv;
                            let pnext_bl = pbl + 1;

                            if pnext_bl == BLOCK_SIZE {
                                let pwp = *write_pos.get_unchecked(pdest);
                                let pnew_wp = pwp + BLOCK_SIZE;
                                let prp = *read_pos.get_unchecked(pdest);
                                if pnew_wp > prp {
                                    let evict = pnew_wp - prp;
                                    ptr::copy_nonoverlapping(
                                        src_ptr.add(prp),
                                        pending_ptr.add(pending_len),
                                        evict,
                                    );
                                    pending_len += evict;
                                    *read_pos.get_unchecked_mut(pdest) = pnew_wp;
                                }
                                ptr::copy_nonoverlapping(
                                    pdest_buf,
                                    src_ptr.add(pwp),
                                    BLOCK_SIZE,
                                );
                                *write_pos.get_unchecked_mut(pdest) = pnew_wp;
                                *buf_len.get_unchecked_mut(pdest) = 0;
                            } else {
                                *buf_len.get_unchecked_mut(pdest) = pnext_bl as u8;
                            }
                        }
                    } else {
                        *buf_len.get_unchecked_mut(dest) = next_bl as u8;
                    }
                }
            }
        }

        // Phase C: Flush partial tail blocks (`pending_len` is already 0).
        for (z, &mask) in room_mask.iter().enumerate() {
            let mut rm = mask;
            while rm != 0 {
                let r = (z << 6) | (rm.trailing_zeros() as usize);
                rm &= rm - 1;
                let bl = *buf_len.get_unchecked(r) as usize;
                if bl > 0 {
                    let wp = *write_pos.get_unchecked(r);
                    let r_buf = buf.get_unchecked(r).as_ptr() as *const T;
                    ptr::copy_nonoverlapping(r_buf, src_ptr.add(wp), bl);
                }
            }
        }
    }
}

/// L1-resident hoisted ping-pong MSD radix sort (`n <= STACK_BUF_SIZE = 4096`).
///
/// Uses compact `[u16; 256]` metadata tables (512 bytes per table, L1-hot).
pub(crate) fn stack_maze_pp<T: Integer>(
    src: &mut [T],
    dst: &mut [MaybeUninit<T>],
    in_aux: bool,
    known_range: Option<(u64, u32)>,
) {
    let n = src.len();
    if n <= LEAF_CUTOFF {
        src.sort_unstable();
        if in_aux {
            // SAFETY: `src` and `dst` are non-overlapping slices of length `n`.
            unsafe {
                ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr() as *mut T, n);
            }
        }
        return;
    }

    let (min_u, bit_len, span_opt) = match known_range {
        Some((mu, rem_bits)) => (mu, rem_bits, None),
        None => {
            let (min_val, max_val) = scan_min_max(src);
            if min_val == max_val {
                if in_aux {
                    // SAFETY: `dst` has length `n`.
                    let dst_init =
                        unsafe { core::slice::from_raw_parts_mut(dst.as_mut_ptr() as *mut T, n) };
                    dst_init.fill(min_val);
                }
                return;
            }
            let min_u = min_val.to_u64();
            let span = max_val.to_u64().wrapping_sub(min_u);
            let bit_len = 64 - span.leading_zeros();
            (min_u, bit_len, Some(span))
        }
    };

    // Compact L1-friendly u16 tables (n <= 4096 < u16::MAX).
    let mut counts = [0u16; 256];

    if bit_len <= 8 {
        let eff_min = match span_opt {
            Some(_) => min_u,
            None => {
                let mask = (1u64 << bit_len) - 1;
                let prefix = src[0].to_u64().wrapping_sub(min_u) & !mask;
                min_u.wrapping_add(prefix)
            }
        };
        let limit = match span_opt {
            Some(s) => s as usize,
            None => ((1usize << bit_len) - 1).min(255),
        };
        for &v in src.iter() {
            let idx = v.to_u64().wrapping_sub(eff_min) as u8;
            counts[idx as usize] += 1;
        }
        // SAFETY: `target` is either `src` or `dst`, both of length `n`, and we
        // overwrite every element `0..n` completely.
        let target: &mut [T] = if in_aux {
            unsafe { core::slice::from_raw_parts_mut(dst.as_mut_ptr() as *mut T, n) }
        } else {
            src
        };
        let mut out = 0usize;
        for (r, &c) in counts[..=limit].iter().enumerate() {
            let c = c as usize;
            if c > 0 {
                let val = T::from_u64(eff_min.wrapping_add(r as u64));
                target[out..out + c].fill(val);
                out += c;
            }
        }
        return;
    }

    let shift = bit_len - 8;
    for &v in src.iter() {
        let bucket = (v.to_u64().wrapping_sub(min_u) >> shift) as u8;
        counts[bucket as usize] += 1;
    }

    let mut offsets = [0u16; 256];
    let mut write_pos = [0u16; 256];
    let mut room_mask = [0u64; 4];
    let mut total = 0u16;
    let mut max_count = 0usize;
    let mut active_rooms = 0usize;
    for r in 0..256 {
        let c = counts[r];
        if c > 0 {
            room_mask[r >> 6] |= 1u64 << (r & 63);
            offsets[r] = total;
            write_pos[r] = total;
            total += c;
            active_rooms += 1;
            if (c as usize) > max_count {
                max_count = c as usize;
            }
        }
    }

    // If every element fell into a single bucket (possible when `known_range`
    // skipped a zero-bit gap), fall back immediately to a min/max scan.
    if active_rooms == 1 {
        stack_maze_pp(src, dst, in_aux, None);
        return;
    }

    // Scatter all `n` elements from `src` into `dst`, initializing all `n` slots of `dst`.
    let dst_ptr = dst.as_mut_ptr() as *mut T;
    // SAFETY: Sum of `counts` equals `n == dst.len()`, so each `wp < n`.
    unsafe {
        for &v in src.iter() {
            let r = (v.to_u64().wrapping_sub(min_u) >> shift) as u8 as usize;
            let wp = *write_pos.get_unchecked(r) as usize;
            *dst_ptr.add(wp) = v;
            *write_pos.get_unchecked_mut(r) = (wp + 1) as u16;
        }
    }

    // SAFETY: All `n` elements of `dst` were initialized by the scatter loop above.
    let dst_init = unsafe { core::slice::from_raw_parts_mut(dst_ptr, n) };
    let src_uninit =
        unsafe { core::slice::from_raw_parts_mut(src.as_mut_ptr() as *mut MaybeUninit<T>, n) };

    let next_known = if max_count < (n * 3) / 4 {
        Some((min_u, shift))
    } else {
        None
    };

    for (z, &mask) in room_mask.iter().enumerate() {
        let mut rm = mask;
        while rm != 0 {
            let r = (z << 6) | (rm.trailing_zeros() as usize);
            rm &= rm - 1;
            let c = counts[r] as usize;
            let st = offsets[r] as usize;
            if c == 1 {
                if !in_aux {
                    src_uninit[st].write(dst_init[st]);
                }
                continue;
            }
            stack_maze_pp(
                &mut dst_init[st..st + c],
                &mut src_uninit[st..st + c],
                !in_aux,
                next_known,
            );
        }
    }
}
