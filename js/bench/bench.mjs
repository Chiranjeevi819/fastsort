import { performance } from "node:perf_hooks";
import { maze256, maze256Int32, maze256Uint32, maze256Float32 } from "../src/index.mjs";

function makeRng(seed = 108) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0);
  };
}

function bench(fn, template, iters) {
  const Ctor = template.constructor;
  const work = new Ctor(template.length);

  // Warmup (let V8 TurboFan optimize)
  for (let i = 0; i < 3; i++) {
    work.set(template);
    fn(work);
  }

  let totalMs = 0;
  for (let i = 0; i < iters; i++) {
    work.set(template);
    const t0 = performance.now();
    fn(work);
    totalMs += performance.now() - t0;
  }
  return (totalMs * 1e6) / (template.length * iters);
}

console.log(`Node ${process.version} — V8 TypedArray.sort() vs fastsort.maze256 (ns/elem — lower is faster)\n`);

const rng = makeRng(108);

// 1. Int32Array size scaling
console.log("=== Int32Array Size Scaling (Uniform 32-bit) ===");
console.log(`${"N".padEnd(10)} | ${"maze256".padStart(12)} | ${"V8 .sort()".padStart(12)} | ${"Speedup".padStart(9)}`);
console.log("-".repeat(52));
for (const n of [1_000, 10_000, 100_000, 1_000_000]) {
  const iters = n >= 1_000_000 ? 10 : n >= 100_000 ? 30 : 200;
  const data = new Int32Array(n);
  for (let i = 0; i < n; i++) data[i] = rng() | 0;
  const nsMaze = bench(maze256Int32, data, iters);
  const nsV8 = bench((a) => a.sort(), data, iters);
  console.log(
    `${String(n).padEnd(10)} | ${nsMaze.toFixed(2).padStart(9)} ns | ${nsV8.toFixed(2).padStart(9)} ns | ${(nsV8 / nsMaze).toFixed(2).padStart(8)}x`
  );
}

// 2. TypedArray types at N = 1,000,000
console.log("\n=== TypedArray Types (N = 1,000,000, Uniform) ===");
console.log(`${"Type".padEnd(14)} | ${"maze256".padStart(12)} | ${"V8 .sort()".padStart(12)} | ${"Speedup".padStart(9)}`);
console.log("-".repeat(56));
const N = 1_000_000;
const typeCases = [
  ["Int32Array", Int32Array, maze256Int32, () => rng() | 0],
  ["Uint32Array", Uint32Array, maze256Uint32, () => rng() >>> 0],
  ["Float32Array", Float32Array, maze256Float32, () => ((rng() | 0) / 0x7fffffff) * 1e6],
  ["Int16Array", Int16Array, maze256, () => (rng() & 0xffff) - 32768],
  ["Int8Array", Int8Array, maze256, () => (rng() & 0xff) - 128],
];
for (const [name, Ctor, fn, gen] of typeCases) {
  const data = new Ctor(N);
  for (let i = 0; i < N; i++) data[i] = gen();
  const nsMaze = bench(fn, data, 10);
  const nsV8 = bench((a) => a.sort(), data, 10);
  console.log(
    `${name.padEnd(14)} | ${nsMaze.toFixed(2).padStart(9)} ns | ${nsV8.toFixed(2).padStart(9)} ns | ${(nsV8 / nsMaze).toFixed(2).padStart(8)}x`
  );
}
