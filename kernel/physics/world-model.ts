/**
 * OASIS Kernel — World Model (P2)
 *
 * A NON-EUCLIDEAN model of the world.
 *
 * CORE INSIGHT: The world is not a grid of cells. It's a
 * TENSION FIELD where every entity (obstacle, goal, unknown zone)
 * is a source of semantic pressure.
 *
 * An obstacle doesn't "block" — it REPELS. A goal doesn't "attract"
 * via pathfinding — it creates a GRADIENT that agents follow.
 * Unknown regions don't "not exist" — they radiate ENTROPY.
 *
 * Navigation becomes GRADIENT DESCENT through the world's tension
 * field. The path of least resistance IS the optimal trajectory —
 * no A*, no RRT*, no explicit planning. Just physics.
 *
 * INNOVATION: Semantic Pressure Zones
 *
 * Instead of hard boundaries (wall/not-wall), the world model
 * maintains continuous pressure fields:
 * - Repulsive zones: obstacles, hazards, forbidden areas
 * - Attractive zones: goals, charging stations, safe havens
 * - Entropy zones: unmapped regions, sensor blind spots
 * - Semantic zones: "slippery floor", "fragile objects", "human presence"
 *
 * Each zone deforms the tension field, and agents navigate by
 * following the combined gradient — like water flowing downhill.
 */

import { monotonicNow } from '../types.js';
import { computeLeastResistancePath, type LeastResistancePath, type TrajectoryPoint } from './pathfinding.js';
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
  distance,
} from './vector-math.js';

// ─── Pressure Zone Types ────────────────────────────────────────

export const ZoneType = {
  /** Pushes agents away — obstacles, walls, hazards */
  REPULSIVE: 'REPULSIVE',
  /** Pulls agents toward — goals, targets, waypoints */
  ATTRACTIVE: 'ATTRACTIVE',
  /** Radiates uncertainty — unmapped, sensor blind spots */
  ENTROPY: 'ENTROPY',
  /** Modifies behavior — "slow down", "be gentle", "avoid noise" */
  SEMANTIC: 'SEMANTIC',
} as const;
export type ZoneType = (typeof ZoneType)[keyof typeof ZoneType];

export interface PressureZone {
  readonly id: string;
  readonly type: ZoneType;
  /** Center of the zone in latent space */
  readonly center: Vec;
  /** How quickly the pressure falls off with distance */
  readonly falloffRate: number;
  /** Peak intensity at center */
  readonly intensity: number;
  /** Semantic signature — what "kind" of zone this is */
  readonly signature: Vec;
  /** Optional: human-readable label */
  readonly label: string;
  /** Confidence in this zone's existence [0, 1] */
  confidence: number;
  /** Timestamp of last update */
  lastUpdatedAt: bigint;
  /** How many ticks since last perception confirmation */
  staleness: number;
}

// ─── World Sample Result ────────────────────────────────────────

export interface WorldSample {
  /** Net force at the sampled position */
  readonly netForce: Vec;
  /** Local entropy (uncertainty about this region) */
  readonly localEntropy: number;
  /** Semantic pressure magnitude (non-directional environmental info) */
  readonly semanticPressure: number;
  /** Dominant zone affecting this position */
  readonly dominantZone: PressureZone | null;
  /** All contributing zones */
  readonly contributors: Array<{ zone: PressureZone; contribution: number }>;
}

// ─── Trajectory (path of least resistance) ──────────────────────

export interface TrajectoryPoint {
  readonly position: Vec;
  readonly force: Vec;
  readonly entropy: number;
  readonly speed: number;
}

export interface LeastResistancePath {
  readonly points: TrajectoryPoint[];
  readonly totalEntropy: number;
  readonly totalResistance: number;
  readonly feasible: boolean;
  readonly reason: string;
}

// ─── World Model ────────────────────────────────────────────────

export class WorldModel {
  private readonly zones = new Map<string, PressureZone>();
  private readonly dim: number;
  private tickCount = 0;

  /** Zones decay after this many ticks without perception update */
  private readonly maxStaleness: number;
  /** Entropy radiated by unknown/stale zones */
  private readonly entropyRadiation: number;

  constructor(dim: number, maxStaleness = 30, entropyRadiation = 0.3) {
    this.dim = dim;
    this.maxStaleness = maxStaleness;
    this.entropyRadiation = entropyRadiation;
    this._sampleForce = new Float64Array(dim);
    this._sampleDir = new Float64Array(dim);
  }

  // ─── Zone Management ────────────────────────────────────────

  /**
   * Add or update a pressure zone from perception input.
   *
   * When the sensor fusion detects an obstacle, it doesn't create
   * a "wall object" — it creates a REPULSIVE ZONE in the world model.
   */
  upsertZone(zone: PressureZone): void {
    this.zones.set(zone.id, { ...zone, staleness: 0, lastUpdatedAt: monotonicNow() });
  }

  /** Remove a zone */
  removeZone(id: string): boolean {
    return this.zones.delete(id);
  }

  /** Get a zone by ID */
  getZone(id: string): PressureZone | undefined {
    return this.zones.get(id);
  }

  /** Get all zones */
  getAllZones(): PressureZone[] {
    return [...this.zones.values()];
  }

  /**
   * Create a repulsive zone from perception input.
   * Convenience method for obstacle detection.
   */
  addObstacle(
    id: string,
    center: Vec,
    intensity: number,
    signature: Vec,
    label = 'obstacle',
  ): PressureZone {
    const zone: PressureZone = {
      id,
      type: ZoneType.REPULSIVE,
      center,
      falloffRate: 2.0, // Inverse square
      intensity,
      signature,
      label,
      confidence: 1.0,
      lastUpdatedAt: monotonicNow(),
      staleness: 0,
    };
    this.upsertZone(zone);
    return zone;
  }

  /**
   * Create an attractive zone for a goal/waypoint.
   */
  addGoal(
    id: string,
    center: Vec,
    intensity: number,
    signature: Vec,
    label = 'goal',
  ): PressureZone {
    const zone: PressureZone = {
      id,
      type: ZoneType.ATTRACTIVE,
      center,
      falloffRate: 1.0, // Linear falloff — gentle pull
      intensity,
      signature,
      label,
      confidence: 1.0,
      lastUpdatedAt: monotonicNow(),
      staleness: 0,
    };
    this.upsertZone(zone);
    return zone;
  }

  /**
   * Create an entropy zone (unknown/unmapped region).
   * These increase the local entropy, triggering R14/R15.
   */
  addUnknownRegion(
    id: string,
    center: Vec,
    intensity: number,
    label = 'unknown',
  ): PressureZone {
    const zone: PressureZone = {
      id,
      type: ZoneType.ENTROPY,
      center,
      falloffRate: 1.5,
      intensity,
      signature: zeros(this.dim),
      label,
      confidence: 0.5,
      lastUpdatedAt: monotonicNow(),
      staleness: 0,
    };
    this.upsertZone(zone);
    return zone;
  }

  // ─── World Sampling ─────────────────────────────────────────

  /**
   * Sample the world model at a given position.
   *
   * Returns the NET FORCE and LOCAL ENTROPY at that position.
   * This is what the agent "feels" from the world.
   *
   * INNOVATION: Pressure fields compose through superposition.
   * Multiple obstacles create a combined repulsive field.
   * A goal behind an obstacle creates a "pressure saddle" —
   * the agent must find the path around, which emerges naturally
   * from the gradient of the combined field.
   */
  /** Pre-allocated scratch buffers for zero-alloc sampling */
  private readonly _sampleForce: Vec;
  private readonly _sampleDir: Vec;

  sample(position: Vec): WorldSample {
    // Reuse pre-allocated buffer instead of zeros()
    this._sampleForce.fill(0);

    let localEntropy = 0;
    let semanticPressure = 0;
    let dominantZone: PressureZone | null = null;
    let dominantContribution = 0;
    const contributors: Array<{ zone: PressureZone; contribution: number }> = [];

    for (const zone of this.zones.values()) {
      const dist = distance(position, zone.center);
      const effectiveConfidence = zone.confidence * Math.max(0, 1 - zone.staleness / this.maxStaleness);
      if (effectiveConfidence < 0.01) continue;

      const safeDist = Math.max(dist, 0.1);
      const fieldStrength = zone.intensity * effectiveConfidence / Math.pow(safeDist, zone.falloffRate);
      if (fieldStrength < 0.001) continue;

      // Direction away — computed in-place (single allocation reused)
      let dirNorm = 0;
      if (dist > 0.001) {
        for (let d = 0; d < this.dim; d++) {
          this._sampleDir[d] = (position[d] ?? 0) - (zone.center[d] ?? 0);
          dirNorm += this._sampleDir[d]! * this._sampleDir[d]!;
        }
        dirNorm = Math.sqrt(dirNorm);
        if (dirNorm > 0) for (let d = 0; d < this.dim; d++) this._sampleDir[d]! /= dirNorm;
      } else {
        this._sampleDir.fill(0);
      }
      const directionAway = this._sampleDir;

      // Accumulate force in-place (zero allocation)
      let forceMult = 0;
      switch (zone.type) {
        case ZoneType.REPULSIVE:  forceMult = fieldStrength; break;
        case ZoneType.ATTRACTIVE: forceMult = -fieldStrength; break;
        case ZoneType.ENTROPY:    localEntropy += fieldStrength * this.entropyRadiation; break;
        case ZoneType.SEMANTIC:   semanticPressure += fieldStrength; break;
      }
      if (forceMult !== 0) {
        for (let d = 0; d < this.dim; d++) {
          this._sampleForce[d]! += directionAway[d]! * forceMult;
        }
      }
      contributors.push({ zone, contribution: fieldStrength });

      if (fieldStrength > dominantContribution) {
        dominantContribution = fieldStrength;
        dominantZone = zone;
      }
    }

    // Clamp entropy to [0, 1]
    localEntropy = Math.min(1, localEntropy);

    // Copy accumulator to result (single allocation per sample)
    const netForce = Float64Array.from(this._sampleForce);

    return {
      netForce,
      localEntropy,
      semanticPressure,
      dominantZone,
      contributors,
    };
  }

  /** Compute path of least resistance (delegates to pathfinding.ts) */
  computeLeastResistancePath(start: Vec, goalZoneId: string, maxSteps = 200, stepSize = 0.05, entropyLimit = 0.85): LeastResistancePath {
    return computeLeastResistancePath(this, start, goalZoneId, maxSteps, stepSize, entropyLimit);
  }

  /**
   * Advance one tick — ages all zones, decays stale confidence.
   */
  tick(): void {
    this.tickCount++;
    for (const zone of this.zones.values()) {
      zone.staleness++;
      // Stale zones lose confidence
      if (zone.staleness > this.maxStaleness) {
        zone.confidence *= 0.9; // Exponential decay
      }
    }
    // Remove dead zones
    for (const [id, zone] of this.zones) {
      if (zone.confidence < 0.01) this.zones.delete(id);
    }
  }

  /** Get tick count */
  getTickCount(): number {
    return this.tickCount;
  }

  /** Get zone count */
  getZoneCount(): number {
    return this.zones.size;
  }
}
