/**
 * OASIS Kernel — Sensor Type Definitions
 *
 * Sensors in OASIS don't produce "data points".
 * They produce IMPRINTS — projections into latent space
 * that deform the Tension Field.
 *
 * A camera doesn't say "object at (3,2)".
 * It says "I feel repulsion at this angle in the latent manifold."
 */

import type { Vec } from '../physics/vector-math.js';
import type { AgentId, TenantId, DriverId } from '../types.js';

// ─── Sensor Modalities ──────────────────────────────────────────

export const SensorModality = {
  /** 2D image stream — color, depth, semantic segmentation */
  CAMERA: 'CAMERA',
  /** 3D point cloud — range, reflectivity */
  LIDAR: 'LIDAR',
  /** Inertial measurement — acceleration, angular velocity */
  IMU: 'IMU',
  /** Distance measurement — ultrasonic, infrared */
  PROXIMITY: 'PROXIMITY',
  /** Joint angles, motor currents, end-effector force */
  PROPRIOCEPTION: 'PROPRIOCEPTION',
  /** Temperature, humidity, gas concentration */
  ENVIRONMENT: 'ENVIRONMENT',
  /** Agent-to-agent semantic signals */
  SEMANTIC: 'SEMANTIC',
} as const;
export type SensorModality = (typeof SensorModality)[keyof typeof SensorModality];

// ─── Raw Sensor Reading ─────────────────────────────────────────

export interface SensorReading {
  /** Which sensor produced this */
  readonly sensorId: string;
  /** Which driver this sensor belongs to */
  readonly driverId: DriverId;
  /** Sensor type */
  readonly modality: SensorModality;
  /** Raw numeric values (modality-specific) */
  readonly values: Record<string, number>;
  /** Sensor's self-reported confidence [0, 1] */
  readonly confidence: number;
  /** Monotonic timestamp */
  readonly timestamp: bigint;
}

// ─── Sensor Imprint (after projection into latent space) ────────

export interface SensorImprint {
  /** The sensor that produced this imprint */
  readonly sensorId: string;
  readonly modality: SensorModality;
  /** Direction in latent space this sensor "pushes" toward */
  readonly direction: Vec;
  /** Magnitude of the push */
  readonly magnitude: number;
  /** How reliable this imprint is [0, 1] — decays with age and sensor health */
  readonly reliability: number;
  /** Semantic signature — what "kind" of observation this is */
  readonly signature: Vec;
  /** When this imprint was created */
  readonly timestamp: bigint;
  /** Age in ticks since creation */
  age: number;
}

// ─── Perception Vector (fused from all imprints) ────────────────

export interface PerceptionVector {
  /** Fused direction in latent space */
  readonly direction: Vec;
  /** Fused magnitude */
  readonly magnitude: number;
  /** Overall perceptual confidence [0, 1] */
  readonly confidence: number;
  /** Per-modality agreement scores — high = sensors agree */
  readonly modalityAgreement: Map<SensorModality, number>;
  /** Detected contradictions between modalities */
  readonly contradictions: PerceptionContradiction[];
  /** Number of active sensors contributing */
  readonly activeSensorCount: number;
  /** Number of sensors that have gone silent (R15 triggers) */
  readonly blackHoleCount: number;
  /** Timestamp of fusion */
  readonly timestamp: bigint;
}

// ─── Contradiction Detection ────────────────────────────────────

export interface PerceptionContradiction {
  /** First sensor */
  readonly sensorA: string;
  readonly modalityA: SensorModality;
  /** Second sensor */
  readonly sensorB: string;
  readonly modalityB: SensorModality;
  /** How much they disagree [0, 1] — 1 = perfectly opposite */
  readonly severity: number;
  /** The axis along which they disagree */
  readonly conflictAxis: Vec;
}

// ─── Sensor Configuration ───────────────────────────────────────

export interface SensorConfig {
  readonly sensorId: string;
  readonly driverId: DriverId;
  readonly modality: SensorModality;
  /** How this sensor projects readings into latent space */
  readonly projectionDim: number;
  /** Base reliability weight [0, 1] */
  readonly baseReliability: number;
  /** Maximum staleness in ticks before R15 triggers */
  readonly maxStaleTicks: number;
  /** Which dimensions of latent space this sensor primarily affects */
  readonly primaryDimensions: number[];
}
