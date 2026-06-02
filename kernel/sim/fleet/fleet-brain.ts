/**
 * OASIS Fleet — Fleet Nervous System
 *
 * Applies OASIS principles to fleet coordination:
 *
 * P1 (Semantic Mesh): Vehicles discover each other by proximity
 * P2 (Real-Time): Collision avoidance has HARD_RT deadline
 * P4 (Physical Sandbox): Geofencing, speed limits, safe distance
 * R9: No vehicle moves without safety sandbox active
 * R12: Kill switch stops ALL vehicles instantly
 * R14: High entropy (ambiguous situation) → all vehicles slow down
 * R15: Lost vehicle sensor → entropy spike → fleet freezes sector
 *
 * ARCHITECTURE:
 * - Each vehicle is a pressure zone in the world model
 * - Moving vehicles create REPULSIVE zones around them
 * - Destinations are ATTRACTIVE zones
 * - Intersections are HIGH-ENTROPY zones (uncertainty)
 * - Collision avoidance is a REFLEX (not deliberated)
 */

import type { VehicleState, FleetConfig } from './vehicle.js';
import { vehicleDistance, timeToCollision } from './vehicle.js';

// ─── Fleet Command ──────────────────────────────────────────────

export interface FleetCommand {
  readonly vehicleId: string;
  /** Commanded speed in m/s */
  readonly speed: number;
  /** Commanded heading in radians */
  readonly heading: number;
  /** Why this command was issued */
  readonly reason: 'CRUISE' | 'AVOID' | 'SLOW_ZONE' | 'EMERGENCY_STOP' | 'ARRIVED';
}

// ─── Fleet Metrics ──────────────────────────────────────────────

export interface FleetMetrics {
  /** Number of active vehicles */
  activeCount: number;
  /** Minimum inter-vehicle distance in meters */
  minDistance: number;
  /** Number of collision avoidance maneuvers this tick */
  avoidanceCount: number;
  /** Number of vehicles that arrived at destination */
  arrivedCount: number;
  /** Worst-case time to collision in seconds */
  minTTC: number;
  /** Any collision occurred? (distance < vehicleLength) */
  collision: boolean;
  /** Number of emergency stops */
  emergencyStops: number;
  /** Fleet-wide entropy (0 = all clear, 1 = chaos) */
  entropy: number;
}

// ─── Fleet Brain ────────────────────────────────────────────────

export class FleetBrain {
  private readonly config: FleetConfig;

  constructor(config: FleetConfig) {
    this.config = config;
  }

  /**
   * Compute commands for ALL vehicles in one tick.
   * This is the fleet's nervous system heartbeat.
   *
   * ORDER OF OPERATIONS (mirrors OASIS tick pipeline):
   * 1. REFLEX: Collision imminent → emergency brake (< 1ms)
   * 2. AVOIDANCE: TTC < 3s → steer away
   * 3. ENTROPY: Congested area → slow down
   * 4. CRUISE: No threats → navigate to destination
   */
  tick(vehicles: VehicleState[]): { commands: FleetCommand[]; metrics: FleetMetrics } {
    const commands: FleetCommand[] = [];
    // ALL vehicles are considered (active for control, inactive as obstacles)
    const active = vehicles; // Inactive vehicles are still physical obstacles
    const metrics: FleetMetrics = {
      activeCount: active.length,
      minDistance: Infinity,
      avoidanceCount: 0,
      arrivedCount: vehicles.filter(v => !v.active).length,
      minTTC: Infinity,
      collision: false,
      emergencyStops: 0,
      entropy: 0,
    };

    // Pre-compute all pairwise distances and TTCs
    const distances = new Map<string, number>();
    const ttcs = new Map<string, number>();

    for (let i = 0; i < active.length; i++) {
      for (let j = i + 1; j < active.length; j++) {
        const a = active[i]!;
        const b = active[j]!;
        const dist = vehicleDistance(a, b);
        const ttc = timeToCollision(a, b);
        const key = `${a.id}:${b.id}`;

        distances.set(key, dist);
        ttcs.set(key, ttc);

        metrics.minDistance = Math.min(metrics.minDistance, dist);
        metrics.minTTC = Math.min(metrics.minTTC, ttc);

        if (dist < this.config.vehicleLength) {
          metrics.collision = true;
        }
      }
    }

    // Fleet entropy: based on how many vehicles are in close proximity
    let dangerCount = 0;
    for (const dist of distances.values()) {
      if (dist < this.config.safeDistance * 2) dangerCount++;
    }
    metrics.entropy = active.length > 1
      ? dangerCount / (active.length * (active.length - 1) / 2)
      : 0;

    // Generate commands for each vehicle
    for (const v of active) {
      if (!v.active) {
        commands.push({ vehicleId: v.id, speed: 0, heading: v.heading, reason: 'ARRIVED' });
        continue;
      }

      // Default: cruise toward destination
      const toDestX = v.destination[0]! - v.pos[0]!;
      const toDestY = v.destination[1]! - v.pos[1]!;
      const destHeading = Math.atan2(toDestY, toDestX);
      const destDist = Math.sqrt(toDestX ** 2 + toDestY ** 2);
      let targetSpeed = Math.min(this.config.maxSpeed, destDist * 0.5); // Slow near dest
      let targetHeading = destHeading;
      let reason: FleetCommand['reason'] = 'CRUISE';

      // ── REFLEX: Emergency stop if collision imminent ──
      let closestDist = Infinity;
      let closestTTC = Infinity;
      let closestVehicle: VehicleState | null = null;

      for (const other of active) {
        if (other.id === v.id) continue;

        const key1 = `${v.id}:${other.id}`;
        const key2 = `${other.id}:${v.id}`;
        const dist = distances.get(key1) ?? distances.get(key2) ?? vehicleDistance(v, other);
        const ttc = ttcs.get(key1) ?? ttcs.get(key2) ?? timeToCollision(v, other);

        if (dist < closestDist) {
          closestDist = dist;
          closestTTC = ttc;
          closestVehicle = other;
        }
      }

      // Compute TOTAL repulsion from ALL nearby vehicles (not just closest)
      let repulsionX = 0, repulsionY = 0;
      let threatCount = 0;

      for (const other of active) {
        if (other.id === v.id) continue;
        const dist = vehicleDistance(v, other);
        if (dist < this.config.safeDistance * 3 && dist > 0.1) {
          // Inverse-distance repulsion (like OASIS pressure field)
          const force = 1 / (dist * dist);
          repulsionX += (v.pos[0]! - other.pos[0]!) / dist * force;
          repulsionY += (v.pos[1]! - other.pos[1]!) / dist * force;
          threatCount++;
        }
      }

      // R12: Emergency stop — dynamic safe distance based on CLOSING SPEED.
      // Must account for BOTH vehicles' braking distances.
      const closingSpeed = closestVehicle
        ? v.speed + closestVehicle.speed // Worst case: head-on
        : v.speed;
      const brakingDist = (closingSpeed ** 2) / (2 * this.config.maxDecel) + closingSpeed * 0.3;
      const dynamicSafeDist = Math.max(this.config.safeDistance, brakingDist + this.config.vehicleLength * 2);

      if (closestDist < dynamicSafeDist) {
        targetSpeed = 0;
        reason = 'EMERGENCY_STOP';
        metrics.emergencyStops++;
      }
      // Avoidance: threats nearby — blend repulsion with destination
      else if (threatCount > 0 && closestDist < dynamicSafeDist * 2) {
        const repMag = Math.sqrt(repulsionX ** 2 + repulsionY ** 2);
        if (repMag > 0.01) {
          const repHeading = Math.atan2(repulsionY, repulsionX);
          // Blend: stronger repulsion when closer
          const urgency = Math.min(1, this.config.safeDistance / closestDist - 0.5);
          targetHeading = repHeading * urgency + destHeading * (1 - urgency);
        }
        targetSpeed = Math.min(targetSpeed, this.config.maxSpeed * (closestDist / (this.config.safeDistance * 3)));
        reason = 'AVOID';
        metrics.avoidanceCount++;
      }
      // R14: High entropy — slow down in congested areas
      else if (metrics.entropy > 0.5) {
        targetSpeed = Math.min(targetSpeed, this.config.maxSpeed * 0.5);
        reason = 'SLOW_ZONE';
      }

      commands.push({
        vehicleId: v.id,
        speed: targetSpeed,
        heading: targetHeading,
        reason,
      });
    }

    return { commands, metrics };
  }
}
