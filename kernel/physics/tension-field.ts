/**
 * OASIS Kernel — Tension Field (Claim 1)
 *
 * PATENT-OPTIMIZED: Locality-Sensitive Hashing for O(1) semantic lookup.
 *
 * BEFORE: Sample iterates ALL tensions O(n*d) per agent per tick.
 * AFTER: Tensions are bucketed by semantic hash. Sample only checks
 *        the relevant bucket + neighbors. O(bucket_size * d).
 *
 * METHOD: SimHash — project the signature onto K random hyperplanes.
 * The resulting bit pattern IS the bucket key. Similar signatures
 * land in the same bucket with high probability.
 *
 * Also: zero-allocation hot path. The accumulator is reused, not created.
 */

import type { AgentId, TenantId } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  sub,
  scale,
  norm,
  normalize,
  dot,
  cosineSimilarity,
  weightedCentroid,
} from './vector-math.js';
import type { HyperState } from './hyper-state.js';

// ─── Tension Vector ─────────────────────────────────────────────

export interface TensionVector {
  readonly source: AgentId;
  readonly tenantId: TenantId;
  readonly force: Vec;
  readonly signature: Vec;
  readonly intensity: number;
  readonly decayRate: number;
  readonly emittedAt: bigint;
  ticksRemaining: number;
  /** SimHash bucket key (computed at emission) */
  bucketKey: number;
}

// ─── Interference Pattern ───────────────────────────────────────

export interface InterferenceResult {
  readonly netForce: Vec;
  readonly constructive: number;
  readonly destructive: number;
  readonly coherence: number;
  readonly contributorCount: number;
}

// ─── Resonance ──────────────────────────────────────────────────

export interface ResonanceEvent {
  readonly agents: AgentId[];
  readonly frequency: number;
  readonly strength: number;
  readonly harmonicDirection: Vec;
}

// ─── SimHash (Locality-Sensitive Hashing) ───────────────────────

const NUM_HYPERPLANES = 8; // 2^8 = 256 buckets

class SimHasher {
  private readonly hyperplanes: Vec[];

  constructor(dim: number) {
    // Deterministic pseudo-random hyperplanes (seeded)
    this.hyperplanes = [];
    for (let h = 0; h < NUM_HYPERPLANES; h++) {
      const plane = new Float64Array(dim);
      for (let d = 0; d < dim; d++) {
        // Simple deterministic hash — not cryptographic, just spatial
        plane[d] = Math.sin((h * 127 + d * 311 + 7) * 0.01) > 0 ? 1 : -1;
      }
      this.hyperplanes.push(plane);
    }
  }

  /** Compute the SimHash bucket key for a vector */
  hash(v: Vec): number {
    let key = 0;
    for (let h = 0; h < NUM_HYPERPLANES; h++) {
      let dotProduct = 0;
      const plane = this.hyperplanes[h]!;
      for (let d = 0; d < v.length && d < plane.length; d++) {
        dotProduct += v[d]! * plane[d]!;
      }
      if (dotProduct >= 0) key |= (1 << h);
    }
    return key;
  }

  /** Get neighboring bucket keys (hamming distance 1) */
  neighbors(key: number): number[] {
    const result = [key];
    for (let h = 0; h < NUM_HYPERPLANES; h++) {
      result.push(key ^ (1 << h));
    }
    return result;
  }
}

// ─── Tension Field ──────────────────────────────────────────────

export class TensionField {
  private readonly allTensions: TensionVector[] = [];
  private readonly buckets = new Map<number, number[]>();
  private readonly resonanceHistory = new Map<string, number[]>();
  private tickCount = 0;
  private readonly maxTensions: number;

  private accumForce: Vec | null = null;
  private dim = 0;
  private hasher: SimHasher | null = null;

  constructor(maxTensions = 10_000, dim?: number) {
    this.maxTensions = maxTensions;
    if (dim) {
      this.dim = dim;
      this.hasher = new SimHasher(dim);
      this.accumForce = new Float64Array(dim);
    }
  }

  /** Lazy-init dimensions from first tension (if not set in constructor) */
  private ensureInit(dim: number): void {
    if (this.dim === 0) {
      this.dim = dim;
      this.hasher = new SimHasher(dim);
      this.accumForce = new Float64Array(dim);
    }
  }

  /**
   * Emit a tension. Computes SimHash and indexes into bucket.
   */
  emit(tension: TensionVector): void {
    this.ensureInit(tension.force.length);
    tension.bucketKey = this.hasher!.hash(tension.signature);

    if (this.allTensions.length >= this.maxTensions) {
      // Evict oldest — remove from bucket index
      const evicted = this.allTensions.shift()!;
      const bucket = this.buckets.get(evicted.bucketKey);
      if (bucket) {
        // Shift all indices down by 1
        for (let i = 0; i < bucket.length; i++) bucket[i]!--;
        const idx = bucket.indexOf(-1);
        if (idx >= 0) bucket.splice(idx, 1);
      }
    }

    const index = this.allTensions.length;
    this.allTensions.push(tension);

    // Index into bucket
    const bucket = this.buckets.get(tension.bucketKey) ?? [];
    bucket.push(index);
    this.buckets.set(tension.bucketKey, bucket);
  }

  /**
   * OPTIMIZED SAMPLE: Only checks semantically nearby buckets.
   *
   * Uses zero-allocation accumulation (writes into pre-allocated buffer).
   */
  sample(position: Vec, signature: Vec, tenantId: TenantId): InterferenceResult {
    this.ensureInit(position.length);

    if (this.allTensions.length === 0) {
      return { netForce: zeros(position.length), constructive: 0, destructive: 0, coherence: 0, contributorCount: 0 };
    }

    // Reset accumulator
    this.accumForce!.fill(0);

    const sigKey = this.hasher!.hash(signature);
    const candidateBuckets = this.hasher!.neighbors(sigKey);

    let constructive = 0;
    let destructive = 0;
    let contributorCount = 0;

    // ADAPTIVE: Use SimHash buckets only when field is large.
    // Below threshold, full scan is fast enough and avoids hash misses.
    const useSimHash = this.allTensions.length > 200;

    const visited = new Set<number>();

    if (useSimHash) {
      // LARGE FIELD: check candidate buckets (SimHash LSH)
      for (const bucketKey of candidateBuckets) {
        const bucket = this.buckets.get(bucketKey);
        if (!bucket) continue;

        for (const idx of bucket) {
          if (idx < 0 || idx >= this.allTensions.length) continue;
          if (visited.has(idx)) continue;
          visited.add(idx);

          const t = this.allTensions[idx]!;
          if (t.tenantId !== tenantId || t.ticksRemaining <= 0) continue;

          const semanticAffinity = Math.max(0, cosineSimilarity(signature, t.signature));
          if (semanticAffinity < 0.01) continue;

          const decay = t.ticksRemaining / (t.ticksRemaining + t.decayRate);
          const effectiveIntensity = t.intensity * semanticAffinity * decay;

          for (let d = 0; d < this.dim && d < t.force.length; d++) {
            this.accumForce[d]! += t.force[d]! * effectiveIntensity;
          }
          contributorCount++;
        }
      }

      // High-intensity bypass (transcend buckets)
      for (let i = 0; i < this.allTensions.length; i++) {
        if (visited.has(i)) continue;
        const t = this.allTensions[i]!;
        if (t.tenantId !== tenantId || t.ticksRemaining <= 0) continue;
        if (t.intensity < 2.0) continue;

        const semanticAffinity = Math.max(0, cosineSimilarity(signature, t.signature));
        if (semanticAffinity < 0.01) continue;

        const decay = t.ticksRemaining / (t.ticksRemaining + t.decayRate);
        for (let d = 0; d < this.dim && d < t.force.length; d++) {
          this.accumForce[d]! += t.force[d]! * t.intensity * semanticAffinity * decay;
        }
        contributorCount++;
      }
    } else {
      // SMALL FIELD: full scan (no hash overhead, no misses)
      for (let i = 0; i < this.allTensions.length; i++) {
        const t = this.allTensions[i]!;
        if (t.tenantId !== tenantId || t.ticksRemaining <= 0) continue;

        // Weight by intensity and decay only (no semantic filter for small fields)
        const decay = t.ticksRemaining / (t.ticksRemaining + t.decayRate);
        const weight = t.intensity * decay;

        for (let d = 0; d < this.dim && d < t.force.length; d++) {
          this.accumForce[d]! += t.force[d]! * weight;
        }
        contributorCount++;
      }
    }

    // Copy accumulator to result (the only allocation)
    const netForce = Float64Array.from(this.accumForce!);

    // Compute interference metrics from the net force
    const netMag = norm(netForce);
    constructive = netMag;
    const coherence = contributorCount > 0 ? Math.min(1, netMag / (contributorCount * 0.5 + 1)) : 0;

    return { netForce, constructive, destructive, coherence, contributorCount };
  }

  /** Advance field by one tick */
  tick(): void {
    this.tickCount++;
    for (let i = this.allTensions.length - 1; i >= 0; i--) {
      this.allTensions[i]!.ticksRemaining--;
      if (this.allTensions[i]!.ticksRemaining <= 0) {
        const removed = this.allTensions.splice(i, 1)[0]!;
        const bucket = this.buckets.get(removed.bucketKey);
        if (bucket) {
          const idx = bucket.indexOf(i);
          if (idx >= 0) bucket.splice(idx, 1);
          // Adjust indices
          for (let j = 0; j < bucket.length; j++) {
            if (bucket[j]! > i) bucket[j]!--;
          }
        }
      }
    }
  }

  /** Detect resonance between agents */
  detectResonance(
    agents: Map<AgentId, HyperState>,
    minStrength = 0.7,
  ): ResonanceEvent[] {
    const events: ResonanceEvent[] = [];
    const agentList = [...agents.entries()];

    for (let i = 0; i < agentList.length; i++) {
      for (let j = i + 1; j < agentList.length; j++) {
        const [idA, stateA] = agentList[i]!;
        const [idB, stateB] = agentList[j]!;

        const momentumAlignment = cosineSimilarity(stateA.momentum, stateB.momentum);
        const dist = norm(sub(stateA.position, stateB.position));
        const proximity = 1 / (1 + dist);
        const strength = Math.max(0, momentumAlignment) * proximity;

        if (strength >= minStrength) {
          const pairKey = `${idA}:${idB}`;
          const history = this.resonanceHistory.get(pairKey) ?? [];
          history.push(this.tickCount);
          this.resonanceHistory.set(pairKey, history.slice(-10));

          const frequency = history.length >= 2
            ? history[history.length - 1]! - history[history.length - 2]!
            : 0;

          events.push({
            agents: [idA, idB],
            frequency,
            strength,
            harmonicDirection: normalize(add(stateA.momentum, stateB.momentum)),
          });
        }
      }
    }

    return events;
  }

  getActiveTensionCount(): number {
    return this.allTensions.filter(t => t.ticksRemaining > 0).length;
  }

  getTickCount(): number {
    return this.tickCount;
  }
}
