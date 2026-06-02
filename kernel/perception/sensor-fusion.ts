/**
 * OASIS Kernel — Sensor Fusion (P1)
 *
 * Fuses raw sensor streams into a single Perception Vector
 * that deforms the Tension Field.
 *
 * INNOVATION: Bayesian Imprint Fusion
 *
 * Traditional sensor fusion (Kalman, particle filters) works in
 * Euclidean space. OASIS fuses in LATENT space:
 *
 * 1. Each sensor reading is PROJECTED into latent space via
 *    modality-specific projection functions (learned or configured)
 * 2. Projections become IMPRINTS — directional forces with reliability
 * 3. Imprints are FUSED using reliability-weighted superposition
 * 4. Contradictions are detected as destructive interference
 * 5. Sensor dropout triggers R15 (entropy spike)
 *
 * The output is NOT a "best estimate of world state".
 * It's a FORCE that pushes the agent's HyperState through
 * the latent manifold — perception as pressure.
 */

import { monotonicNow, nsToMs } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  sub,
  scale,
  norm,
  normalize,
  dot,
  cosineSimilarity,
  weightedCentroid,
  randomUnit,
} from '../physics/vector-math.js';
import type {
  SensorReading,
  SensorImprint,
  SensorConfig,
  PerceptionVector,
  PerceptionContradiction,
  SensorModality,
} from './sensor-types.js';

// ─── Projection Functions ───────────────────────────────────────

/**
 * Project a raw sensor reading into latent space.
 *
 * Each modality has a different projection:
 * - CAMERA: spatial features → angular direction in latent space
 * - LIDAR: range/angle → repulsive force magnitude
 * - IMU: acceleration → momentum perturbation
 * - PROXIMITY: distance → repulsive intensity (inverse square)
 */
function projectReading(reading: SensorReading, config: SensorConfig, dim: number): SensorImprint {
  const direction = zeros(dim);
  let magnitude = 0;

  switch (reading.modality) {
    case 'CAMERA': {
      // Camera detects objects → creates directional pressure
      // "freeSpace" pushes toward that direction (attractive)
      // "obstacle" pushes away (repulsive)
      const obstacle = reading.values['obstacle'] ?? 0;
      const freeSpace = reading.values['freeSpace'] ?? 0;
      const angle = reading.values['angle'] ?? 0;

      // Project into primary dimensions
      for (let i = 0; i < config.primaryDimensions.length; i++) {
        const d = config.primaryDimensions[i]!;
        if (d < dim) {
          // Attractive for free space, repulsive for obstacles
          direction[d] = (freeSpace - obstacle) * Math.cos(angle + i * 0.5);
        }
      }
      magnitude = Math.abs(freeSpace - obstacle);
      break;
    }

    case 'LIDAR': {
      // LiDAR detects range → inverse-square repulsion from obstacles
      const range = reading.values['range'] ?? 10;
      const angle = reading.values['angle'] ?? 0;
      const reflectivity = reading.values['reflectivity'] ?? 1;

      // Closer = stronger repulsion (inverse square law)
      const repulsion = range > 0.1 ? 1 / (range * range) : 100;

      for (let i = 0; i < config.primaryDimensions.length; i++) {
        const d = config.primaryDimensions[i]!;
        if (d < dim) {
          direction[d] = -repulsion * Math.cos(angle + i * 0.3) * reflectivity;
        }
      }
      magnitude = repulsion;
      break;
    }

    case 'IMU': {
      // IMU measures acceleration → direct momentum perturbation
      const accelX = reading.values['accelX'] ?? 0;
      const accelY = reading.values['accelY'] ?? 0;
      const accelZ = reading.values['accelZ'] ?? 0;
      const gyroX = reading.values['gyroX'] ?? 0;
      const gyroY = reading.values['gyroY'] ?? 0;
      const gyroZ = reading.values['gyroZ'] ?? 0;

      const dims = config.primaryDimensions;
      if (dims[0] !== undefined && dims[0] < dim) direction[dims[0]] = accelX;
      if (dims[1] !== undefined && dims[1] < dim) direction[dims[1]] = accelY;
      if (dims[2] !== undefined && dims[2] < dim) direction[dims[2]] = accelZ;
      if (dims[3] !== undefined && dims[3] < dim) direction[dims[3]] = gyroX * 0.5;
      if (dims[4] !== undefined && dims[4] < dim) direction[dims[4]] = gyroY * 0.5;
      if (dims[5] !== undefined && dims[5] < dim) direction[dims[5]] = gyroZ * 0.5;

      magnitude = Math.sqrt(accelX ** 2 + accelY ** 2 + accelZ ** 2);
      break;
    }

    case 'PROXIMITY': {
      // Proximity → strong repulsive field at close range
      const distance = reading.values['distance'] ?? 10;
      const repulsion = distance > 0.05 ? 1 / distance : 20;

      for (const d of config.primaryDimensions) {
        if (d < dim) direction[d] = -repulsion;
      }
      magnitude = repulsion;
      break;
    }

    default: {
      // Generic: spread values across primary dimensions
      const vals = Object.values(reading.values);
      for (let i = 0; i < Math.min(vals.length, config.primaryDimensions.length); i++) {
        const d = config.primaryDimensions[i]!;
        if (d < dim) direction[d] = vals[i]!;
      }
      magnitude = norm(direction);
    }
  }

  // Build semantic signature from modality + primary dims
  const signature = zeros(dim);
  for (const d of config.primaryDimensions) {
    if (d < dim) signature[d] = 1;
  }

  return {
    sensorId: reading.sensorId,
    modality: reading.modality,
    direction: norm(direction) > 0 ? normalize(direction) : direction,
    magnitude,
    reliability: reading.confidence * config.baseReliability,
    signature: norm(signature) > 0 ? normalize(signature) : signature,
    timestamp: reading.timestamp,
    age: 0,
  };
}

// ─── Sensor Fusion Engine ───────────────────────────────────────

export class SensorFusion {
  private readonly sensors = new Map<string, SensorConfig>();
  private readonly latestImprints = new Map<string, SensorImprint>();
  private readonly lastSeenTick = new Map<string, number>();
  private readonly dim: number;
  private tickCount = 0;

  /** Contradiction threshold — cosine similarity below this = contradiction */
  private readonly contradictionThreshold: number;

  constructor(dim: number, contradictionThreshold = -0.3) {
    this.dim = dim;
    this.contradictionThreshold = contradictionThreshold;
  }

  /** Register a sensor */
  registerSensor(config: SensorConfig): void {
    this.sensors.set(config.sensorId, config);
    this.lastSeenTick.set(config.sensorId, this.tickCount);
  }

  /** Unregister a sensor */
  unregisterSensor(sensorId: string): void {
    this.sensors.delete(sensorId);
    this.latestImprints.delete(sensorId);
    this.lastSeenTick.delete(sensorId);
  }

  /**
   * Ingest a raw sensor reading.
   * Projects it into latent space and stores as the latest imprint.
   */
  ingest(reading: SensorReading): SensorImprint {
    const config = this.sensors.get(reading.sensorId);
    if (!config) throw new Error(`Unknown sensor: ${reading.sensorId}`);

    const imprint = projectReading(reading, config, this.dim);
    this.latestImprints.set(reading.sensorId, imprint);
    this.lastSeenTick.set(reading.sensorId, this.tickCount);
    return imprint;
  }

  /**
   * FUSE all active sensor imprints into a single Perception Vector.
   *
   * This is the core perception computation:
   * 1. Age all imprints and decay reliability
   * 2. Detect R15 black holes (missing sensors)
   * 3. Detect contradictions (destructive interference)
   * 4. Weighted superposition of all imprints
   */
  fuse(): PerceptionVector {
    const activeImprints: SensorImprint[] = [];
    const blackHoles: string[] = [];
    const modalityGroups = new Map<SensorModality, SensorImprint[]>();

    // Step 1: Collect active imprints, detect black holes (R15)
    for (const [sensorId, config] of this.sensors) {
      const imprint = this.latestImprints.get(sensorId);
      const lastSeen = this.lastSeenTick.get(sensorId) ?? 0;
      const staleness = this.tickCount - lastSeen;

      if (!imprint || staleness > config.maxStaleTicks) {
        // R15: Sensor has gone dark — information black hole
        blackHoles.push(sensorId);
        continue;
      }

      // Age-decay the reliability
      const decayFactor = Math.max(0, 1 - (staleness / config.maxStaleTicks) * 0.5);
      const agedImprint: SensorImprint = {
        ...imprint,
        reliability: imprint.reliability * decayFactor,
        age: staleness,
      };

      activeImprints.push(agedImprint);

      // Group by modality
      const group = modalityGroups.get(config.modality) ?? [];
      group.push(agedImprint);
      modalityGroups.set(config.modality, group);
    }

    // Step 2: Detect contradictions
    const contradictions = this.detectContradictions(activeImprints);

    // Step 3: Compute per-modality agreement
    const modalityAgreement = new Map<SensorModality, number>();
    for (const [modality, group] of modalityGroups) {
      if (group.length < 2) {
        modalityAgreement.set(modality, 1.0);
        continue;
      }
      // Average pairwise cosine similarity
      let totalSim = 0;
      let pairs = 0;
      for (let i = 0; i < group.length; i++) {
        for (let j = i + 1; j < group.length; j++) {
          totalSim += cosineSimilarity(group[i]!.direction, group[j]!.direction);
          pairs++;
        }
      }
      modalityAgreement.set(modality, pairs > 0 ? (totalSim / pairs + 1) / 2 : 1);
    }

    // Step 4: Weighted superposition
    if (activeImprints.length === 0) {
      return {
        direction: zeros(this.dim),
        magnitude: 0,
        confidence: 0,
        modalityAgreement,
        contradictions,
        activeSensorCount: 0,
        blackHoleCount: blackHoles.length,
        timestamp: monotonicNow(),
      };
    }

    // Reduce reliability for sensors involved in contradictions
    const reliabilityPenalties = new Map<string, number>();
    for (const c of contradictions) {
      const penaltyA = reliabilityPenalties.get(c.sensorA) ?? 0;
      const penaltyB = reliabilityPenalties.get(c.sensorB) ?? 0;
      reliabilityPenalties.set(c.sensorA, penaltyA + c.severity * 0.3);
      reliabilityPenalties.set(c.sensorB, penaltyB + c.severity * 0.3);
    }

    const directions: Vec[] = [];
    const weights: number[] = [];
    let totalReliability = 0;

    for (const imprint of activeImprints) {
      const penalty = reliabilityPenalties.get(imprint.sensorId) ?? 0;
      const effectiveReliability = Math.max(0.01, imprint.reliability - penalty);
      const weight = effectiveReliability * imprint.magnitude;

      directions.push(imprint.direction);
      weights.push(weight);
      totalReliability += effectiveReliability;
    }

    const fusedDirection = weightedCentroid(directions, weights);
    const fusedMagnitude = norm(fusedDirection);
    const confidence = activeImprints.length > 0
      ? (totalReliability / activeImprints.length) *
        (1 - blackHoles.length / Math.max(1, this.sensors.size)) *
        (1 - contradictions.length * 0.15)
      : 0;

    return {
      direction: fusedMagnitude > 0 ? normalize(fusedDirection) : fusedDirection,
      magnitude: fusedMagnitude,
      confidence: Math.max(0, Math.min(1, confidence)),
      modalityAgreement,
      contradictions,
      activeSensorCount: activeImprints.length,
      blackHoleCount: blackHoles.length,
      timestamp: monotonicNow(),
    };
  }

  /** Advance one tick — ages all imprints */
  tick(): void {
    this.tickCount++;
  }

  /** Get current tick */
  getTickCount(): number {
    return this.tickCount;
  }

  /** Get registered sensor count */
  getSensorCount(): number {
    return this.sensors.size;
  }

  // ─── Private ────────────────────────────────────────────────

  private detectContradictions(imprints: SensorImprint[]): PerceptionContradiction[] {
    const contradictions: PerceptionContradiction[] = [];

    for (let i = 0; i < imprints.length; i++) {
      for (let j = i + 1; j < imprints.length; j++) {
        const a = imprints[i]!;
        const b = imprints[j]!;

        // Only check cross-modality or same-modality with significant magnitude
        if (a.magnitude < 0.01 || b.magnitude < 0.01) continue;

        const similarity = cosineSimilarity(a.direction, b.direction);

        if (similarity < this.contradictionThreshold) {
          // These sensors are saying opposite things
          const severity = Math.abs(similarity); // 0 = orthogonal, 1 = perfectly opposite
          contradictions.push({
            sensorA: a.sensorId,
            modalityA: a.modality,
            sensorB: b.sensorId,
            modalityB: b.modality,
            severity,
            conflictAxis: normalize(sub(a.direction, b.direction)),
          });
        }
      }
    }

    return contradictions;
  }
}
