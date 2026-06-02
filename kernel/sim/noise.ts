/**
 * OASIS — Sensor Noise & Environmental Disturbances
 *
 * Real-world conditions that make control hard:
 * - IMU noise (accelerometer + gyroscope)
 * - Wind gusts and turbulence
 * - Motor thrust variance
 *
 * All models are parameterized from real sensor datasheets:
 * - MPU6050 accelerometer: ±0.05 m/s² noise, 0.1% scale error
 * - MPU6050 gyroscope: ±0.01 rad/s noise, 0.5°/hr drift
 * - Wind: Dryden turbulence model (simplified)
 */

import type { DroneState } from './fly-physics.js';
import { SeededRandom } from '../optim/seeded-random.js';

// ─── Deterministic Random (seedable for test reproducibility) ───

/** Global PRNG — seedable for deterministic tests */
let rng = new SeededRandom(42);

/** Reset the noise PRNG with a specific seed */
export function seedNoise(seed: number): void {
  rng = new SeededRandom(seed);
}

/** Box-Muller gaussian with mean 0, stddev σ (deterministic) */
export function gaussian(sigma: number): number {
  return rng.gaussian(sigma);
}

let hasSpare = false;
let spare = 0;

/** Box-Muller gaussian using Math.random (LEGACY — use gaussian() instead) */
export function gaussianLegacy(sigma: number): number {
  if (hasSpare) {
    hasSpare = false;
    return spare * sigma;
  }
  let u: number, v: number, s: number;
  do {
    u = Math.random() * 2 - 1;
    v = Math.random() * 2 - 1;
    s = u * u + v * v;
  } while (s >= 1 || s === 0);
  s = Math.sqrt(-2 * Math.log(s) / s);
  spare = v * s;
  hasSpare = true;
  return u * s * sigma;
}

// ─── IMU Noise Model ────────────────────────────────────────────

export interface IMUConfig {
  /** Accelerometer noise σ in m/s² (MPU6050: ~0.05) */
  accelNoiseSigma: number;
  /** Gyroscope noise σ in rad/s (MPU6050: ~0.01) */
  gyroNoiseSigma: number;
  /** Gyroscope bias drift rate in rad/s per tick */
  gyroBiasDrift: number;
  /** Accelerometer scale error (0.001 = 0.1%) */
  accelScaleError: number;
}

export const MPU6050_NOISE: IMUConfig = {
  accelNoiseSigma: 0.04,    // ±0.04 m/s² (datasheet: ~300μg/√Hz at 333Hz)
  gyroNoiseSigma: 0.005,    // ±0.005 rad/s (datasheet: ~0.05°/s/√Hz at 333Hz)
  gyroBiasDrift: 0.000005,  // Bias random walk (~0.5°/hr typical)
  accelScaleError: 0.001,   // 0.1% scale factor error
};

/** Read noisy IMU from true state */
export function noisyIMU(
  state: DroneState,
  config: IMUConfig,
  gyroBias: Float64Array, // Mutable — drifts over time
): { accel: Float64Array; gyro: Float64Array } {
  // Gyro bias random walk
  for (let i = 0; i < 3; i++) {
    gyroBias[i]! += gaussian(config.gyroBiasDrift);
  }

  const accel = new Float64Array(3);
  const gyro = new Float64Array(3);

  // Gravity vector rotated to body frame using true attitude
  const cr = Math.cos(state.att[0]!), sr = Math.sin(state.att[0]!);
  const cp = Math.cos(state.att[1]!), sp = Math.sin(state.att[1]!);
  const gravBody = [
    9.81 * sp,          // X in body
    -9.81 * sr * cp,    // Y in body
    9.81 * cr * cp,     // Z in body
  ];

  for (let i = 0; i < 3; i++) {
    // Accelerometer: gravity_in_body + noise + scale error
    accel[i] = gravBody[i]! * (1 + gaussian(config.accelScaleError)) + gaussian(config.accelNoiseSigma);

    // Gyroscope: true + bias + noise
    gyro[i] = state.omega[i]! + gyroBias[i]! + gaussian(config.gyroNoiseSigma);
  }

  return { accel, gyro };
}

// ─── Wind Model ─────────────────────────────────────────────────

export interface WindConfig {
  /** Mean wind speed in m/s */
  meanSpeed: number;
  /** Wind direction in radians (0 = +X) */
  direction: number;
  /** Gust intensity σ in m/s */
  gustSigma: number;
  /** Turbulence time constant (higher = slower changes) */
  turbulenceTau: number;
}

export const LIGHT_WIND: WindConfig = {
  meanSpeed: 0.3,
  direction: 0,
  gustSigma: 0.15,
  turbulenceTau: 0.5,
};

export const MODERATE_WIND: WindConfig = {
  meanSpeed: 1.0,
  direction: Math.PI / 4,
  gustSigma: 0.5,
  turbulenceTau: 0.3,
};

export interface WindState {
  gustX: number;
  gustY: number;
  gustZ: number;
}

/** Compute wind force on the drone at current tick */
export function windForce(
  config: WindConfig,
  windState: WindState,
  dt: number,
  mass: number,
): Float64Array {
  // Dryden turbulence (simplified first-order): gust = (1-dt/tau)*gust + noise
  const decay = 1 - dt / config.turbulenceTau;
  windState.gustX = decay * windState.gustX + gaussian(config.gustSigma) * Math.sqrt(dt);
  windState.gustY = decay * windState.gustY + gaussian(config.gustSigma) * Math.sqrt(dt);
  windState.gustZ = decay * windState.gustZ + gaussian(config.gustSigma * 0.3) * Math.sqrt(dt);

  const force = new Float64Array(3);
  force[0] = (config.meanSpeed * Math.cos(config.direction) + windState.gustX) * 0.001; // Drag force
  force[1] = (config.meanSpeed * Math.sin(config.direction) + windState.gustY) * 0.001;
  force[2] = windState.gustZ * 0.001;

  return force;
}

// ─── Complementary Filter ───────────────────────────────────────

/**
 * Fuse gyro and accelerometer to estimate attitude.
 *
 * Gyro: fast, accurate short-term, drifts long-term
 * Accel: noisy, no drift, measures gravity direction
 *
 * Complementary filter: attitude = α * (gyro_integrated) + (1-α) * (accel_angle)
 * α ≈ 0.98 (trust gyro 98%, accel 2%)
 */
export interface AttitudeEstimate {
  roll: number;
  pitch: number;
  yaw: number; // Gyro-only (no magnetometer)
}

/**
 * MAHONY FILTER — Non-linear complementary filter.
 *
 * Unlike linear complementary, Mahony uses the CROSS PRODUCT
 * between measured gravity and expected gravity to compute
 * the attitude error. This is more robust to noise because
 * it works on the SO(3) manifold, not Euler angles.
 *
 * Standard for micro-drones. Reference: Mahony et al. 2008
 * "Nonlinear Complementary Filters on the Special Orthogonal Group"
 *
 * Kp: proportional gain (how fast to correct) — higher = faster but noisier
 * Ki: integral gain (compensates gyro bias) — prevents long-term drift
 */
const mahonyIntegral = [0, 0, 0]; // Bias estimate

export function complementaryFilter(
  prev: AttitudeEstimate,
  gyro: Float64Array,
  accel: Float64Array,
  dt: number,
  _alpha = 0.98, // Ignored — kept for API compat
  Kp = 2.0,  // Mahony proportional gain
  Ki = 0.005, // Mahony integral gain
): AttitudeEstimate {
  // Normalize accelerometer (gravity direction)
  const aMag = Math.sqrt(accel[0]! ** 2 + accel[1]! ** 2 + accel[2]! ** 2);
  if (aMag < 0.1) {
    // Free fall — can't use accel, gyro-only
    return {
      roll: prev.roll + gyro[0]! * dt,
      pitch: prev.pitch + gyro[1]! * dt,
      yaw: prev.yaw + gyro[2]! * dt,
    };
  }

  const ax = accel[0]! / aMag;
  const ay = accel[1]! / aMag;
  const az = accel[2]! / aMag;

  // Expected gravity direction in body frame from current attitude
  const cr = Math.cos(prev.roll), sr = Math.sin(prev.roll);
  const cp = Math.cos(prev.pitch), sp = Math.sin(prev.pitch);
  const gx = sp;
  const gy = -sr * cp;
  const gz = cr * cp;

  // Error = cross product (measured_gravity × expected_gravity)
  const ex = ay * gz - az * gy;
  const ey = az * gx - ax * gz;
  const ez = ax * gy - ay * gx;

  // Integral feedback (accumulates to estimate gyro bias)
  mahonyIntegral[0]! += Ki * ex * dt;
  mahonyIntegral[1]! += Ki * ey * dt;
  mahonyIntegral[2]! += Ki * ez * dt;

  // Corrected gyro = raw gyro + proportional_correction + integral_correction
  const corrGx = gyro[0]! + Kp * ex + mahonyIntegral[0]!;
  const corrGy = gyro[1]! + Kp * ey + mahonyIntegral[1]!;
  const corrGz = gyro[2]! + Kp * ez + mahonyIntegral[2]!;

  return {
    roll: prev.roll + corrGx * dt,
    pitch: prev.pitch + corrGy * dt,
    yaw: prev.yaw + corrGz * dt,
  };
}

export function resetMahony(): void {
  mahonyIntegral[0] = 0;
  mahonyIntegral[1] = 0;
  mahonyIntegral[2] = 0;
}
