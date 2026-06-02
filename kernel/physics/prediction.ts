/**
 * OASIS Kernel — Temporal Prediction
 *
 * WEAKNESS FIXED: The world model was REACTIVE — it only showed
 * what sensors see NOW. A real system needs to predict WHERE
 * things will be in 100ms, 500ms, 1s.
 *
 * INNOVATION: Pressure Field Extrapolation
 *
 * Given the current velocity of pressure zones (computed from
 * their position history), predict the field state at time T+dt.
 *
 * This enables:
 * - Preemptive avoidance (dodge before collision)
 * - Anticipatory planning (start braking early)
 * - Proactive swarm coordination (predict where to converge)
 *
 * METHOD: Linear extrapolation of zone centers + velocity,
 * with confidence decay proportional to prediction horizon.
 * The further into the future, the less certain the prediction.
 */

import {
  type Vec,
  zeros,
  add,
  sub,
  scale,
  norm,
  normalize,
  distance,
} from './vector-math.js';
import { WorldModel, type PressureZone, ZoneType } from './world-model.js';

// ─── Zone Velocity Tracker ──────────────────────────────────────

interface ZoneTrack {
  readonly zoneId: string;
  readonly positions: Vec[];    // History of center positions
  readonly timestamps: number[]; // Tick numbers
  velocity: Vec;                 // Estimated velocity (position delta per tick)
}

// ─── Predicted Zone ─────────────────────────────────────────────

export interface PredictedZone {
  readonly zoneId: string;
  readonly type: string;
  /** Predicted center at time T+horizon */
  readonly predictedCenter: Vec;
  /** Prediction confidence [0, 1] — decays with horizon */
  readonly confidence: number;
  /** Predicted intensity (may decay if zone is fading) */
  readonly predictedIntensity: number;
  /** Time horizon in ticks */
  readonly horizonTicks: number;
}

export interface PredictionResult {
  readonly predictions: PredictedZone[];
  /** Predicted collision: will an agent position overlap a repulsive zone? */
  readonly collisionWarnings: CollisionWarning[];
  /** Horizon in ticks */
  readonly horizon: number;
}

export interface CollisionWarning {
  readonly zoneId: string;
  readonly agentPosition: Vec;
  readonly predictedZoneCenter: Vec;
  readonly timeToContact: number; // Ticks until overlap
  readonly severity: number;      // [0, 1]
}

// ─── Temporal Predictor ─────────────────────────────────────────

export class TemporalPredictor {
  private readonly tracks = new Map<string, ZoneTrack>();
  private readonly dim: number;
  private readonly historyLength: number;
  private tickCount = 0;

  constructor(dim: number, historyLength = 20) {
    this.dim = dim;
    this.historyLength = historyLength;
  }

  /**
   * Update tracking from current world model state.
   * Call this every tick to maintain velocity estimates.
   */
  observe(world: WorldModel): void {
    this.tickCount++;

    for (const zone of world.getAllZones()) {
      let track = this.tracks.get(zone.id);

      if (!track) {
        track = {
          zoneId: zone.id,
          positions: [],
          timestamps: [],
          velocity: zeros(this.dim),
        };
        this.tracks.set(zone.id, track);
      }

      // Record position
      track.positions.push(Float64Array.from(zone.center));
      track.timestamps.push(this.tickCount);

      // Trim history
      if (track.positions.length > this.historyLength) {
        track.positions.shift();
        track.timestamps.shift();
      }

      // Estimate velocity from last two positions
      if (track.positions.length >= 2) {
        const n = track.positions.length;
        const dt = track.timestamps[n - 1]! - track.timestamps[n - 2]!;
        if (dt > 0) {
          track.velocity = scale(
            sub(track.positions[n - 1]!, track.positions[n - 2]!),
            1 / dt,
          );
        }
      }
    }

    // Remove tracks for zones that no longer exist
    for (const [id] of this.tracks) {
      if (!world.getZone(id)) {
        this.tracks.delete(id);
      }
    }
  }

  /**
   * Predict the world state at T + horizonTicks.
   *
   * Each zone's center is extrapolated:
   *   predicted_center = current_center + velocity * horizonTicks
   *
   * Confidence decays exponentially with horizon:
   *   confidence = zone.confidence * exp(-horizon * decayRate)
   */
  predict(world: WorldModel, horizonTicks: number, decayRate = 0.05): PredictionResult {
    const predictions: PredictedZone[] = [];

    for (const zone of world.getAllZones()) {
      const track = this.tracks.get(zone.id);
      const velocity = track?.velocity ?? zeros(this.dim);

      // Extrapolate center
      const predictedCenter = add(zone.center, scale(velocity, horizonTicks));

      // Confidence decays with horizon
      const confidence = zone.confidence * Math.exp(-horizonTicks * decayRate);

      // Intensity may also decay if zone is already fading
      const staleFactor = Math.max(0, 1 - zone.staleness / 30);
      const predictedIntensity = zone.intensity * staleFactor;

      predictions.push({
        zoneId: zone.id,
        type: zone.type,
        predictedCenter,
        confidence,
        predictedIntensity,
        horizonTicks,
      });
    }

    return {
      predictions,
      collisionWarnings: [],
      horizon: horizonTicks,
    };
  }

  /**
   * Check if an agent moving at a given velocity will collide
   * with any predicted repulsive zone.
   *
   * Returns warnings sorted by time-to-contact (soonest first).
   */
  predictCollisions(
    world: WorldModel,
    agentPosition: Vec,
    agentVelocity: Vec,
    maxHorizon = 50,
    safeDistance = 1.0,
  ): CollisionWarning[] {
    const warnings: CollisionWarning[] = [];

    for (const zone of world.getAllZones()) {
      if (zone.type !== ZoneType.REPULSIVE) continue;

      const track = this.tracks.get(zone.id);
      const zoneVelocity = track?.velocity ?? zeros(this.dim);

      // Find time-to-contact by checking distance at each future tick
      for (let t = 1; t <= maxHorizon; t++) {
        const futureAgent = add(agentPosition, scale(agentVelocity, t));
        const futureZone = add(zone.center, scale(zoneVelocity, t));
        const dist = distance(futureAgent, futureZone);

        if (dist < safeDistance) {
          const severity = Math.min(1, safeDistance / (dist + 0.01));

          warnings.push({
            zoneId: zone.id,
            agentPosition: futureAgent,
            predictedZoneCenter: futureZone,
            timeToContact: t,
            severity,
          });
          break; // Only first contact per zone
        }
      }
    }

    return warnings.sort((a, b) => a.timeToContact - b.timeToContact);
  }

  /** Get estimated velocity of a zone */
  getZoneVelocity(zoneId: string): Vec | null {
    return this.tracks.get(zoneId)?.velocity ?? null;
  }

  /** Get number of tracked zones */
  getTrackedCount(): number {
    return this.tracks.size;
  }
}
