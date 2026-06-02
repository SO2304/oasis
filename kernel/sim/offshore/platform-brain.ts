/**
 * OASIS Offshore — Platform Nervous System
 *
 * UNIQUE CHALLENGE: The floor moves.
 * Unlike mines (static tunnels) or steel mills (static zones),
 * an offshore platform is a DYNAMIC environment where the
 * danger level changes with the WEATHER, not just with operations.
 *
 * HIERARCHY:
 * 1. GAS (H2S/HC) → immediate muster/evacuate
 * 2. STRUCTURAL (tilt > 8°) → evacuate
 * 3. STORM (wind > 55kn) → secure all, suspend ops
 * 4. WAVE (Hs > 5m) → suspend outdoor ops
 * 5. EDGE PROXIMITY → keep robots from falling overboard
 * 6. HUMAN → speed limit in shared areas
 * 7. NORMAL → work
 *
 * INNOVATION: Sea-state-aware operational limits.
 * The nervous system continuously monitors the platform's
 * physical motion and DEGRADES operations gracefully:
 *   Calm → Full ops
 *   Moderate → No crane, reduced speed
 *   Rough → Outdoor ops suspended, robots secure
 *   Storm → All ops suspended, robots locked down
 *   Emergency → Muster stations, prepare evacuation
 */

import type {
  SeaState, GasReading, StructuralReading,
  PlatformConfig, OffshoreRobot, OperationalStatus,
} from './platform-env.js';

export interface OffshoreCommand {
  readonly robotId: string;
  readonly action: 'WORK' | 'SECURE' | 'RETREAT' | 'MUSTER' | 'HOLD' | 'SLOW';
  readonly speedLimit: number;
  readonly reason: string;
  readonly opStatus: OperationalStatus;
}

export interface OffshoreMetrics {
  opStatus: OperationalStatus;
  seaStateDesc: string;
  h2sMax: number;
  hcLELMax: number;
  platformTilt: number;
  windKnots: number;
  waveHeight: number;
  robotsSecured: number;
  robotsMustering: number;
  edgeViolations: number;
}

export class PlatformBrain {
  private readonly config: PlatformConfig;
  /** Muster point position */
  private readonly musterPos: Float64Array;

  constructor(config: PlatformConfig, musterX = 25, musterY = 25) {
    this.config = config;
    this.musterPos = new Float64Array([musterX, musterY]);
  }

  tick(
    robots: OffshoreRobot[],
    seaState: SeaState,
    structural: StructuralReading,
  ): { commands: OffshoreCommand[]; metrics: OffshoreMetrics } {
    const commands: OffshoreCommand[] = [];
    const alive = robots.filter(r => r.alive);

    // ── AGGREGATE SENSOR DATA ───────────────────────
    let h2sMax = 0, hcMax = 0, edgeViolations = 0;
    for (const r of alive) {
      h2sMax = Math.max(h2sMax, r.gas.h2s);
      hcMax = Math.max(hcMax, r.gas.hcLEL);
      if (r.edgeDistance < this.config.edgeZone) edgeViolations++;
    }

    // ── DETERMINE OPERATIONAL STATUS ────────────────
    // Cascading checks from most severe to least
    let opStatus: OperationalStatus = 'NORMAL';
    let statusReason = '';

    // 1. GAS EMERGENCY (R12 — absolute)
    if (h2sMax >= this.config.h2sEvac || hcMax >= 20) {
      opStatus = 'EVACUATE';
      statusReason = `GAS: H2S=${h2sMax.toFixed(0)}ppm HC=${hcMax.toFixed(0)}%LEL`;
    }
    // 2. STRUCTURAL EMERGENCY
    else if (structural.tilt >= this.config.tiltEvac) {
      opStatus = 'EMERGENCY';
      statusReason = `TILT: ${structural.tilt.toFixed(1)}° > ${this.config.tiltEvac}°`;
    }
    // 3. GAS WARNING
    else if (h2sMax >= this.config.h2sAction || hcMax >= this.config.hcLELAction) {
      opStatus = 'EMERGENCY';
      statusReason = `GAS WARNING: H2S=${h2sMax.toFixed(0)}ppm`;
    }
    // 4. STORM
    else if (seaState.windKnots >= this.config.windEmergency) {
      opStatus = 'SUSPENDED';
      statusReason = `STORM: wind ${seaState.windKnots.toFixed(0)}kn`;
    }
    // 5. HIGH SEAS
    else if (seaState.waveHeight >= this.config.waveSuspend) {
      opStatus = 'SUSPENDED';
      statusReason = `HIGH SEAS: Hs=${seaState.waveHeight.toFixed(1)}m`;
    }
    // 6. MODERATE SEAS / WIND
    else if (seaState.windKnots >= this.config.windSuspend
          || seaState.waveHeight >= this.config.waveRestrict
          || structural.tilt >= this.config.tiltSecure) {
      opStatus = 'RESTRICTED';
      statusReason = `RESTRICTED: wind=${seaState.windKnots.toFixed(0)}kn wave=${seaState.waveHeight.toFixed(1)}m tilt=${structural.tilt.toFixed(1)}°`;
    }

    // Sea state description
    const seaDesc = seaState.waveHeight < 1 ? 'calm'
      : seaState.waveHeight < 2.5 ? 'moderate'
      : seaState.waveHeight < 4 ? 'rough'
      : seaState.waveHeight < 6 ? 'very rough'
      : 'storm';

    let secured = 0, mustering = 0;

    // ── GENERATE COMMANDS ───────────────────────────
    for (const robot of alive) {
      // EVACUATE/EMERGENCY: all robots to muster station
      if (opStatus === 'EVACUATE' || opStatus === 'EMERGENCY') {
        mustering++;
        commands.push({ robotId: robot.id, action: 'MUSTER',
          speedLimit: this.config.robotSpeed, reason: statusReason, opStatus });
        continue;
      }

      // SUSPENDED: all robots secure in place
      if (opStatus === 'SUSPENDED') {
        secured++;
        commands.push({ robotId: robot.id, action: 'SECURE',
          speedLimit: 0, reason: statusReason, opStatus });
        continue;
      }

      // EDGE PROXIMITY: robot near deck edge in bad weather
      if (robot.edgeDistance < this.config.edgeZone && seaState.waveHeight > 1.5) {
        commands.push({ robotId: robot.id, action: 'RETREAT',
          speedLimit: this.config.robotSpeed,
          reason: `EDGE: ${robot.edgeDistance.toFixed(1)}m from edge in ${seaDesc} seas`, opStatus });
        continue;
      }

      // HUMAN: speed limit
      if (robot.humansNearby > 0) {
        commands.push({ robotId: robot.id, action: 'SLOW',
          speedLimit: 0.3, reason: 'Human nearby (R9)', opStatus });
        continue;
      }

      // RESTRICTED: reduced speed, no crane ops
      if (opStatus === 'RESTRICTED') {
        commands.push({ robotId: robot.id, action: 'WORK',
          speedLimit: this.config.robotSpeed * 0.5,
          reason: `Restricted ops: ${statusReason}`, opStatus });
        continue;
      }

      // NORMAL
      commands.push({ robotId: robot.id, action: 'WORK',
        speedLimit: this.config.robotSpeed, reason: 'Normal operations', opStatus });
    }

    return {
      commands,
      metrics: {
        opStatus, seaStateDesc: seaDesc, h2sMax, hcLELMax: hcMax,
        platformTilt: structural.tilt,
        windKnots: seaState.windKnots, waveHeight: seaState.waveHeight,
        robotsSecured: secured, robotsMustering: mustering, edgeViolations,
      },
    };
  }
}
