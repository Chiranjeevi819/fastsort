# @chiranjeevi819/fastsort (JavaScript / TypeScript)

Zero-dependency, zero-per-call-allocation **In-Place Maze256** sorting for JavaScript & TypeScript `TypedArray`s, tailored for V8 TurboFan's monomorphic `Int32Array` pipeline.

- **Supported types:** `Int8Array`, `Uint8Array`, `Uint8ClampedArray`, `Int16Array`, `Uint16Array`, `Int32Array`, `Uint32Array`, and `Float32Array`.
- **Zero per-call heap allocations:** Hoists fixed scratch buffers once per JS realm (~300 KB total including the 16-bit counting table) so repeated sorts generate zero GC pressure.
- **Exact JS `TypedArray.prototype.sort()` semantics:** Including `-0` ordered before `+0`, `±Infinity`, subnormals, and `NaN`s placed at the end for `Float32Array`.
- **Full TypeScript definitions** bundled (`src/index.d.ts`), pure ES module, works in Node 18+, Deno, Bun, browsers, and Cloudflare Workers.

---

## Installation

```bash
npm install @chiranjeevi819/fastsort
```

## Quick Start

```js
import { sort, maze256, maze256Float32 } from "@chiranjeevi819/fastsort";

const ids = new Int32Array([42, -7, 1000, 0, -999, 13]);
sort(ids);
// Int32Array [-999, -7, 0, 13, 42, 1000]

const depths = new Float32Array([NaN, 1.5, -0.0, 0.0, -Infinity, -1.5]);
maze256Float32(depths);
// Float32Array [-Infinity, -1.5, -0, 0, 1.5, NaN]
```

---

## How It Works in JS / V8

- **Monomorphic `Int32Array` Core:** `Int32Array`, `Uint32Array` (via zero-copy `Int32Array` sign-bit XOR), and `Float32Array` (via zero-copy IEEE-754 `Int32` involution `u ^ ((u >> 31) & 0x7fffffff)`) all share a single 100% monomorphic `Int32Array` Maze256 engine with pure 32-bit integer arithmetic (`((v - minVal) | 0) >>> shift`), avoiding V8 polymorphic IC deoptimizations and float64 boxing.
- **Direct 1-Pass Counting Sort for 8-bit & 16-bit TypedArrays:** `Int8Array`, `Uint8Array`, `Uint8ClampedArray`, `Int16Array`, and `Uint16Array` use range-adaptive 1-pass counting sort for **9x–10.7x speedups** over V8's built-in `.sort()`.

---

## Benchmarks

Measured with `npm run bench` (`node bench/bench.mjs`) on **Node v24.7.0** (Intel Core i7-8565U @ 1.80GHz, `windows/amd64`):

### TypedArray Types (`N = 1,000,000`, Uniform)

| Type | `fastsort.maze256` | `V8 TypedArray.sort()` | Speedup |
|---|---:|---:|---:|
| `Int32Array` | **63.45 ns/el** | 110.28 ns/el | **1.74x** |
| `Uint32Array` | **59.97 ns/el** | 117.73 ns/el | **1.96x** |
| `Float32Array` | **78.21 ns/el** | 219.98 ns/el | **2.81x** |
| `Int16Array` | **10.68 ns/el** | 114.13 ns/el | **10.69x** |
| `Int8Array` | **5.56 ns/el** | 50.75 ns/el | **9.13x** |

### `Int32Array` Size Scaling (Uniform 32-bit)

| `N` | `fastsort.maze256Int32` | `V8 TypedArray.sort()` | Speedup |
|---:|---:|---:|---:|
| 1,000 | 62.67 ns/el | **47.39 ns/el** | 0.76x |
| 10,000 | **76.00 ns/el** | 117.39 ns/el | **1.54x** |
| 100,000 | **91.24 ns/el** | 147.18 ns/el | **1.61x** |
| 1,000,000 | **63.45 ns/el** | 110.28 ns/el | **1.74x** |

---

## Development

```bash
npm test    # runs node --test (0 external devDependencies)
npm run bench
```

## License

MIT — see [LICENSE](LICENSE).
