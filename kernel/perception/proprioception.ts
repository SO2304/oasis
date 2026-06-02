/**
 * OASIS Kernel — Proprioception (PR1)
 *
 * Closes the sensorimotor loop: actuator telemetry → sensor imprint
 * → back into the Tension Field.
 *
 * INSIGHT: Proprioception is NOT external sensing. It's the body
 * reporting its own state. In biological systems, proprioceptors
 * in muscles and joints tell the brain where the limbs are WITHOUT
 * looking at them.
 *
 * In OASIS, the HAL drivers report telemetry (position, force,
 * current, temperature). This module projects that telemetry
 * into latent space as a "body imprint" — a continuous signal
 * that the Reflection Engine compares with predictions.
 *
 * Key proprioceptive signals:
 * - Position: where am I actually?
 * - Velocity: how fast am I actually moving?
 * - Force: how much resistance am I feeling?
 * - Current: how hard is the motor working?
 * - Temperature: is something overheating?
 * - Stress: how close to mechanical failure?
 */

import type { DriverId, DriverTelemetry } from '../types.js';
import { monotonicNow } from '../types.js';
import {
  type Vec,
  zeros,
  normalize,
  norm,
  scale,
} from '../physics/vector-math.js';
import type { SensorImprint } from './sensor-types.js';

// ─── Proprioceptive State ───────────────────────────────────────

export interface ProprioceptiveState {
  readonly driverId: DriverId;
  /** Current position imprint in latent space */
  readonly positionImprint: Vec;
  /** Current effort imprint (force/current/stress combined) */
  readonly effortImprint: Vec;
  /** Body stress level [0, 1] — composite of current, temp, stress */
  readonly bodyStress: number;
  /** Is the body behaving normally? */
  readonly healthy: boolean;
  /** Raw telemetry values for reflection comparison */
  readonly rawValues: Record<string, number>;
  readonly timestamp: bigint;
}

// ─── Configuration ──────────────────────────────────────────────

export interface ProprioceptionConfig {
  /** Which latent dimensions encode position */
  readonly positionDims: number[];
  /** Which latent dimensions encode effort/force */
  readonly effortDims: number[];
  /** Temperature warning threshold (celsius) */
  readonly tempWarning: number;
  /** Temperature critical threshold (celsius) */
  readonly tempCritical: number;
  /** Stress threshold for pain signal */
  readonly stressThreshold: number;
}

const DEFAULT_CONFIG: ProprioceptionConfig = {
  positionDims: [24, 25, 26],
  effortDims: [27, 28, 29],
  tempWarning: 60,
  tempCritical: 85,
  stressThreshold: 0.7,
};

// ─── Proprioception Module ──────────────────────────────────────

export class Proprioception {
  private readonly configs = new Map<DriverId, ProprioceptionConfig>();
  private readonly states = new Map<DriverId, ProprioceptiveState>();
  private readonly dim: number;

  constructor(dim: number) {
    this.dim = dim;
  }

  /** Register a driver for proprioceptive monitoring */
  register(driverId: DriverId, config: Partial<ProprioceptionConfig> = {}): void {
    this.configs.set(driverId, { ...DEFAULT_CONFIG, ...config });
  }

  /**
   * Process telemetry from a driver into a proprioceptive state.
   *
   * This is the "nerve signal" from body to brain.
   */
  processTelemetry(telemetry: DriverTelemetry): ProprioceptiveState {
    const config = this.configs.get(telemetry.driverId);
    if (!config) {
      throw new Error(`Driver ${telemetry.driverId} not registered for proprioception`);
    }

    const v = telemetry.values;

    // Project position into latent space
    const positionImprint = zeros(this.dim);
    const posVals = [v['posX'] ?? 0, v['posY'] ?? 0, v['posZ'] ?? 0];
    for (let i = 0; i < config.positionDims.length && i < posVals.length; i++) {
      const d = config.positionDims[i]!;
      if (d < this.dim) positionImprint[d] = posVals[i]!;
    }

    // Project effort into latent space
    // Effort = combination of force, current draw, and stress
    const effortImprint = zeros(this.dim);
    const force = v['force'] ?? 0;
    const currentDraw = v['currentDraw'] ?? 0;
    const stress = v['stress'] ?? 0;
    const effortVals = [force, currentDraw * 10, stress];
    for (let i = 0; i < config.effortDims.length && i < effortVals.length; i++) {
      const d = config.effortDims[i]!;
      if (d < this.dim) effortImprint[d] = effortVals[i]!;
    }

    // Composite body stress
    const temperature = v['temperature'] ?? 25;
    const tempStress = temperature > config.tempWarning
      ? Math.min(1, (temperature - config.tempWarning) / (config.tempCritical - config.tempWarning))
      : 0;

    const bodyStress = Math.min(1, Math.max(stress, tempStress, currentDraw > 5 ? currentDraw / 10 : 0));

    const state: ProprioceptiveState = {
      driverId: telemetry.driverId,
      positionImprint,
      effortImprint,
      bodyStress,
      healthy: telemetry.healthy && bodyStress < config.stressThreshold,
      rawValues: { ...telemetry.values },
      timestamp: telemetry.timestamp,
    };

    this.states.set(telemetry.driverId, state);
    return state;
  }

  /**
   * Convert proprioceptive state to a SensorImprint for fusion.
   * This lets the proprioception feed into the same pipeline as
   * external sensors — unified perception.
   */
  toSensorImprint(driverId: DriverId): SensorImprint | null {
    const state = this.states.get(driverId);
    if (!state) return null;

    // Direction = position + effort combined
    const combined = zeros(this.dim);
    for (let i = 0; i < this.dim; i++) {
      combined[i] = (state.positionImprint[i] ?? 0) + (state.effortImprint[i] ?? 0);
    }

    const mag = norm(combined);

    return {
      sensorId: `proprio-${driverId}`,
      modality: 'PROPRIOCEPTION',
      direction: mag > 0 ? normalize(combined) : combined,
      magnitude: mag,
      reliability: state.healthy ? 0.99 : 0.5, // Proprioception is highly reliable when healthy
      signature: normalize(state.effortImprint.length > 0 && norm(state.effortImprint) > 0
        ? state.effortImprint
        : combined),
      timestamp: state.timestamp,
      age: 0,
    };
  }

  /** Get the latest proprioceptive state for a driver */
  getState(driverId: DriverId): ProprioceptiveState | undefined {
    return this.states.get(driverId);
  }

  /** Get all monitored driver IDs */
  getMonitoredDrivers(): DriverId[] {
    return [...this.configs.keys()];
  }
}
