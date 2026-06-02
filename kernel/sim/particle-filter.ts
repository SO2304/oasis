/**
 * OASIS — Particle Filter (Quantum-Inspired State Estimation)
 *
 * WHY THIS IS BETTER THAN MAHONY/KALMAN:
 *
 * Mahony: assumes small angles, linear dynamics → breaks at high tilt
 * EKF: linearizes around estimate → fails with multimodal distributions
 * Particle Filter: maintains N PARALLEL hypotheses of the true state
 *
 * Each "particle" is a possible world — a hypothesis about the drone's
 * actual attitude and position. When a sensor reading arrives, particles
 * that agree with the measurement get REINFORCED (higher weight),
 * particles that disagree get SUPPRESSED (lower weight).
 *
 * This is SUPERPOSITION → MEASUREMENT → COLLAPSE:
 * - N particles = superposition of possible states
 * - Sensor update = measurement (Bayesian weight update)
 * - Weighted mean = collapse to best estimate
 *
 * The key advantage: handles NON-GAUSSIAN noise and MULTIMODAL
 * distributions. If the drone might be in state A OR state B
 * (ambiguous sensor), the particle filter maintains BOTH hypotheses
 * until more data disambiguates. Kalman can't do this.
 *
 * Cost: O(N) per update. With N=50 particles at 333Hz = 16,650 updates/sec.
 * On this Ryzen 3, each particle update is ~100ns. Total: 5μs/tick. Feasible.
 */

export interface Particle {
  /** State hypothesis: [roll, pitch, yaw, wx, wy, wz] */
  state: Float64Array;
  /** Weight (probability of this hypothesis being correct) */
  weight: number;
}

export interface ParticleFilterConfig {
  /** Number of particles (more = better estimate, slower) */
  numParticles: number;
  /** Process noise σ (how much we expect the state to change per tick) */
  processNoise: number;
  /** Measurement noise σ for accelerometer */
  accelMeasNoise: number;
  /** Measurement noise σ for gyroscope */
  gyroMeasNoise: number;
  /** Effective sample size threshold for resampling */
  resampleThreshold: number;
}

export const DEFAULT_PF_CONFIG: ParticleFilterConfig = {
  numParticles: 50,
  processNoise: 0.002,
  accelMeasNoise: 0.05,
  gyroMeasNoise: 0.01,
  resampleThreshold: 0.5,
};

/** Simple gaussian for particle noise */
function gaussPF(sigma: number): number {
  const u1 = Math.random() || 1e-10;
  const u2 = Math.random();
  return Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2) * sigma;
}

export class ParticleFilter {
  private particles: Particle[];
  private readonly config: ParticleFilterConfig;
  private estimate: Float64Array; // [roll, pitch, yaw, wx, wy, wz]

  constructor(config: ParticleFilterConfig = DEFAULT_PF_CONFIG) {
    this.config = config;
    this.estimate = new Float64Array(6);
    this.particles = [];

    // Initialize particles around zero state
    for (let i = 0; i < config.numParticles; i++) {
      this.particles.push({
        state: new Float64Array(6), // All start at zero attitude
        weight: 1 / config.numParticles,
      });
    }
  }

  /**
   * PREDICT: Propagate each particle forward using the motion model.
   * Each particle integrates the gyro measurement + random process noise.
   * This is the "time evolution" of the quantum state.
   */
  predict(gyro: Float64Array, dt: number): void {
    for (const p of this.particles) {
      // Integrate gyro (with noise for each particle)
      p.state[0]! += (gyro[0]! + gaussPF(this.config.processNoise)) * dt; // roll
      p.state[1]! += (gyro[1]! + gaussPF(this.config.processNoise)) * dt; // pitch
      p.state[2]! += (gyro[2]! + gaussPF(this.config.processNoise)) * dt; // yaw

      // Store angular velocities (with noise)
      p.state[3] = gyro[0]! + gaussPF(this.config.gyroMeasNoise);
      p.state[4] = gyro[1]! + gaussPF(this.config.gyroMeasNoise);
      p.state[5] = gyro[2]! + gaussPF(this.config.gyroMeasNoise);
    }
  }

  /**
   * UPDATE: Reweight particles based on accelerometer measurement.
   * Particles whose predicted gravity direction matches the measured
   * acceleration get HIGHER weight. This is the "measurement collapse".
   *
   * The likelihood function: how well does this particle's attitude
   * explain the observed acceleration (which should be pure gravity
   * in body frame if the drone is not accelerating)?
   */
  update(accel: Float64Array): void {
    const aMag = Math.sqrt(accel[0]! ** 2 + accel[1]! ** 2 + accel[2]! ** 2);
    if (aMag < 0.1) return; // Free fall — can't update

    const ax = accel[0]! / aMag;
    const ay = accel[1]! / aMag;
    const az = accel[2]! / aMag;

    let weightSum = 0;

    for (const p of this.particles) {
      // Expected gravity direction from this particle's attitude
      const cr = Math.cos(p.state[0]!), sr = Math.sin(p.state[0]!);
      const cp = Math.cos(p.state[1]!), sp = Math.sin(p.state[1]!);

      const expectedGx = sp;
      const expectedGy = -sr * cp;
      const expectedGz = cr * cp;

      // Error = difference between measured and expected gravity
      const errX = ax - expectedGx;
      const errY = ay - expectedGy;
      const errZ = az - expectedGz;
      const errSq = errX * errX + errY * errY + errZ * errZ;

      // Gaussian likelihood
      const sigma2 = this.config.accelMeasNoise * this.config.accelMeasNoise * 2;
      p.weight *= Math.exp(-errSq / sigma2);

      weightSum += p.weight;
    }

    // Normalize weights
    if (weightSum > 1e-20) {
      for (const p of this.particles) p.weight /= weightSum;
    } else {
      // All weights collapsed — reinitialize uniformly
      for (const p of this.particles) p.weight = 1 / this.config.numParticles;
    }

    // Compute weighted estimate (COLLAPSE)
    this.estimate.fill(0);
    for (const p of this.particles) {
      for (let i = 0; i < 6; i++) {
        this.estimate[i]! += p.state[i]! * p.weight;
      }
    }

    // Resample if effective sample size is too low
    const nEff = this.effectiveSampleSize();
    if (nEff < this.config.numParticles * this.config.resampleThreshold) {
      this.resample();
    }
  }

  /** Get the current best estimate */
  getEstimate(): { roll: number; pitch: number; yaw: number; wx: number; wy: number; wz: number } {
    return {
      roll: this.estimate[0]!,
      pitch: this.estimate[1]!,
      yaw: this.estimate[2]!,
      wx: this.estimate[3]!,
      wy: this.estimate[4]!,
      wz: this.estimate[5]!,
    };
  }

  /** Effective sample size (measure of particle diversity) */
  effectiveSampleSize(): number {
    let sumSq = 0;
    for (const p of this.particles) sumSq += p.weight * p.weight;
    return sumSq > 0 ? 1 / sumSq : 0;
  }

  /** Get spread of particles (uncertainty measure) */
  getSpread(): number {
    let variance = 0;
    for (const p of this.particles) {
      const dr = p.state[0]! - this.estimate[0]!;
      const dp = p.state[1]! - this.estimate[1]!;
      variance += (dr * dr + dp * dp) * p.weight;
    }
    return Math.sqrt(variance);
  }

  // ─── Private ──────────────────────────────────────────

  /**
   * SYSTEMATIC RESAMPLING: Replace low-weight particles with
   * copies of high-weight particles (+ noise to maintain diversity).
   * This prevents particle depletion.
   */
  private resample(): void {
    const N = this.particles.length;
    const newParticles: Particle[] = [];

    // Cumulative weight distribution
    const cumWeights = new Float64Array(N);
    cumWeights[0] = this.particles[0]!.weight;
    for (let i = 1; i < N; i++) {
      cumWeights[i] = cumWeights[i - 1]! + this.particles[i]!.weight;
    }

    // Systematic resampling
    const step = 1 / N;
    let u = Math.random() * step;

    let j = 0;
    for (let i = 0; i < N; i++) {
      while (j < N - 1 && u > cumWeights[j]!) j++;

      // Copy particle with small noise (roughening)
      const newState = Float64Array.from(this.particles[j]!.state);
      for (let k = 0; k < 6; k++) {
        newState[k]! += gaussPF(this.config.processNoise * 0.5);
      }

      newParticles.push({
        state: newState,
        weight: 1 / N,
      });

      u += step;
    }

    this.particles = newParticles;
  }

  /** Reset the filter */
  reset(): void {
    for (const p of this.particles) {
      p.state.fill(0);
      p.weight = 1 / this.particles.length;
    }
    this.estimate.fill(0);
  }
}
