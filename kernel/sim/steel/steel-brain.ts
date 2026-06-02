/**
 * OASIS Steel Mill — Nervous System
 *
 * HIERARCHY OF SAFETY (non-negotiable):
 * 1. HUMAN life > everything
 * 2. Zone RED = absolute exclusion
 * 3. Crane overhead = clear the floor
 * 4. Temperature exposure = time-limited
 * 5. Production efficiency = last priority
 *
 * INNOVATION: Phase-aware zone reclassification.
 * The nervous system KNOWS the production cycle.
 * A zone that is GREEN during IDLE becomes RED during MELT.
 * The safety rules adapt to the CURRENT industrial process,
 * not to a static floor plan.
 */

import type { MillState, SteelRobot, SteelConfig, Zone } from './steel-env.js';
import { getZoneAt, distanceToZone } from './steel-env.js';

export interface SteelCommand {
  readonly robotId: string;
  readonly action: 'WORK' | 'EVACUATE' | 'HOLD' | 'SLOW' | 'DISABLED';
  readonly speedLimit: number;
  readonly reason: string;
  readonly alertLevel: 'SAFE' | 'CAUTION' | 'DANGER' | 'CRITICAL';
}

export interface SteelMetrics {
  phase: string;
  redZoneViolations: number;
  craneZoneViolations: number;
  humanProximityEvents: number;
  overheatedRobots: number;
  yellowExposureWarnings: number;
  allRobotsSafe: boolean;
}

export class SteelBrain {
  private readonly config: SteelConfig;

  constructor(config: SteelConfig) {
    this.config = config;
  }

  tick(mill: MillState, robots: SteelRobot[], dt: number): { commands: SteelCommand[]; metrics: SteelMetrics } {
    const commands: SteelCommand[] = [];
    let redViolations = 0, craneViolations = 0, humanEvents = 0;
    let overheated = 0, yellowWarnings = 0;

    for (const robot of robots) {
      if (!robot.alive) {
        commands.push({ robotId: robot.id, action: 'DISABLED', speedLimit: 0,
          reason: 'Robot disabled', alertLevel: 'SAFE' });
        continue;
      }

      const zone = getZoneAt(robot.pos, mill.zones);
      if (zone) robot.currentZone = zone.id;

      // ── Update robot thermal state ────────────────
      if (zone) {
        // Robot heats up in hot zones, cools down in cold zones
        const ambientTemp = zone.temperature;
        robot.internalTemp += (ambientTemp - robot.internalTemp) * 0.01 * dt;
      }

      // ── REFLEX 1: RED ZONE = IMMEDIATE EVACUATION ─
      if (zone && (zone.type === 'RED' || zone.moltenMetal)) {
        redViolations++;
        commands.push({ robotId: robot.id, action: 'EVACUATE', speedLimit: this.config.robotSpeed,
          reason: `RED ZONE: ${zone.id} — molten metal/active process`, alertLevel: 'CRITICAL' });
        continue;
      }

      // ── REFLEX 2: CRANE OVERHEAD = CLEAR FLOOR ────
      const distToCrane = Math.sqrt(
        (robot.pos[0]! - mill.cranePos[0]!) ** 2 +
        (robot.pos[1]! - mill.cranePos[1]!) ** 2,
      );
      if (mill.craneMoving && distToCrane < this.config.craneClearRadius) {
        craneViolations++;
        commands.push({ robotId: robot.id, action: 'EVACUATE', speedLimit: this.config.robotSpeed,
          reason: `CRANE: ${mill.craneLoadTons}t overhead at ${distToCrane.toFixed(0)}m — CLEAR`, alertLevel: 'CRITICAL' });
        continue;
      }

      // ── REFLEX 3: HUMAN NEARBY ────────────────────
      if (robot.humansNearby > 0) {
        humanEvents++;
        commands.push({ robotId: robot.id, action: 'SLOW', speedLimit: 0.5,
          reason: `Human within ${this.config.humanSafeDistance}m — speed limited (R9)`, alertLevel: 'CAUTION' });
        continue;
      }

      // ── THERMAL CHECK ─────────────────────────────
      if (robot.internalTemp > this.config.maxRobotTemp) {
        overheated++;
        commands.push({ robotId: robot.id, action: 'EVACUATE', speedLimit: this.config.robotSpeed,
          reason: `OVERHEAT: ${robot.internalTemp.toFixed(0)}°C > ${this.config.maxRobotTemp}°C`, alertLevel: 'DANGER' });
        continue;
      }

      // ── YELLOW ZONE EXPOSURE ──────────────────────
      if (zone && zone.type === 'YELLOW') {
        robot.yellowExposureS += dt;
        if (robot.yellowExposureS > this.config.maxYellowExposure) {
          yellowWarnings++;
          commands.push({ robotId: robot.id, action: 'EVACUATE', speedLimit: this.config.robotSpeed,
            reason: `YELLOW exposure: ${robot.yellowExposureS.toFixed(0)}s > ${this.config.maxYellowExposure}s`, alertLevel: 'DANGER' });
          continue;
        }
      } else {
        // Reset exposure when leaving yellow zone
        robot.yellowExposureS = Math.max(0, robot.yellowExposureS - dt * 0.5); // Cool down slowly
      }

      // ── PROXIMITY TO RED ZONES ────────────────────
      let nearestRedDist = Infinity;
      for (const z of mill.zones) {
        if (z.type === 'RED' || z.moltenMetal) {
          const d = distanceToZone(robot.pos, z);
          nearestRedDist = Math.min(nearestRedDist, d);
        }
      }
      if (nearestRedDist < this.config.meltSafeDistance) {
        commands.push({ robotId: robot.id, action: 'HOLD', speedLimit: 0,
          reason: `Too close to RED zone: ${nearestRedDist.toFixed(0)}m < ${this.config.meltSafeDistance}m`, alertLevel: 'CAUTION' });
        continue;
      }

      // ── NORMAL OPERATIONS ─────────────────────────
      commands.push({ robotId: robot.id, action: 'WORK', speedLimit: this.config.robotSpeed,
        reason: `Normal — zone: ${robot.currentZone}, phase: ${mill.currentPhase}`, alertLevel: 'SAFE' });
    }

    return {
      commands,
      metrics: {
        phase: mill.currentPhase,
        redZoneViolations: redViolations,
        craneZoneViolations: craneViolations,
        humanProximityEvents: humanEvents,
        overheatedRobots: overheated,
        yellowExposureWarnings: yellowWarnings,
        allRobotsSafe: redViolations === 0 && craneViolations === 0 && overheated === 0,
      },
    };
  }
}
