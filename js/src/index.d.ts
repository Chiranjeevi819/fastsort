export type SortableTypedArray =
  | Int8Array
  | Uint8Array
  | Uint8ClampedArray
  | Int16Array
  | Uint16Array
  | Int32Array
  | Uint32Array
  | Float32Array;

/**
 * Sorts a supported TypedArray in ascending order in-place using In-Place Maze256,
 * with zero per-call heap allocations.
 *
 * Matches standard TypedArray.prototype.sort() ordering, including -0 before +0
 * and NaNs placed at the end for Float32Array.
 */
export function sort<T extends SortableTypedArray>(data: T): T;

/**
 * In-Place Maze256: range-adaptive 8-bit MSD radix sort with 64-byte block-buffered
 * scatter, zero-write cycle-leader fallback, and L1 ping-pong leaf sorting.
 */
export function maze256<T extends SortableTypedArray>(data: T): T;

/** Monomorphic fast entry point for Int32Array. */
export function maze256Int32(data: Int32Array): Int32Array;

/** Monomorphic fast entry point for Uint32Array. */
export function maze256Uint32(data: Uint32Array): Uint32Array;

/** Monomorphic fast entry point for Float32Array (zero-copy IEEE-754 bit-flip). */
export function maze256Float32(data: Float32Array): Float32Array;
