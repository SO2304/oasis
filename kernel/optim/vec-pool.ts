/**
 * OASIS Kernel — Vector Pool
 *
 * PROBLEM: Every vector operation creates a new Float64Array.
 * At 1kHz with 10,000 tensions, that's millions of allocations/sec.
 * GC pauses destroy determinism.
 *
 * SOLUTION: Pre-allocate a pool of vectors. Acquire → use → release.
 * Zero allocations in the hot path. GC never fires during control loops.
 *
 * INNOVATION: Generational pooling.
 * Vectors are grouped by "generation" (tick number). At tick end,
 * the entire previous generation is released in O(1) — no individual
 * release calls needed for temporary computation vectors.
 */

import type { Vec } from '../physics/vector-math.js';

export class VecPool {
  private readonly pools = new Map<number, Vec[]>(); // dim → free list
  private readonly inUse = new Set<Vec>();
  private readonly generational = new Map<number, Vec[]>(); // generation → vecs
  private currentGeneration = 0;

  /** Stats for monitoring */
  private allocCount = 0;
  private reuseCount = 0;
  private peakInUse = 0;

  /**
   * Pre-warm the pool with vectors of a given dimension.
   * Call this once at startup to avoid allocation during operation.
   */
  warmup(dim: number, count: number): void {
    const pool = this.pools.get(dim) ?? [];
    for (let i = 0; i < count; i++) {
      pool.push(new Float64Array(dim));
    }
    this.pools.set(dim, pool);
    this.allocCount += count;
  }

  /**
   * Acquire a zeroed vector from the pool.
   * If the pool is empty, allocates a new one (but this shouldn't
   * happen after warmup).
   */
  acquire(dim: number): Vec {
    const pool = this.pools.get(dim);
    let vec: Vec;

    if (pool && pool.length > 0) {
      vec = pool.pop()!;
      vec.fill(0);
      this.reuseCount++;
    } else {
      vec = new Float64Array(dim);
      this.allocCount++;
    }

    this.inUse.add(vec);
    if (this.inUse.size > this.peakInUse) {
      this.peakInUse = this.inUse.size;
    }

    // Track in current generation
    const gen = this.generational.get(this.currentGeneration) ?? [];
    gen.push(vec);
    this.generational.set(this.currentGeneration, gen);

    return vec;
  }

  /** Release a single vector back to the pool */
  release(vec: Vec): void {
    if (!this.inUse.has(vec)) return;
    this.inUse.delete(vec);

    const pool = this.pools.get(vec.length) ?? [];
    pool.push(vec);
    this.pools.set(vec.length, pool);
  }

  /**
   * Advance to next generation.
   * All vectors from the PREVIOUS generation are released in O(1).
   * This eliminates the need for individual release calls for
   * temporary computation vectors.
   */
  advanceGeneration(): void {
    const prevGen = this.currentGeneration - 1;
    const prevVecs = this.generational.get(prevGen);

    if (prevVecs) {
      for (const vec of prevVecs) {
        if (this.inUse.has(vec)) {
          this.inUse.delete(vec);
          const pool = this.pools.get(vec.length) ?? [];
          pool.push(vec);
          this.pools.set(vec.length, pool);
        }
      }
      this.generational.delete(prevGen);
    }

    this.currentGeneration++;
  }

  /** Pool statistics */
  getStats(): PoolStats {
    let pooledCount = 0;
    for (const pool of this.pools.values()) pooledCount += pool.length;

    return {
      totalAllocated: this.allocCount,
      totalReused: this.reuseCount,
      currentlyInUse: this.inUse.size,
      currentlyPooled: pooledCount,
      peakInUse: this.peakInUse,
      reuseRate: this.allocCount > 0
        ? this.reuseCount / (this.allocCount + this.reuseCount)
        : 0,
    };
  }
}

export interface PoolStats {
  totalAllocated: number;
  totalReused: number;
  currentlyInUse: number;
  currentlyPooled: number;
  peakInUse: number;
  reuseRate: number;
}

// ─── In-Place Vector Operations ─────────────────────────────────
// These write into existing buffers instead of allocating new ones.
// Use with pooled vectors for zero-allocation hot paths.

/** dst = a + b (writes into dst, returns dst) */
export function addInto(dst: Vec, a: Vec, b: Vec): Vec {
  for (let i = 0; i < dst.length; i++) dst[i] = a[i]! + b[i]!;
  return dst;
}

/** dst = a - b */
export function subInto(dst: Vec, a: Vec, b: Vec): Vec {
  for (let i = 0; i < dst.length; i++) dst[i] = a[i]! - b[i]!;
  return dst;
}

/** dst = v * s */
export function scaleInto(dst: Vec, v: Vec, s: number): Vec {
  for (let i = 0; i < dst.length; i++) dst[i] = v[i]! * s;
  return dst;
}

/** dst += v * s (accumulate scaled vector — the most common hot-path op) */
export function accumScaled(dst: Vec, v: Vec, s: number): Vec {
  for (let i = 0; i < dst.length; i++) dst[i]! += v[i]! * s;
  return dst;
}

/** Copy src into dst */
export function copyInto(dst: Vec, src: Vec): Vec {
  dst.set(src);
  return dst;
}

/** Zero a vector in-place */
export function zeroOut(v: Vec): Vec {
  v.fill(0);
  return v;
}

/** Dot product (no allocation, just math) */
export function dotFast(a: Vec, b: Vec): number {
  let sum = 0;
  for (let i = 0; i < a.length; i++) sum += a[i]! * b[i]!;
  return sum;
}

/** Norm squared (avoids sqrt when only comparing magnitudes) */
export function normSq(v: Vec): number {
  let sum = 0;
  for (let i = 0; i < v.length; i++) sum += v[i]! * v[i]!;
  return sum;
}
