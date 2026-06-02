/**
 * OASIS Kernel — Vector Math Tests
 *
 * Validates the mathematical foundation of the Tensorial Brain.
 */

import { describe, it, expect } from 'vitest';
import {
  vec, zeros, add, sub, scale, dot, norm, normalize,
  cosineSimilarity, distance, lerp, slerp,
  hadamard, weightedCentroid, project, reject, randomUnit,
} from '../vector-math.js';

describe('Vector Mathematics', () => {
  it('should create vectors correctly', () => {
    const v = vec(1, 2, 3);
    expect(v[0]).toBe(1);
    expect(v[1]).toBe(2);
    expect(v[2]).toBe(3);
    expect(v.length).toBe(3);
  });

  it('should add vectors element-wise', () => {
    const a = vec(1, 2, 3);
    const b = vec(4, 5, 6);
    const c = add(a, b);
    expect(c[0]).toBe(5);
    expect(c[1]).toBe(7);
    expect(c[2]).toBe(9);
  });

  it('should subtract vectors', () => {
    const r = sub(vec(5, 3, 1), vec(1, 1, 1));
    expect(r[0]).toBe(4);
    expect(r[1]).toBe(2);
    expect(r[2]).toBe(0);
  });

  it('should compute dot product', () => {
    expect(dot(vec(1, 0, 0), vec(0, 1, 0))).toBe(0); // Orthogonal
    expect(dot(vec(1, 0, 0), vec(1, 0, 0))).toBe(1); // Parallel
    expect(dot(vec(1, 2, 3), vec(4, 5, 6))).toBe(32);
  });

  it('should compute norm', () => {
    expect(norm(vec(3, 4))).toBe(5); // 3-4-5 triangle
    expect(norm(vec(1, 0, 0))).toBe(1);
    expect(norm(zeros(3))).toBe(0);
  });

  it('should normalize to unit vector', () => {
    const n = normalize(vec(3, 4));
    expect(norm(n)).toBeCloseTo(1, 10);
    expect(n[0]).toBeCloseTo(0.6, 10);
    expect(n[1]).toBeCloseTo(0.8, 10);
  });

  it('should handle zero vector normalization', () => {
    const n = normalize(zeros(3));
    expect(norm(n)).toBe(0); // Returns zero, not NaN
  });

  it('should compute cosine similarity', () => {
    expect(cosineSimilarity(vec(1, 0), vec(0, 1))).toBeCloseTo(0, 10); // Orthogonal
    expect(cosineSimilarity(vec(1, 0), vec(1, 0))).toBeCloseTo(1, 10); // Identical
    expect(cosineSimilarity(vec(1, 0), vec(-1, 0))).toBeCloseTo(-1, 10); // Opposite
  });

  it('should compute LERP correctly', () => {
    const a = vec(0, 0);
    const b = vec(10, 10);
    const mid = lerp(a, b, 0.5);
    expect(mid[0]).toBeCloseTo(5, 10);
    expect(mid[1]).toBeCloseTo(5, 10);
  });

  it('should compute SLERP on unit sphere', () => {
    const a = normalize(vec(1, 0, 0));
    const b = normalize(vec(0, 1, 0));
    const mid = slerp(a, b, 0.5);

    // SLERP midpoint should also be on the unit sphere
    expect(norm(mid)).toBeCloseTo(1, 5);
    // Should be equidistant from both
    expect(cosineSimilarity(mid, a)).toBeCloseTo(cosineSimilarity(mid, b), 5);
  });

  it('should compute Hadamard product', () => {
    const r = hadamard(vec(2, 3, 4), vec(5, 6, 7));
    expect(r[0]).toBe(10);
    expect(r[1]).toBe(18);
    expect(r[2]).toBe(28);
  });

  it('should compute weighted centroid', () => {
    const c = weightedCentroid(
      [vec(0, 0), vec(10, 10)],
      [1, 1], // Equal weights
    );
    expect(c[0]).toBeCloseTo(5, 10);
    expect(c[1]).toBeCloseTo(5, 10);
  });

  it('should project and reject correctly', () => {
    const a = vec(3, 4);
    const b = vec(1, 0); // X-axis

    const proj = project(a, b);
    const rej = reject(a, b);

    expect(proj[0]).toBeCloseTo(3, 10); // X component
    expect(proj[1]).toBeCloseTo(0, 10);
    expect(rej[0]).toBeCloseTo(0, 10);
    expect(rej[1]).toBeCloseTo(4, 10); // Y component

    // a = proj + rej
    const reconstructed = add(proj, rej);
    expect(reconstructed[0]).toBeCloseTo(3, 10);
    expect(reconstructed[1]).toBeCloseTo(4, 10);
  });

  it('should generate random unit vectors', () => {
    const v = randomUnit(128);
    expect(v.length).toBe(128);
    expect(norm(v)).toBeCloseTo(1, 5);
  });

  it('should reject dimension mismatches', () => {
    expect(() => add(vec(1, 2), vec(1, 2, 3))).toThrow('Dimension mismatch');
    expect(() => dot(vec(1), vec(1, 2))).toThrow('Dimension mismatch');
  });
});
