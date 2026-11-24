use crate::maze256;
#[cfg(feature = "std")]
use crate::maze256_parallel;

#[inline(always)]
fn f32_involution(u: i32) -> i32 {
    u ^ ((u >> 31) & 0x7fff_ffff)
}

#[inline(always)]
fn f64_involution(u: i64) -> i64 {
    u ^ ((u >> 63) & 0x7fff_ffff_ffff_ffff)
}

/// Sorts `data` in ascending total order (matching [`f32::total_cmp`]) in-place
/// with zero heap allocations using the **In-Place Maze256** algorithm.
///
/// Uses a zero-copy, self-inverse IEEE-754 bit involution
/// `u ^ ((u >> 31) & 0x7fffffff)` that maps every `f32` bit pattern
/// (including `-NaN`, `-Infinity`, `-0.0`, `+0.0`, `+Infinity`, and `+NaN`)
/// monotonically onto signed `i32` order.
#[inline]
pub fn sort_f32(data: &mut [f32]) {
    maze256_f32(data);
}

/// Sorts `data` in ascending total order (matching [`f32::total_cmp`]) in-place
/// with zero heap allocations using **In-Place Maze256**.
pub fn maze256_f32(data: &mut [f32]) {
    let n = data.len();
    if n <= 1 {
        return;
    }
    // SAFETY: `f32` and `i32` have identical size (4 bytes) and alignment (4),
    // and every 32-bit pattern is valid for both types.
    let view = unsafe { core::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut i32, n) };

    for v in view.iter_mut() {
        *v = f32_involution(*v);
    }
    maze256(view);
    for v in view.iter_mut() {
        *v = f32_involution(*v);
    }
}

/// Sorts `data` in ascending total order (matching [`f64::total_cmp`]) in-place
/// with zero heap allocations using the **In-Place Maze256** algorithm.
///
/// Uses a zero-copy, self-inverse IEEE-754 bit involution
/// `u ^ ((u >> 63) & 0x7fffffffffffffff)` that maps every `f64` bit pattern
/// (including `-NaN`, `-Infinity`, `-0.0`, `+0.0`, `+Infinity`, and `+NaN`)
/// monotonically onto signed `i64` order.
#[inline]
pub fn sort_f64(data: &mut [f64]) {
    maze256_f64(data);
}

/// Sorts `data` in ascending total order (matching [`f64::total_cmp`]) in-place
/// with zero heap allocations using **In-Place Maze256**.
pub fn maze256_f64(data: &mut [f64]) {
    let n = data.len();
    if n <= 1 {
        return;
    }
    // SAFETY: `f64` and `i64` have identical size (8 bytes) and alignment (8),
    // and every 64-bit pattern is valid for both types.
    let view = unsafe { core::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut i64, n) };

    for v in view.iter_mut() {
        *v = f64_involution(*v);
    }
    maze256(view);
    for v in view.iter_mut() {
        *v = f64_involution(*v);
    }
}

/// Sorts `data` in ascending total order (matching [`f32::total_cmp`]) in-place
/// across multiple CPU cores using [`maze256_parallel`].
#[cfg(feature = "std")]
#[inline]
pub fn sort_f32_parallel(data: &mut [f32]) {
    maze256_f32_parallel(data);
}

/// Sorts `data` in ascending total order (matching [`f32::total_cmp`]) in-place
/// across multiple CPU cores using [`maze256_parallel`].
#[cfg(feature = "std")]
pub fn maze256_f32_parallel(data: &mut [f32]) {
    let n = data.len();
    if n <= 1 {
        return;
    }
    // SAFETY: `f32` and `i32` have identical size (4 bytes) and alignment (4).
    let view = unsafe { core::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut i32, n) };

    for v in view.iter_mut() {
        *v = f32_involution(*v);
    }
    maze256_parallel(view);
    for v in view.iter_mut() {
        *v = f32_involution(*v);
    }
}

/// Sorts `data` in ascending total order (matching [`f64::total_cmp`]) in-place
/// across multiple CPU cores using [`maze256_parallel`].
#[cfg(feature = "std")]
#[inline]
pub fn sort_f64_parallel(data: &mut [f64]) {
    maze256_f64_parallel(data);
}

/// Sorts `data` in ascending total order (matching [`f64::total_cmp`]) in-place
/// across multiple CPU cores using [`maze256_parallel`].
#[cfg(feature = "std")]
pub fn maze256_f64_parallel(data: &mut [f64]) {
    let n = data.len();
    if n <= 1 {
        return;
    }
    // SAFETY: `f64` and `i64` have identical size (8 bytes) and alignment (8).
    let view = unsafe { core::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut i64, n) };

    for v in view.iter_mut() {
        *v = f64_involution(*v);
    }
    maze256_parallel(view);
    for v in view.iter_mut() {
        *v = f64_involution(*v);
    }
}
