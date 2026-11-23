import test from "node:test";
import assert from "node:assert/strict";
import { sort, maze256, maze256Int32, maze256Uint32, maze256Float32 } from "../src/index.mjs";

function makeRng(seed = 108) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0);
  };
}

function arraysEqual(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (Object.is(a[i], b[i])) continue;
    return false;
  }
  return true;
}

test("all TypedArray integer types across small, medium, and large sizes", () => {
  const rng = makeRng(42);
  const sizes = [0, 1, 2, 48, 49, 256, 4096, 4097, 20000];
  const types = [
    ["Int8Array", Int8Array, () => (rng() & 0xff) - 128],
    ["Uint8Array", Uint8Array, () => rng() & 0xff],
    ["Uint8ClampedArray", Uint8ClampedArray, () => rng() & 0xff],
    ["Int16Array", Int16Array, () => (rng() & 0xffff) - 32768],
    ["Uint16Array", Uint16Array, () => rng() & 0xffff],
    ["Int32Array", Int32Array, () => rng() | 0],
    ["Uint32Array", Uint32Array, () => rng() >>> 0],
  ];

  for (const [name, Ctor, gen] of types) {
    for (const n of sizes) {
      const data = new Ctor(n);
      for (let i = 0; i < n; i++) data[i] = gen();
      const expected = new Ctor(data);
      expected.sort();

      const got = new Ctor(data);
      sort(got);
      assert.ok(arraysEqual(got, expected), `${name} mismatch at n=${n}`);
    }
  }
});

test("Float32Array including -0, +0, ±Infinity, subnormals, and NaNs", () => {
  const rng = makeRng(77);
  const specials = [
    NaN,
    -Infinity,
    Infinity,
    -0.0,
    0.0,
    -1e-40,
    1e-40,
    -3.4e38,
    3.4e38,
    -1.5,
    1.5,
  ];
  for (const n of [0, 1, 15, 64, 4097, 20000]) {
    const data = new Float32Array(n);
    for (let i = 0; i < n; i++) {
      if (i < specials.length) {
        data[i] = specials[i];
      } else {
        data[i] = ((rng() | 0) / 0x7fffffff) * 1e6;
      }
    }
    const expected = new Float32Array(data);
    expected.sort();

    const got = new Float32Array(data);
    maze256Float32(got);
    assert.ok(arraysEqual(got, expected), `Float32Array mismatch at n=${n}`);
  }
});

test("adversarial edge cases: pending stress, extremes, staircase, monotonic", () => {
  const rng = makeRng(999);

  // Pending stress (256 cyclic buckets)
  const pend = new Int32Array(65536);
  for (let i = 0; i < pend.length; i++) {
    pend[i] = ((255 - (i & 255)) << 20) | (i & 0xfffff);
  }
  const pendExp = new Int32Array(pend).sort();
  maze256Int32(pend);
  assert.ok(arraysEqual(pend, pendExp), "pending stress failed");

  // Full Int32 extremes
  const ext = new Int32Array(20000);
  const vals = [-2147483648, -2147483647, -1, 0, 1, 2147483646, 2147483647];
  for (let i = 0; i < ext.length; i++) ext[i] = vals[rng() % vals.length];
  const extExp = new Int32Array(ext).sort();
  maze256(ext);
  assert.ok(arraysEqual(ext, extExp), "extremes failed");

  // Full Uint32 extremes
  const uext = new Uint32Array(20000);
  const uvals = [0, 1, 255, 256, 0x7fffffff, 0x80000000, 0xfffffffe, 0xffffffff];
  for (let i = 0; i < uext.length; i++) uext[i] = uvals[rng() % uvals.length];
  const uextExp = new Uint32Array(uext).sort();
  maze256Uint32(uext);
  assert.ok(arraysEqual(uext, uextExp), "uint32 extremes failed");

  // Reverse sorted
  const rev = new Int32Array(20000);
  for (let i = 0; i < rev.length; i++) rev[i] = 20000 - i;
  const revExp = new Int32Array(rev).sort();
  sort(rev);
  assert.ok(arraysEqual(rev, revExp), "reverse sorted failed");
});
