/**
 * OASIS Kernel — Vector Mathematics
 *
 * Pure, zero-dependency vector operations for N-dimensional latent space.
 * All operations are immutable — vectors are never mutated in place.
 *
 * INNOVATION: These aren't just utility functions. They're the "instruction set"
 * of the Tensorial Brain. Every agent interaction is ultimately a vector operation.
 */

/** Immutable N-dimensional vector */
export type Vec = Float64Array;

/** Create a zero vector of dimension N */
export function zeros(n: number): Vec {
  return new Float64Array(n);
}

/** Create a vector from values */
export function vec(...values: number[]): Vec {
  return new Float64Array(values);
}

/** Create a random unit vector of dimension N (uniform on hypersphere) */
export function randomUnit(n: number): Vec {
  const v = new Float64Array(n);
  // Marsaglia method: gaussian samples normalized to unit sphere
  let sumSq = 0;
  for (let i = 0; i < n; i++) {
    // Box-Muller transform
    const u1 = Math.random();
    const u2 = Math.random();
    const g = Math.sqrt(-2 * Math.log(u1 || 1e-10)) * Math.cos(2 * Math.PI * u2);
    v[i] = g;
    sumSq += g * g;
  }
  const norm = Math.sqrt(sumSq);
  if (norm > 0) {
    for (let i = 0; i < n; i++) v[i]! /= norm;
  }
  return v;
}

/** Element-wise addition: a + b */
export function add(a: Vec, b: Vec): Vec {
  assertSameDim(a, b);
  const r = new Float64Array(a.length);
  for (let i = 0; i < a.length; i++) r[i] = a[i]! + b[i]!;
  return r;
}

/** Element-wise subtraction: a - b */
export function sub(a: Vec, b: Vec): Vec {
  assertSameDim(a, b);
  const r = new Float64Array(a.length);
  for (let i = 0; i < a.length; i++) r[i] = a[i]! - b[i]!;
  return r;
}

/** Scalar multiplication: s * v */
export function scale(v: Vec, s: number): Vec {
  const r = new Float64Array(v.length);
  for (let i = 0; i < v.length; i++) r[i] = v[i]! * s;
  return r;
}

/** Dot product: a · b */
export function dot(a: Vec, b: Vec): number {
  assertSameDim(a, b);
  let sum = 0;
  for (let i = 0; i < a.length; i++) sum += a[i]! * b[i]!;
  return sum;
}

/** Euclidean norm (L2): ||v|| */
export function norm(v: Vec): number {
  return Math.sqrt(dot(v, v));
}

/** Normalize to unit vector. Returns zero vector if input is zero. */
export function normalize(v: Vec): Vec {
  const n = norm(v);
  return n > 1e-10 ? scale(v, 1 / n) : zeros(v.length);
}

/** Cosine similarity: cos(θ) between a and b. Range [-1, 1].
 * Optimized: single pass (no separate norm calls). */
export function cosineSimilarity(a: Vec, b: Vec): number {
  let dotAB = 0, normASq = 0, normBSq = 0;
  const len = Math.min(a.length, b.length);
  for (let i = 0; i < len; i++) {
    const ai = a[i]!, bi = b[i]!;
    dotAB += ai * bi;
    normASq += ai * ai;
    normBSq += bi * bi;
  }
  const denom = Math.sqrt(normASq * normBSq);
  return denom < 1e-20 ? 0 : dotAB / denom;
}

/** Euclidean distance: ||a - b||. Single-pass, zero allocation. */
export function distance(a: Vec, b: Vec): number {
  let sumSq = 0;
  const len = Math.min(a.length, b.length);
  for (let i = 0; i < len; i++) {
    const d = a[i]! - b[i]!;
    sumSq += d * d;
  }
  return Math.sqrt(sumSq);
}

/**
 * Linear interpolation (LERP) between a and b.
 * t=0 -> a, t=1 -> b, t=0.5 -> midpoint
 */
export function lerp(a: Vec, b: Vec, t: number): Vec {
  return add(scale(a, 1 - t), scale(b, t));
}

/**
 * Spherical linear interpolation (SLERP) on the hypersphere.
 * Preserves magnitude — the interpolation follows the geodesic,
 * not a straight line through the space.
 */
export function slerp(a: Vec, b: Vec, t: number): Vec {
  const cos = cosineSimilarity(a, b);

  // If vectors are nearly parallel, fall back to LERP
  if (Math.abs(cos) > 0.9999) return lerp(a, b, t);

  const theta = Math.acos(Math.max(-1, Math.min(1, cos)));
  const sinTheta = Math.sin(theta);

  const wa = Math.sin((1 - t) * theta) / sinTheta;
  const wb = Math.sin(t * theta) / sinTheta;

  return add(scale(a, wa), scale(b, wb));
}

/**
 * Element-wise Hadamard product: a ⊙ b
 * Used for "gating" — masking dimensions of a vector.
 */
export function hadamard(a: Vec, b: Vec): Vec {
  assertSameDim(a, b);
  const r = new Float64Array(a.length);
  for (let i = 0; i < a.length; i++) r[i] = a[i]! * b[i]!;
  return r;
}

/**
 * Weighted centroid of multiple vectors.
 * The "center of mass" in latent space.
 */
export function weightedCentroid(vectors: Vec[], weights: number[]): Vec {
  if (vectors.length === 0) throw new Error('Cannot compute centroid of empty set');
  if (vectors.length !== weights.length) throw new Error('Vectors and weights must have same length');

  const dim = vectors[0]!.length;
  const result = zeros(dim);
  let totalWeight = 0;

  for (let i = 0; i < vectors.length; i++) {
    const w = weights[i]!;
    totalWeight += w;
    const v = vectors[i]!;
    for (let d = 0; d < dim; d++) result[d]! += v[d]! * w;
  }

  if (totalWeight > 0) {
    for (let d = 0; d < dim; d++) result[d]! /= totalWeight;
  }

  return result;
}

/**
 * Project vector a onto vector b.
 * Returns the component of a that lies along b.
 */
export function project(a: Vec, b: Vec): Vec {
  const bNormSq = dot(b, b);
  if (bNormSq < 1e-10) return zeros(a.length);
  return scale(b, dot(a, b) / bNormSq);
}

/**
 * Reject vector a from vector b.
 * Returns the component of a perpendicular to b.
 * a = project(a,b) + reject(a,b)
 */
export function reject(a: Vec, b: Vec): Vec {
  return sub(a, project(a, b));
}

// ─── Validation ─────────────────────────────────────────────────

function assertSameDim(a: Vec, b: Vec): void {
  if (a.length !== b.length) {
    throw new Error(`Dimension mismatch: ${a.length} vs ${b.length}`);
  }
}
