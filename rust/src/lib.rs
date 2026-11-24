//! `fastsort` — Zero-dependency, zero-allocation **In-Place Maze256** sorting
//! for Rust integers and IEEE-754 floats.
//!
//! # Algorithms
//!
//! - [`sort`] / [`maze256`]: Single-threaded, strictly in-place (`0 B` heap
//!   allocations, works in `no_std`) range-adaptive 8-bit MSD radix sort with
//!   tiered `#[inline(never)]` stack frames, 64-byte cache-line block-buffered
//!   scatter, provably bounded spill stack (`pending <= 1793 < 2048`),
//!   cycle-leader fallback for skewed data, and TZCNT `[u64; 4]` bitmasks.
//! - [`sort_parallel`] / [`maze256_parallel`] *(default `std` feature)*:
//!   Multi-core hybrid parallel variant built purely on [`std::thread::scope`]
//!   with zero external dependencies and zero $O(n)$ auxiliary allocations.
//! - [`sort_f32`] / [`sort_f64`] (and parallel variants): Zero-copy in-place
//!   IEEE-754 order-preserving bit involution matching [`f32::total_cmp`] and
//!   [`f64::total_cmp`].
//!
//! # Quick Start
//!
//! ```rust
//! let mut nums = vec![42i64, -7, 1000, 0, -999, 13];
//! fastsort::sort(&mut nums);
//! assert_eq!(nums, vec![-999, -7, 0, 13, 42, 1000]);
//!
//! let mut floats = vec![f32::NAN, 1.5, -0.0, 0.0, f32::NEG_INFINITY, -1.5];
//! fastsort::sort_f32(&mut floats);
//! assert_eq!(floats[0], f32::NEG_INFINITY);
//! assert_eq!(floats[1], -1.5);
//! assert!(floats[2].is_sign_negative() && floats[2] == 0.0);
//! assert!(floats[3].is_sign_positive() && floats[3] == 0.0);
//! assert_eq!(floats[4], 1.5);
//! assert!(floats[5].is_nan());
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

mod float;
mod maze256;
#[cfg(feature = "std")]
mod parallel;

pub use float::{maze256_f32, maze256_f64, sort_f32, sort_f64};
#[cfg(feature = "std")]
pub use float::{maze256_f32_parallel, maze256_f64_parallel, sort_f32_parallel, sort_f64_parallel};
pub use maze256::maze256;
#[cfg(feature = "std")]
pub use parallel::maze256_parallel;

mod sealed {
    pub trait Sealed {}
    impl Sealed for i8 {}
    impl Sealed for i16 {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}
    impl Sealed for isize {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for usize {}
}

/// Sealed trait implemented for all 10 standard fixed- and pointer-width
/// signed and unsigned Rust integer types (`i8`..`i64`, `isize`, `u8`..`u64`,
/// `usize`).
pub trait Integer: Copy + Ord + Send + Sync + sealed::Sealed + 'static {
    /// Zero value of this integer type.
    const ZERO: Self;
    /// Bit width of this integer type.
    const BITS: u32;
    /// Whether this integer type is signed.
    const IS_SIGNED: bool;

    /// Converts `self` to a 64-bit unsigned representation preserving
    /// two's-complement ordering under wrapping subtraction.
    fn to_u64(self) -> u64;

    /// Converts back from the 64-bit unsigned representation.
    fn from_u64(v: u64) -> Self;
}

macro_rules! impl_integer {
    ($($t:ty => signed: $signed:expr),* $(,)?) => {
        $(
            impl Integer for $t {
                const ZERO: Self = 0;
                const BITS: u32 = <$t>::BITS;
                const IS_SIGNED: bool = $signed;

                #[inline(always)]
                fn to_u64(self) -> u64 {
                    if $signed {
                        (self as i64) as u64
                    } else {
                        self as u64
                    }
                }

                #[inline(always)]
                fn from_u64(v: u64) -> Self {
                    v as $t
                }
            }
        )*
    };
}

impl_integer!(
    i8    => signed: true,
    i16   => signed: true,
    i32   => signed: true,
    i64   => signed: true,
    isize => signed: true,
    u8    => signed: false,
    u16   => signed: false,
    u32   => signed: false,
    u64   => signed: false,
    usize => signed: false,
);

/// Sorts `data` in ascending order in-place using the crate's default integer
/// sorting algorithm ([`maze256`]), with $O(n)$ expected time and strictly
/// `0 B` heap allocations.
#[inline]
pub fn sort<T: Integer>(data: &mut [T]) {
    maze256(data);
}

/// Sorts `data` in ascending order in-place across multiple CPU cores using the
/// crate's default parallel integer sorting algorithm ([`maze256_parallel`]),
/// with zero external dependencies and zero $O(n)$ auxiliary allocations.
#[cfg(feature = "std")]
#[inline]
pub fn sort_parallel<T: Integer>(data: &mut [T]) {
    maze256_parallel(data);
}

#[inline]
pub(crate) fn handle_monotonic<T: Integer>(data: &mut [T]) -> bool {
    let n = data.len();
    if n <= 1 {
        return true;
    }
    let mut asc = true;
    let mut desc = true;
    let mut prev = data[0];
    for &v in &data[1..] {
        if v < prev {
            asc = false;
        } else if v > prev {
            desc = false;
        }
        if !asc && !desc {
            return false;
        }
        prev = v;
    }
    if desc && !asc {
        data.reverse();
    }
    true
}
