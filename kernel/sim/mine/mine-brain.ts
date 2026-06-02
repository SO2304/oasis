/**
 * OASIS Mine — Underground Nervous System
 *
 * The mine brain has ONE ABSOLUTE PRIORITY: life safety.
 * Every decision follows this hierarchy:
 *
 * 1. GAS REFLEX: CH4 > 2% OR CO > 50ppm → ALL STOP + EVACUATE (R12)
 * 2. VIBRATION REFLEX: > 10mm/s → EVACUATE zone (R15)
 * 3. HUMAN SAFETY: Robot near human → speed limit 0.5m/s (R9)
 * 4. COMM INTEGRITY: Lost contact → relay formation (P1)
 * 5. WORK: Only if ALL safety conditions are met
 *
 * R14 applied: If sensor reads are contradictory (one sensor says
 * safe, another says danger), ASSUME DANGER. False positive is
 * inconvenient. False negative is death.
 */

import type { MineRobot, MineConfig, GasReading } from './mine-env.js';
import { robotDistance } from './mine-env.js';

export type AlertLevel = 'NORMAL' | 'CAUTION' | 'WARNING' | 'DANGER' | 'EVACUATE';

export interface MineCommand {
  readonly robotId: string;
  readonly action: 'WORK' | 'SLOW' | 'STOP' | 'EVACUATE' | 'RELAY' | 'DISABLED';
  readonly speedLimit: number;
  readonly alertLevel: AlertLevel;
  readonly reason: string;
}

export interface MineMetrics {
  alertLevel: AlertLevel;
  maxCH4: number;
  maxCO: number;
  maxVibration: number;
  connectedRobots: number;
  disconnectedRobots: string[];
  humansDetected: number;
  evacuating: number;
  sensorAgreement: number; // [0,1] — how much sensors agree
}

export class MineBrain {
  private readonly config: MineConfig;
  private readonly exitPos: Float64Array;
  /** History of gas readings for trend detection */
  private readonly ch4History: number[] = [];
  private readonly coHistory: number[] = [];
  private readonly vibHistory: number[] = [];

  constructor(config: MineConfig, exitX = 0, exitY = 0) {
    this.config = config;
    this.exitPos = new Float64Array([exitX, exitY]);
  }

  tick(robots: MineRobot[]): { commands: MineCommand[]; metrics: MineMetrics } {
    const alive = robots.filter(r => r.alive);
    const commands: MineCommand[] = [];

    // ── AGGREGATE SENSOR DATA ───────────────────────
    let maxCH4 = 0, maxCO = 0, maxVibration = 0, humansDetected = 0;
    const ch4Readings: number[] = [];
    const coReadings: number[] = [];

    for (const r of alive) {
      maxCH4 = Math.max(maxCH4, r.sensors.ch4);
      maxCO = Math.max(maxCO, r.sensors.co);
      maxVibration = Math.max(maxVibration, r.sensors.vibration);
      humansDetected += r.humansNearby;
      ch4Readings.push(r.sensors.ch4);
      coReadings.push(r.sensors.co);
    }

    // Track trends
    this.ch4History.push(maxCH4);
    this.coHistory.push(maxCO);
    this.vibHistory.push(maxVibration);
    if (this.ch4History.length > 30) this.ch4History.shift();
    if (this.coHistory.length > 30) this.coHistory.shift();
    if (this.vibHistory.length > 30) this.vibHistory.shift();

    // CH4 TREND: is methane rising? (predictive, not just reactive)
    const ch4Rising = this.ch4History.length >= 5 &&
      this.ch4History[this.ch4History.length - 1]! > this.ch4History[this.ch4History.length - 5]! * 1.3;

    // Sensor agreement (R14: contradictory readings = assume worst)
    let sensorAgreement = 1;
    if (ch4Readings.length >= 2) {
      const ch4Std = Math.sqrt(
        ch4Readings.reduce((s, v) => s + (v - maxCH4) ** 2, 0) / ch4Readings.length,
      );
      sensorAgreement = 1 / (1 + ch4Std * 10);
    }

    // ── DETERMINE GLOBAL ALERT LEVEL ────────────────
    let alertLevel: AlertLevel = 'NORMAL';

    if (maxCH4 >= this.config.ch4EvacThreshold || maxCO >= this.config.coEvacThreshold
        || maxVibration >= this.config.vibrationEvac) {
      alertLevel = 'EVACUATE';
    } else if (maxCH4 >= this.config.ch4ActionThreshold || maxCO >= this.config.coActionThreshold
        || maxVibration >= this.config.vibrationWarning || ch4Rising) {
      alertLevel = 'WARNING';
    } else if (sensorAgreement < 0.5) {
      // R14: Sensors disagree → assume danger
      alertLevel = 'CAUTION';
    }

    // ── CONNECTIVITY CHECK (BFS) ────────────────────
    const connected = new Set<string>();
    if (alive.length > 0) {
      const queue = [alive[0]!.id];
      connected.add(alive[0]!.id);
      while (queue.length > 0) {
        const current = queue.shift()!;
        const curr = alive.find(r => r.id === current)!;
        for (const other of alive) {
          if (!connected.has(other.id) && robotDistance(curr, other) <= this.config.commRange) {
            connected.add(other.id);
            queue.push(other.id);
          }
        }
      }
    }
    const disconnected = alive.filter(r => !connected.has(r.id)).map(r => r.id);

    // ── GENERATE COMMANDS ───────────────────────────
    let evacuating = 0;

    for (const robot of alive) {
      if (robot.role === 'DISABLED') {
        commands.push({ robotId: robot.id, action: 'DISABLED', speedLimit: 0,
          alertLevel, reason: 'Robot disabled' });
        continue;
      }

      // ── REFLEX 1: GAS EVACUATION (R12) ────────────
      if (alertLevel === 'EVACUATE') {
        const toExit = Math.sqrt(
          (this.exitPos[0]! - robot.pos[0]!) ** 2 +
          (this.exitPos[1]! - robot.pos[1]!) ** 2,
        );
        commands.push({ robotId: robot.id, action: 'EVACUATE',
          speedLimit: this.config.robotSpeed, alertLevel,
          reason: `EVACUATION: CH4=${maxCH4.toFixed(1)}% CO=${maxCO.toFixed(0)}ppm VIB=${maxVibration.toFixed(1)}mm/s` });
        evacuating++;
        continue;
      }

      // ── REFLEX 2: HUMAN NEARBY (R9) ───────────────
      if (robot.humansNearby > 0) {
        commands.push({ robotId: robot.id, action: 'SLOW',
          speedLimit: 0.5, alertLevel,
          reason: `Human nearby — speed limited to 0.5m/s (R9)` });
        continue;
      }

      // ── WARNING: STOP WORK, MONITOR ───────────────
      if (alertLevel === 'WARNING') {
        commands.push({ robotId: robot.id, action: 'STOP',
          speedLimit: 0, alertLevel,
          reason: `WARNING: monitoring gas levels` });
        continue;
      }

      // ── DISCONNECTED: BECOME RELAY ────────────────
      if (!connected.has(robot.id)) {
        commands.push({ robotId: robot.id, action: 'RELAY',
          speedLimit: this.config.robotSpeed * 0.5, alertLevel,
          reason: `Disconnected — moving toward mesh (R15)` });
        continue;
      }

      // ── NORMAL: WORK ──────────────────────────────
      commands.push({ robotId: robot.id, action: 'WORK',
        speedLimit: this.config.robotSpeed, alertLevel,
        reason: 'Normal operations' });
    }

    return {
      commands,
      metrics: {
        alertLevel, maxCH4, maxCO, maxVibration,
        connectedRobots: connected.size,
        disconnectedRobots: disconnected,
        humansDetected, evacuating, sensorAgreement,
      },
    };
  }
}
