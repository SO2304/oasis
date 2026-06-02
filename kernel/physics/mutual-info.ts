/**
 * OASIS Kernel — Mutual Information Estimator
 *
 * WEAKNESS FIXED: Cosine similarity only captures LINEAR correlations.
 * Two agents could have a complex non-linear relationship (e.g.,
 * one's position is a square of the other's velocity) and cosine
 * similarity would miss it entirely.
 *
 * Mutual Information captures ANY statistical dependency:
 * I(X;Y) = 0 means X and Y are independent
 * I(X;Y) > 0 means knowing X reduces uncertainty about Y
 *
 * METHOD: k-Nearest Neighbor estimator (Kraskov et al., 2004)
 * - Non-parametric (no assumptions about distribution shape)
 * - Works for continuous variables
 * - O(n * k * d) where n=samples, k=neighbors, d=dims
 *
 * Simplified implementation: histogram-based MI for speed.
 * We bin each dimension and compute joint/marginal entropies.
 */

import type { Vec } from './vector-math.js';

/**
 * Estimate mutual information between two trajectory sequences.
 *
 * Each sequence is a list of Vec (agent positions over time).
 * MI is computed per-dimension then summed.
 *
 * Returns [0, +inf): 0 = independent, higher = more dependency.
 * Normalized to [0, 1] via MI / max(H(X), H(Y)).
 *
 * @param seqA Trajectory of agent A (list of position vectors)
 * @param seqB Trajectory of agent B (list of position vectors)
 * @param bins Number of histogram bins (more = finer resolution)
 */
export function estimateMutualInformation(
  seqA: Vec[],
  seqB: Vec[],
  bins = 10,
): MutualInformationResult {
  if (seqA.length < 4 || seqB.length < 4) {
    return { mi: 0, normalizedMI: 0, hX: 0, hY: 0, hXY: 0, sampleCount: 0 };
  }

  const n = Math.min(seqA.length, seqB.length);
  const dim = Math.min(seqA[0]!.length, seqB[0]!.length);

  // Compute MI per dimension then average
  let totalMI = 0;
  let totalHX = 0;
  let totalHY = 0;
  let dimsUsed = 0;

  for (let d = 0; d < dim; d++) {
    // Extract 1D series for this dimension
    const xVals: number[] = [];
    const yVals: number[] = [];

    for (let i = 0; i < n; i++) {
      xVals.push(seqA[i]![d] ?? 0);
      yVals.push(seqB[i]![d] ?? 0);
    }

    // Skip constant dimensions (zero variance = zero MI)
    const xRange = range(xVals);
    const yRange = range(yVals);
    if (xRange < 1e-10 || yRange < 1e-10) continue;

    dimsUsed++;

    // Bin the values
    const xBinned = binValues(xVals, bins);
    const yBinned = binValues(yVals, bins);

    // Compute joint and marginal histograms
    const jointHist = new Map<string, number>();
    const xHist = new Map<number, number>();
    const yHist = new Map<number, number>();

    for (let i = 0; i < n; i++) {
      const xb = xBinned[i]!;
      const yb = yBinned[i]!;
      const key = `${xb},${yb}`;

      jointHist.set(key, (jointHist.get(key) ?? 0) + 1);
      xHist.set(xb, (xHist.get(xb) ?? 0) + 1);
      yHist.set(yb, (yHist.get(yb) ?? 0) + 1);
    }

    // Compute entropies
    const hX = entropy(xHist, n);
    const hY = entropy(yHist, n);
    const hXY = entropy(jointHist, n);

    // MI = H(X) + H(Y) - H(X,Y)
    const mi = Math.max(0, hX + hY - hXY);

    totalMI += mi;
    totalHX += hX;
    totalHY += hY;
  }

  if (dimsUsed === 0) {
    return { mi: 0, normalizedMI: 0, hX: 0, hY: 0, hXY: 0, sampleCount: n };
  }

  const avgMI = totalMI / dimsUsed;
  const avgHX = totalHX / dimsUsed;
  const avgHY = totalHY / dimsUsed;
  const maxH = Math.max(avgHX, avgHY);
  const normalizedMI = maxH > 0 ? avgMI / maxH : 0;

  return {
    mi: avgMI,
    normalizedMI: Math.min(1, normalizedMI),
    hX: avgHX,
    hY: avgHY,
    hXY: avgHX + avgHY - avgMI,
    sampleCount: n,
  };
}

/**
 * Detect non-linear correlations between multiple agents.
 * Returns all pairs with MI above threshold.
 */
export function detectNonLinearCorrelations(
  trajectories: Map<string, Vec[]>,
  threshold = 0.3,
  bins = 10,
): NonLinearCorrelation[] {
  const correlations: NonLinearCorrelation[] = [];
  const agents = [...trajectories.entries()];

  for (let i = 0; i < agents.length; i++) {
    for (let j = i + 1; j < agents.length; j++) {
      const [idA, seqA] = agents[i]!;
      const [idB, seqB] = agents[j]!;

      const result = estimateMutualInformation(seqA, seqB, bins);

      if (result.normalizedMI >= threshold) {
        correlations.push({
          agentA: idA,
          agentB: idB,
          mutualInformation: result.mi,
          normalizedMI: result.normalizedMI,
          sampleCount: result.sampleCount,
          isNonLinear: true, // MI captures what cosine can't
        });
      }
    }
  }

  return correlations.sort((a, b) => b.normalizedMI - a.normalizedMI);
}

// ─── Types ──────────────────────────────────────────────────────

export interface MutualInformationResult {
  /** Raw mutual information (nats) */
  readonly mi: number;
  /** Normalized MI [0, 1] */
  readonly normalizedMI: number;
  /** Marginal entropy of X */
  readonly hX: number;
  /** Marginal entropy of Y */
  readonly hY: number;
  /** Joint entropy */
  readonly hXY: number;
  /** Number of samples used */
  readonly sampleCount: number;
}

export interface NonLinearCorrelation {
  readonly agentA: string;
  readonly agentB: string;
  readonly mutualInformation: number;
  readonly normalizedMI: number;
  readonly sampleCount: number;
  readonly isNonLinear: boolean;
}

// ─── Helpers ────────────────────────────────────────────────────

function range(values: number[]): number {
  let min = Infinity, max = -Infinity;
  for (const v of values) {
    if (v < min) min = v;
    if (v > max) max = v;
  }
  return max - min;
}

function binValues(values: number[], bins: number): number[] {
  let min = Infinity, max = -Infinity;
  for (const v of values) {
    if (v < min) min = v;
    if (v > max) max = v;
  }
  const spread = max - min || 1;

  return values.map(v => {
    const bin = Math.floor(((v - min) / spread) * (bins - 1));
    return Math.min(bin, bins - 1);
  });
}

function entropy(hist: Map<string | number, number>, total: number): number {
  let h = 0;
  for (const count of hist.values()) {
    const p = count / total;
    if (p > 0) h -= p * Math.log(p);
  }
  return h;
}
