// In-Place Maze256 for JavaScript/TypeScript TypedArrays.
// Zero per-call heap allocations: scratch buffers are hoisted once per JS realm.
// Tuned for V8 TurboFan: 100% monomorphic Int32Array core + pure 32-bit integer arithmetic.

const INSERTION_LIMIT = 48;
const STACK_BUF_SIZE = 4096;
const BLOCK_SIZE = 8;
const MAX_DEPTH = 5; // 32-bit keys need at most ceil(32/8) = 4 levels

// Hoisted module-level scratch pools (zero GC pressure per sort call).
const I32_AUX = new Int32Array(STACK_BUF_SIZE);
const I32_BUF = new Int32Array(256 * BLOCK_SIZE);
const I32_PENDING = new Int32Array(2048);
const BUF_LEN = new Uint8Array(256);

// Per-depth metadata tables (max depth = 4 for 32-bit integers).
const COUNTS = Array.from({ length: MAX_DEPTH }, () => new Int32Array(256));
const OFFSETS = Array.from({ length: MAX_DEPTH }, () => new Int32Array(256));
const WRITE_POS = Array.from({ length: MAX_DEPTH }, () => new Int32Array(256));
const ENDS = Array.from({ length: MAX_DEPTH }, () => new Int32Array(256));
const READ_POS = new Int32Array(256);
const ACTIVE = Array.from({ length: MAX_DEPTH }, () => new Uint8Array(256));

/**
 * Default entry point: sorts a TypedArray in ascending order in-place using Maze256.
 */
export function sort(data) {
  return maze256(data);
}

/**
 * Dispatches to the monomorphic Maze256 implementation for the given TypedArray type.
 */
export function maze256(data) {
  if (data instanceof Int32Array) return maze256Int32(data);
  if (data instanceof Uint32Array) return maze256Uint32(data);
  if (data instanceof Float32Array) return maze256Float32(data);
  if (data instanceof Int8Array || data instanceof Uint8Array || data instanceof Uint8ClampedArray) {
    return sort8Bit(data);
  }
  if (data instanceof Int16Array) return sort16BitSigned(data);
  if (data instanceof Uint16Array) return sort16BitUnsigned(data);
  throw new TypeError(
    "fastsort.maze256 supports Int8Array, Uint8Array, Uint8ClampedArray, Int16Array, Uint16Array, Int32Array, Uint32Array, and Float32Array"
  );
}

// ============================================================================
// 8-BIT TYPED ARRAYS (DIRECT 1-PASS COUNTING SORT)
// ============================================================================

function sort8Bit(data) {
  const n = data.length;
  if (n <= 1) return data;
  if (n <= INSERTION_LIMIT) {
    insertionSortGeneric(data, 0, n);
    return data;
  }
  if (handleMonotonicGeneric(data, n)) return data;

  const counts = COUNTS[0];
  let minVal = data[0];
  let maxVal = minVal;
  for (let i = 1; i < n; i++) {
    const v = data[i];
    if (v < minVal) minVal = v;
    else if (v > maxVal) maxVal = v;
  }
  if (minVal === maxVal) return data;

  const span = maxVal - minVal;
  counts.fill(0, 0, span + 1);
  for (let i = 0; i < n; i++) {
    counts[data[i] - minVal]++;
  }
  let out = 0;
  for (let r = 0; r <= span; r++) {
    const c = counts[r];
    if (c > 0) {
      data.fill(minVal + r, out, out + c);
      out += c;
    }
  }
  return data;
}

// ============================================================================
// 16-BIT TYPED ARRAYS (DEDICATED 1-PASS 65536-SLOT OR 2-PASS COUNTING SORT)
// ============================================================================

const COUNTS_16 = new Int32Array(65536);

function sort16BitSigned(data) {
  const n = data.length;
  if (n <= 1) return data;
  if (n <= INSERTION_LIMIT) {
    insertionSortGeneric(data, 0, n);
    return data;
  }
  if (handleMonotonicGeneric(data, n)) return data;

  let minVal = data[0];
  let maxVal = minVal;
  for (let i = 1; i < n; i++) {
    const v = data[i];
    if (v < minVal) minVal = v;
    else if (v > maxVal) maxVal = v;
  }
  if (minVal === maxVal) return data;

  const span = maxVal - minVal;
  if (n >= 4096 || span <= 1024) {
    // Direct in-place counting sort using hoisted 65536-slot table (1 read + 1 write pass!)
    COUNTS_16.fill(0, 0, span + 1);
    for (let i = 0; i < n; i++) {
      COUNTS_16[data[i] - minVal]++;
    }
    let out = 0;
    for (let r = 0; r <= span; r++) {
      const c = COUNTS_16[r];
      if (c > 0) {
        data.fill(minVal + r, out, out + c);
        out += c;
      }
    }
    return data;
  }
  data.sort();
  return data;
}

function sort16BitUnsigned(data) {
  const n = data.length;
  if (n <= 1) return data;
  if (n <= INSERTION_LIMIT) {
    insertionSortGeneric(data, 0, n);
    return data;
  }
  if (handleMonotonicGeneric(data, n)) return data;

  let minVal = data[0];
  let maxVal = minVal;
  for (let i = 1; i < n; i++) {
    const v = data[i];
    if (v < minVal) minVal = v;
    else if (v > maxVal) maxVal = v;
  }
  if (minVal === maxVal) return data;

  const span = maxVal - minVal;
  if (n >= 4096 || span <= 1024) {
    COUNTS_16.fill(0, 0, span + 1);
    for (let i = 0; i < n; i++) {
      COUNTS_16[data[i] - minVal]++;
    }
    let out = 0;
    for (let r = 0; r <= span; r++) {
      const c = COUNTS_16[r];
      if (c > 0) {
        data.fill(minVal + r, out, out + c);
        out += c;
      }
    }
    return data;
  }
  data.sort();
  return data;
}

// ============================================================================
// INT32ARRAY — 100% MONOMORPHIC IN-PLACE MAZE256
// ============================================================================

export function maze256Int32(data) {
  const n = data.length;
  if (n <= 1) return data;
  if (n <= INSERTION_LIMIT) {
    insertionSortI32(data, 0, n);
    return data;
  }
  if (handleMonotonicI32(data, n)) return data;
  if (n <= STACK_BUF_SIZE) {
    stackMazePPRecI32(data, I32_AUX, 0, 0, n, false, 0);
    return data;
  }
  inPlaceMazeI32(data, 0, n, 0);
  return data;
}

// ============================================================================
// UINT32ARRAY — ZERO-COPY SIGN-FLIP THROUGH MONOMORPHIC INT32 ENGINE
// ============================================================================

export function maze256Uint32(data) {
  const n = data.length;
  if (n <= 1) return data;
  // Flip sign bit in-place via zero-copy Int32Array view so unsigned order == signed Int32 order.
  const view = new Int32Array(data.buffer, data.byteOffset, n);
  for (let i = 0; i < n; i++) {
    view[i] = view[i] ^ -2147483648;
  }
  maze256Int32(view);
  for (let i = 0; i < n; i++) {
    view[i] = view[i] ^ -2147483648;
  }
  return data;
}

// ============================================================================
// FLOAT32ARRAY — ZERO-COPY IEEE-754 INT32 INVOLUTION + MONOMORPHIC INT32 ENGINE
// ============================================================================

export function maze256Float32(data) {
  const n = data.length;
  if (n <= 1) return data;

  const view = new Int32Array(data.buffer, data.byteOffset, n);

  // Move NaNs to the end (matches Float32Array.prototype.sort) and map non-NaNs
  // monotonically onto signed Int32 order: negative floats XOR 0x7fffffff, positive unchanged.
  let end = n;
  for (let i = 0; i < end; ) {
    const u = view[i];
    if ((u & 0x7fffffff) > 0x7f800000) {
      end--;
      view[i] = view[end];
      view[end] = u;
    } else {
      view[i] = u ^ ((u >> 31) & 0x7fffffff);
      i++;
    }
  }

  if (end > 1) {
    const sub = end === n ? view : view.subarray(0, end);
    maze256Int32(sub);
  }

  // Exact same involution restores IEEE-754 float32 bit patterns.
  for (let i = 0; i < end; i++) {
    const u = view[i];
    view[i] = u ^ ((u >> 31) & 0x7fffffff);
  }

  return data;
}

// ============================================================================
// CORE MONOMORPHIC INT32 IN-PLACE MAZE256 ENGINE
// ============================================================================

function inPlaceMazeI32(src, lo, hi, depth) {
  const n = hi - lo;
  if (n <= INSERTION_LIMIT) {
    insertionSortI32(src, lo, hi);
    return;
  }
  if (n <= STACK_BUF_SIZE) {
    stackMazePPRecI32(src, I32_AUX, lo, 0, n, false, depth);
    return;
  }

  let minVal = src[lo] | 0;
  let maxVal = minVal;
  for (let i = lo + 1; i < hi; i++) {
    const v = src[i] | 0;
    if (v < minVal) minVal = v;
    else if (v > maxVal) maxVal = v;
  }
  if (minVal === maxVal) return;

  // Pure 32-bit unsigned two's-complement span: ((maxVal - minVal) | 0) >>> 0
  const span = ((maxVal - minVal) | 0) >>> 0;
  const bitLen = 32 - Math.clz32(span);
  const counts = COUNTS[depth];

  if (bitLen <= 8) {
    counts.fill(0, 0, span + 1);
    for (let i = lo; i < hi; i++) {
      counts[((src[i] - minVal) | 0) >>> 0]++;
    }
    let out = lo;
    for (let r = 0; r <= span; r++) {
      const c = counts[r];
      if (c > 0) {
        src.fill((minVal + r) | 0, out, out + c);
        out += c;
      }
    }
    return;
  }

  const shift = bitLen - 8;
  counts.fill(0);
  for (let i = lo; i < hi; i++) {
    counts[((src[i] - minVal) | 0) >>> shift]++;
  }

  const offsets = OFFSETS[depth];
  const writePos = WRITE_POS[depth];
  const ends = ENDS[depth];
  const active = ACTIVE[depth];
  let total = lo;
  let activeRooms = 0;
  let lastRoom = 0;
  let maxCount = 0;

  for (let r = 0; r < 256; r++) {
    const c = counts[r];
    if (c > 0) {
      offsets[r] = total;
      writePos[r] = total;
      total += c;
      ends[r] = total;
      lastRoom = r;
      active[activeRooms++] = r;
      if (c > maxCount) maxCount = c;
    }
  }

  if (activeRooms > 1) {
    if (activeRooms >= 32 && maxCount < ((n * 3) >>> 2)) {
      // Spread-out data: 64-byte cache-line block-buffered in-place scatter.
      for (let ai = 0; ai < activeRooms; ai++) {
        const r = active[ai];
        READ_POS[r] = offsets[r];
        BUF_LEN[r] = 0;
      }
      let pendingLen = 0;

      for (let ai = 0; ai < activeRooms; ai++) {
        const r = active[ai];
        const end = ends[r];
        while (READ_POS[r] < end) {
          let v;
          if (pendingLen > 0) {
            v = I32_PENDING[--pendingLen];
          } else {
            v = src[READ_POS[r]++];
          }

          const dest = ((v - minVal) | 0) >>> shift;
          let bl = BUF_LEN[dest];
          const bBase = dest << 3;
          I32_BUF[bBase + bl] = v;
          bl++;

          if (bl === BLOCK_SIZE) {
            const wp = writePos[dest];
            const newWP = wp + BLOCK_SIZE;
            const rp = READ_POS[dest];
            if (newWP > rp) {
              for (let k = rp; k < newWP; k++) {
                I32_PENDING[pendingLen++] = src[k];
              }
              READ_POS[dest] = newWP;
            }
            src[wp] = I32_BUF[bBase];
            src[wp + 1] = I32_BUF[bBase + 1];
            src[wp + 2] = I32_BUF[bBase + 2];
            src[wp + 3] = I32_BUF[bBase + 3];
            src[wp + 4] = I32_BUF[bBase + 4];
            src[wp + 5] = I32_BUF[bBase + 5];
            src[wp + 6] = I32_BUF[bBase + 6];
            src[wp + 7] = I32_BUF[bBase + 7];
            writePos[dest] = newWP;
            BUF_LEN[dest] = 0;
          } else {
            BUF_LEN[dest] = bl;
          }
        }
      }

      while (pendingLen > 0) {
        const v = I32_PENDING[--pendingLen];
        const dest = ((v - minVal) | 0) >>> shift;
        let bl = BUF_LEN[dest];
        const bBase = dest << 3;
        I32_BUF[bBase + bl] = v;
        bl++;
        if (bl === BLOCK_SIZE) {
          const wp = writePos[dest];
          src[wp] = I32_BUF[bBase];
          src[wp + 1] = I32_BUF[bBase + 1];
          src[wp + 2] = I32_BUF[bBase + 2];
          src[wp + 3] = I32_BUF[bBase + 3];
          src[wp + 4] = I32_BUF[bBase + 4];
          src[wp + 5] = I32_BUF[bBase + 5];
          src[wp + 6] = I32_BUF[bBase + 6];
          src[wp + 7] = I32_BUF[bBase + 7];
          writePos[dest] = wp + BLOCK_SIZE;
          BUF_LEN[dest] = 0;
        } else {
          BUF_LEN[dest] = bl;
        }
      }

      for (let ai = 0; ai < activeRooms; ai++) {
        const r = active[ai];
        const bl = BUF_LEN[r];
        if (bl > 0) {
          const wp = writePos[r];
          const bBase = r << 3;
          for (let k = 0; k < bl; k++) {
            src[wp + k] = I32_BUF[bBase + k];
          }
        }
      }
    } else {
      // Skewed / sparse data: zero-write cycle-leader routing.
      for (let ai = 0; ai < activeRooms; ai++) {
        const r = active[ai];
        if (r === lastRoom) break;
        const end = ends[r];
        let wp = writePos[r];
        while (wp < end) {
          let v = src[wp];
          let dest = ((v - minVal) | 0) >>> shift;
          if (dest === r) {
            wp++;
            continue;
          }
          for (;;) {
            const nextPos = writePos[dest]++;
            const tmp = src[nextPos];
            src[nextPos] = v;
            v = tmp;
            dest = ((v - minVal) | 0) >>> shift;
            if (dest === r) {
              src[wp] = v;
              wp++;
              break;
            }
          }
        }
        writePos[r] = wp;
      }
    }
  }

  for (let ai = 0; ai < activeRooms; ai++) {
    const r = active[ai];
    const st = offsets[r];
    const en = ends[r];
    if (en - st > 1) {
      inPlaceMazeI32(src, st, en, depth + 1);
    }
  }
}

function stackMazePPRecI32(orig, aux, origOff, auxOff, n, inAux, depth) {
  if (n <= INSERTION_LIMIT) {
    if (inAux) {
      insertionSortI32(aux, auxOff, auxOff + n);
      for (let i = 0; i < n; i++) orig[origOff + i] = aux[auxOff + i];
    } else {
      insertionSortI32(orig, origOff, origOff + n);
    }
    return;
  }

  const src = inAux ? aux : orig;
  const dst = inAux ? orig : aux;
  const sBase = inAux ? auxOff : origOff;
  const dBase = inAux ? origOff : auxOff;

  let minVal = src[sBase] | 0;
  let maxVal = minVal;
  const sEnd = sBase + n;
  for (let i = sBase + 1; i < sEnd; i++) {
    const v = src[i] | 0;
    if (v < minVal) minVal = v;
    else if (v > maxVal) maxVal = v;
  }
  if (minVal === maxVal) {
    if (inAux) {
      orig.fill(minVal, origOff, origOff + n);
    }
    return;
  }

  const span = ((maxVal - minVal) | 0) >>> 0;
  const bitLen = 32 - Math.clz32(span);
  const counts = COUNTS[depth];

  if (bitLen <= 8) {
    counts.fill(0, 0, span + 1);
    for (let i = sBase; i < sEnd; i++) {
      counts[((src[i] - minVal) | 0) >>> 0]++;
    }
    let out = origOff;
    for (let r = 0; r <= span; r++) {
      const c = counts[r];
      if (c > 0) {
        orig.fill((minVal + r) | 0, out, out + c);
        out += c;
      }
    }
    return;
  }

  const shift = bitLen - 8;
  counts.fill(0);
  for (let i = sBase; i < sEnd; i++) {
    counts[((src[i] - minVal) | 0) >>> shift]++;
  }

  const offsets = OFFSETS[depth];
  const writePos = WRITE_POS[depth];
  const active = ACTIVE[depth];
  let total = 0;
  let activeRooms = 0;
  for (let r = 0; r < 256; r++) {
    const c = counts[r];
    if (c > 0) {
      offsets[r] = total;
      writePos[r] = dBase + total;
      total += c;
      active[activeRooms++] = r;
    }
  }

  for (let i = sBase; i < sEnd; i++) {
    const v = src[i];
    const r = ((v - minVal) | 0) >>> shift;
    dst[writePos[r]++] = v;
  }

  for (let ai = 0; ai < activeRooms; ai++) {
    const r = active[ai];
    const c = counts[r];
    const rel = offsets[r];
    if (c === 1) {
      if (!inAux) {
        orig[origOff + rel] = aux[auxOff + rel];
      }
      continue;
    }
    stackMazePPRecI32(orig, aux, origOff + rel, auxOff + rel, c, !inAux, depth + 1);
  }
}

// ============================================================================
// MONOMORPHIC INT32 & GENERIC HELPERS
// ============================================================================

function handleMonotonicI32(data, n) {
  let asc = true;
  let desc = true;
  for (let i = 1; i < n; i++) {
    const a = data[i];
    const b = data[i - 1];
    if (a < b) asc = false;
    else if (a > b) desc = false;
    if (!asc && !desc) return false;
  }
  if (desc) {
    for (let l = 0, r = n - 1; l < r; l++, r--) {
      const tmp = data[l];
      data[l] = data[r];
      data[r] = tmp;
    }
  }
  return true;
}

function insertionSortI32(a, lo, hi) {
  for (let i = lo + 1; i < hi; i++) {
    const v = a[i];
    if (a[i - 1] > v) {
      let j = i;
      while (j > lo && a[j - 1] > v) {
        a[j] = a[j - 1];
        j--;
      }
      a[j] = v;
    }
  }
}

function handleMonotonicGeneric(data, n) {
  let asc = true;
  let desc = true;
  for (let i = 1; i < n; i++) {
    const a = data[i];
    const b = data[i - 1];
    if (a < b) asc = false;
    else if (a > b) desc = false;
    if (!asc && !desc) return false;
  }
  if (desc) {
    for (let l = 0, r = n - 1; l < r; l++, r--) {
      const tmp = data[l];
      data[l] = data[r];
      data[r] = tmp;
    }
  }
  return true;
}

function insertionSortGeneric(a, lo, hi) {
  for (let i = lo + 1; i < hi; i++) {
    const v = a[i];
    if (a[i - 1] > v) {
      let j = i;
      while (j > lo && a[j - 1] > v) {
        a[j] = a[j - 1];
        j--;
      }
      a[j] = v;
    }
  }
}
