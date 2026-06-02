/**
 * OASIS Kernel — Stigmergy Engine
 *
 * HOW ANTS COMMUNICATE WITHOUT TALKING:
 * Ants don't send messages to each other. They leave pheromone
 * trails in the environment. Other ants sense those trails and
 * follow them. The environment IS the communication channel.
 *
 * OASIS agents do the same thing — but in LATENT SPACE.
 *
 * A "pheromone" is a persistent tension that an agent deposits
 * in the TensionField at its current position. It says:
 * "I was here, moving in this direction, with this intent."
 *
 * Other agents sense the pheromone gradient and are influenced
 * WITHOUT any direct communication (R2: no direct agent calls).
 *
 * INNOVATION: Semantic Pheromones
 * Unlike ant pheromones (which are just "food this way"),
 * OASIS pheromones carry semantic information:
 * - ATTRACT: "come here, good path"
 * - REPEL: "danger, stay away"
 * - RECRUIT: "I need help here"
 * - CLAIM: "I'm handling this, don't duplicate"
 *
 * Pheromones DECAY over time (like real pheromones evaporating).
 * Heavily-trafficked paths get reinforced (positive feedback).
 * Abandoned paths fade (negative feedback).
 * This creates emergent highways through latent space.
 */

import type { AgentId, TenantId } from '../types.js';
import { monotonicNow } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  scale,
  normalize,
  norm,
  cosineSimilarity,
  distance,
} from '../physics/vector-math.js';
import type { TensionField } from '../physics/tension-field.js';

// ─── Pheromone Types ────────────────────────────────────────────

export const PheromoneType = {
  /** "Good path, come this way" */
  ATTRACT: 'ATTRACT',
  /** "Danger/dead-end, avoid" */
  REPEL: 'REPEL',
  /** "I need help, come to me" */
  RECRUIT: 'RECRUIT',
  /** "I'm handling this task, don't duplicate" */
  CLAIM: 'CLAIM',
} as const;
export type PheromoneType = (typeof PheromoneType)[keyof typeof PheromoneType];

export interface Pheromone {
  readonly id: string;
  readonly type: PheromoneType;
  readonly depositorId: AgentId;
  readonly position: Vec;
  readonly direction: Vec;
  /** Initial intensity — decays over time */
  intensity: number;
  /** How fast this pheromone evaporates (per tick) */
  readonly decayRate: number;
  /** Tick when deposited */
  readonly depositedAt: number;
  /** Reinforcement count (how many agents added to this trail) */
  reinforcements: number;
}

// ─── Trail (sequence of pheromones forming a path) ──────────────

export interface Trail {
  readonly id: string;
  readonly pheromones: Pheromone[];
  /** Net direction of the trail */
  readonly direction: Vec;
  /** Total intensity (sum of pheromone intensities) */
  readonly totalIntensity: number;
  /** How many unique agents contributed */
  readonly contributorCount: number;
}

// ─── Stigmergy Engine ───────────────────────────────────────────

export class StigmergyEngine {
  private readonly pheromones: Pheromone[] = [];
  private readonly dim: number;
  private tickCount = 0;
  private pheromoneCounter = 0;

  /** Max pheromones before eviction of weakest */
  private readonly maxPheromones: number;
  /** Distance threshold for pheromone merging */
  private readonly mergeRadius: number;
  /** Minimum intensity before evaporation */
  private readonly evaporationFloor: number;

  constructor(
    dim: number,
    maxPheromones = 5000,
    mergeRadius = 0.3,
    evaporationFloor = 0.01,
  ) {
    this.dim = dim;
    this.maxPheromones = maxPheromones;
    this.mergeRadius = mergeRadius;
    this.evaporationFloor = evaporationFloor;
  }

  /**
   * Deposit a pheromone at the agent's current position.
   *
   * If a similar pheromone already exists nearby (same type,
   * similar direction), REINFORCE it instead of creating a new one.
   * This is how ant trails get stronger with traffic.
   */
  deposit(
    agentId: AgentId,
    type: PheromoneType,
    position: Vec,
    direction: Vec,
    intensity = 1.0,
    decayRate = 0.05,
  ): Pheromone {
    // Check for nearby pheromone to reinforce
    const existing = this.findNearby(position, type, direction);

    if (existing) {
      // REINFORCE: add to existing pheromone
      existing.intensity += intensity * 0.5; // Diminishing returns
      existing.reinforcements++;
      return existing;
    }

    // Evict weakest if at capacity
    if (this.pheromones.length >= this.maxPheromones) {
      let weakestIdx = 0;
      let weakestIntensity = Infinity;
      for (let i = 0; i < this.pheromones.length; i++) {
        if (this.pheromones[i]!.intensity < weakestIntensity) {
          weakestIntensity = this.pheromones[i]!.intensity;
          weakestIdx = i;
        }
      }
      this.pheromones.splice(weakestIdx, 1);
    }

    // Create new pheromone
    const pheromone: Pheromone = {
      id: `ph-${++this.pheromoneCounter}`,
      type,
      depositorId: agentId,
      position: Float64Array.from(position),
      direction: norm(direction) > 0 ? normalize(direction) : zeros(this.dim),
      intensity,
      decayRate,
      depositedAt: this.tickCount,
      reinforcements: 0,
    };

    this.pheromones.push(pheromone);
    return pheromone;
  }

  /**
   * Sense the pheromone gradient at a position.
   *
   * Returns the combined force that pheromones exert,
   * weighted by distance and intensity.
   */
  sense(position: Vec, tenantFilter?: TenantId): PheromoneGradient {
    let attractForce = zeros(this.dim);
    let repelForce = zeros(this.dim);
    let recruitForce = zeros(this.dim);
    let claimCount = 0;
    let totalIntensity = 0;

    for (const ph of this.pheromones) {
      if (ph.intensity < this.evaporationFloor) continue;

      const dist = distance(position, ph.position);
      if (dist > 5) continue; // Beyond sensing range

      // Inverse distance weighting (closer = stronger)
      const weight = ph.intensity / (1 + dist * dist);

      switch (ph.type) {
        case PheromoneType.ATTRACT:
          // Pull toward the pheromone's direction
          attractForce = add(attractForce, scale(ph.direction, weight));
          break;
        case PheromoneType.REPEL:
          // Push away from this position
          if (dist > 0.01) {
            const away = normalize(
              Float64Array.from(position).map((v, i) => v - (ph.position[i] ?? 0)) as Vec,
            );
            repelForce = add(repelForce, scale(away, weight));
          }
          break;
        case PheromoneType.RECRUIT:
          // Pull toward the recruitment point
          if (dist > 0.01) {
            const toward = normalize(
              Float64Array.from(ph.position).map((v, i) => v - (position[i] ?? 0)) as Vec,
            );
            recruitForce = add(recruitForce, scale(toward, weight));
          }
          break;
        case PheromoneType.CLAIM:
          claimCount++;
          break;
      }

      totalIntensity += weight;
    }

    return {
      attractForce,
      repelForce,
      recruitForce,
      claimCount,
      totalIntensity,
      sensedCount: this.pheromones.filter(p => p.intensity >= this.evaporationFloor).length,
    };
  }

  /**
   * Detect emergent trails — sequences of aligned pheromones
   * that form a path through latent space.
   */
  detectTrails(minLength = 3, alignmentThreshold = 0.5): Trail[] {
    const trails: Trail[] = [];
    const used = new Set<string>();
    const attractPheromones = this.pheromones.filter(
      p => p.type === PheromoneType.ATTRACT && p.intensity >= this.evaporationFloor,
    );

    for (const start of attractPheromones) {
      if (used.has(start.id)) continue;

      // Follow the trail by finding aligned neighbors
      const trail: Pheromone[] = [start];
      used.add(start.id);
      let current = start;

      for (let step = 0; step < 50; step++) {
        let bestNext: Pheromone | null = null;
        let bestScore = -Infinity;

        for (const candidate of attractPheromones) {
          if (used.has(candidate.id)) continue;

          const dist = distance(current.position, candidate.position);
          if (dist > 1 || dist < 0.01) continue;

          const alignment = cosineSimilarity(current.direction, candidate.direction);
          if (alignment < alignmentThreshold) continue;

          const score = alignment * candidate.intensity / (1 + dist);
          if (score > bestScore) {
            bestScore = score;
            bestNext = candidate;
          }
        }

        if (!bestNext) break;
        trail.push(bestNext);
        used.add(bestNext.id);
        current = bestNext;
      }

      if (trail.length >= minLength) {
        const contributors = new Set(trail.map(p => p.depositorId));
        const directions = trail.map(p => p.direction);
        let netDir = zeros(this.dim);
        for (const d of directions) netDir = add(netDir, d);
        const totalIntensity = trail.reduce((s, p) => s + p.intensity, 0);

        trails.push({
          id: `trail-${trails.length}`,
          pheromones: trail,
          direction: norm(netDir) > 0 ? normalize(netDir) : netDir,
          totalIntensity,
          contributorCount: contributors.size,
        });
      }
    }

    return trails;
  }

  /**
   * Advance one tick — evaporate pheromones.
   * Reinforced pheromones evaporate slower (more traffic = more persistent).
   */
  tick(): void {
    this.tickCount++;

    for (let i = this.pheromones.length - 1; i >= 0; i--) {
      const ph = this.pheromones[i]!;
      // Reinforced pheromones decay slower
      const effectiveDecay = ph.decayRate / (1 + ph.reinforcements * 0.2);
      ph.intensity -= effectiveDecay;

      if (ph.intensity < this.evaporationFloor) {
        this.pheromones.splice(i, 1);
      }
    }
  }

  /** Active pheromone count */
  getActiveCount(): number {
    return this.pheromones.filter(p => p.intensity >= this.evaporationFloor).length;
  }

  getTickCount(): number {
    return this.tickCount;
  }

  // ─── Private ────────────────────────────────────────────────

  private findNearby(position: Vec, type: PheromoneType, direction: Vec): Pheromone | null {
    for (const ph of this.pheromones) {
      if (ph.type !== type) continue;
      if (distance(position, ph.position) > this.mergeRadius) continue;
      if (norm(direction) > 0 && norm(ph.direction) > 0) {
        if (cosineSimilarity(direction, ph.direction) < 0.7) continue;
      }
      return ph;
    }
    return null;
  }
}

export interface PheromoneGradient {
  readonly attractForce: Vec;
  readonly repelForce: Vec;
  readonly recruitForce: Vec;
  readonly claimCount: number;
  readonly totalIntensity: number;
  readonly sensedCount: number;
}
