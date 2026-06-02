/**
 * OASIS — Seedable PRNG (Mulberry32)
 *
 * Math.random() is NOT reproducible. Tests that use it
 * can flake. This PRNG produces identical sequences from
 * the same seed, making stochastic tests deterministic.
 *
 * Algorithm: Mulberry32 — fast, 32-bit, period 2^32.
 * NOT cryptographic. For simulation only.
 */

export class SeededRandom {
  private state: number;

  constructor(seed: number) {
    this.state = seed | 0;
  }

  /** Returns [0, 1) — drop-in replacement for Math.random() */
  next(): number {
    this.state = (this.state + 0x6D2B79F5) | 0;
    let t = Math.imul(this.state ^ (this.state >>> 15), 1 | this.state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  /** Gaussian with mean 0, stddev sigma */
  gaussian(sigma: number): number {
    const u1 = this.next() || 1e-10;
    const u2 = this.next();
    return Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2) * sigma;
  }
}
