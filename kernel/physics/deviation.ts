/**
 * OASIS Kernel — Deviation Analysis
 *
 * Extracted from ReflectionEngine for R10 compliance.
 * Classification and diagnosis of prediction errors.
 */

import type { DriverId } from '../types.js';
import type { Vec } from './vector-math.js';

// ─── Efference Copy ─────────────────────────────────────────────

export interface EfferenceCopy {
  readonly driverId: DriverId;
  readonly predictedPosition: Vec;
  readonly predictedEffort: Vec;
  readonly commandForce: number;
  readonly timestamp: bigint;
  readonly expectedLatencyTicks: number;
}

// ─── Deviation Vector ───────────────────────────────────────────

export interface DeviationVector {
  readonly driverId: DriverId;
  readonly direction: Vec;
  readonly magnitude: number;
  readonly positionError: number;
  readonly effortError: number;
  readonly persistenceTicks: number;
  readonly severity: DeviationSeverity;
}

export const DeviationSeverity = {
  NOMINAL: 'NOMINAL',
  RESISTANCE: 'RESISTANCE',
  ANOMALY: 'ANOMALY',
  DYSMORPHIA: 'DYSMORPHIA',
} as const;
export type DeviationSeverity = (typeof DeviationSeverity)[keyof typeof DeviationSeverity];

// ─── Pain Signal ────────────────────────────────────────────────

export interface PainSignal {
  readonly driverId: DriverId;
  readonly intensity: number;
  readonly direction: Vec;
  readonly cause: string;
  readonly r16Trigger: boolean;
}

export interface ReflectionTickResult {
  readonly deviations: DeviationVector[];
  readonly painSignals: PainSignal[];
  readonly r16Triggered: boolean;
  readonly tensionsInjected: number;
  readonly adaptations: number;
}

// ─── Severity Classification ────────────────────────────────────

export function classifySeverity(
  magnitude: number,
  persistence: number,
  thresholds: { resistance: number; anomaly: number; dysmorphia: number },
): DeviationSeverity {
  let severity: DeviationSeverity;

  if (magnitude < thresholds.resistance) {
    severity = DeviationSeverity.NOMINAL;
  } else if (magnitude < thresholds.anomaly) {
    severity = DeviationSeverity.RESISTANCE;
  } else if (magnitude < thresholds.dysmorphia || persistence < 5) {
    severity = DeviationSeverity.ANOMALY;
  } else {
    severity = DeviationSeverity.DYSMORPHIA;
  }

  if (persistence > 10 && severity === DeviationSeverity.RESISTANCE) {
    severity = DeviationSeverity.ANOMALY;
  }
  if (persistence > 20 && severity === DeviationSeverity.ANOMALY) {
    severity = DeviationSeverity.DYSMORPHIA;
  }

  return severity;
}

// ─── Diagnosis ──────────────────────────────────────────────────

export function diagnoseCause(dev: DeviationVector): string {
  if (dev.effortError > dev.positionError * 2) {
    return 'MECHANICAL_JAM: High effort with no movement — possible blockage';
  }
  if (dev.positionError > dev.effortError * 2) {
    return 'DRIFT: Movement without expected effort — possible slippage';
  }
  if (dev.persistenceTicks > 15) {
    return `SUSTAINED_DEVIATION: ${dev.persistenceTicks} ticks of continuous error`;
  }
  return `PREDICTION_ERROR: pos=${dev.positionError.toFixed(3)} effort=${dev.effortError.toFixed(3)}`;
}
