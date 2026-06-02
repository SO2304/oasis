/**
 * OASIS Kernel — Perception Bridge (P3)
 *
 * Connects the sensory system to the Tensorial Brain:
 * SensorFusion → WorldModel → TensionField → LatentEngine
 *
 * INNOVATION: Perception as Continuous World Deformation
 *
 * Traditional robotics: sense → plan → act (serial pipeline)
 * OASIS: sense → deform field → agents flow naturally (parallel, continuous)
 *
 * The Perception Bridge does NOT "plan". It:
 * 1. Ingests sensor readings
 * 2. Fuses them into a Perception Vector
 * 3. Updates the World Model (adds/removes pressure zones)
 * 4. Injects the perception as tension into the field
 * 5. R15: Detects black holes and spikes entropy
 *
 * The LatentEngine then naturally responds — agents' trajectories
 * adjust because the field they're flowing through has changed.
 * Like water finding a new path when you drop a rock in a stream.
 */

import type { AgentId, TenantId, DriverId } from '../types.js';
import { driverId, monotonicNow } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  scale,
  normalize,
  norm,
  cosineSimilarity,
} from '../physics/vector-math.js';
import { type HyperState, R14_ENTROPY_CRITICAL } from '../physics/hyper-state.js';
import { TensionField } from '../physics/tension-field.js';
import { WorldModel, ZoneType } from '../physics/world-model.js';
import { SensorFusion } from './sensor-fusion.js';
import type {
  SensorReading,
  SensorConfig,
  PerceptionVector,
  SensorModality,
} from './sensor-types.js';

// ─── Perception Events ──────────────────────────────────────────

export interface PerceptionTickResult {
  /** The fused perception vector */
  readonly perception: PerceptionVector;
  /** Zones created or updated in the world model */
  readonly zonesUpdated: number;
  /** R15 black holes detected (sensor dropouts) */
  readonly blackHoles: string[];
  /** Whether R15 triggered an entropy spike */
  readonly r15Triggered: boolean;
  /** Entropy injected into the field */
  readonly entropyInjected: number;
  /** Contradictions detected between sensors */
  readonly contradictionCount: number;
}

// ─── Perception Bridge ──────────────────────────────────────────

export class PerceptionBridge {
  private readonly fusion: SensorFusion;
  private readonly world: WorldModel;
  private readonly field: TensionField;
  private readonly dim: number;
  private readonly tenantId: TenantId;
  private readonly ownerAgentId: AgentId;
  private zoneCounter = 0;

  /** Entropy multiplier for R15 black holes */
  private readonly blackHoleEntropyMultiplier: number;

  constructor(
    ownerAgentId: AgentId,
    tenantId: TenantId,
    dim: number,
    field: TensionField,
    world?: WorldModel,
    blackHoleEntropyMultiplier = 0.4,
  ) {
    this.ownerAgentId = ownerAgentId;
    this.tenantId = tenantId;
    this.dim = dim;
    this.field = field;
    this.world = world ?? new WorldModel(dim);
    this.fusion = new SensorFusion(dim);
    this.blackHoleEntropyMultiplier = blackHoleEntropyMultiplier;
  }

  /** Register a sensor */
  registerSensor(config: SensorConfig): void {
    this.fusion.registerSensor(config);
  }

  /** Get the world model for inspection */
  getWorldModel(): WorldModel {
    return this.world;
  }

  /** Get the sensor fusion engine */
  getSensorFusion(): SensorFusion {
    return this.fusion;
  }

  /**
   * Ingest a raw sensor reading and project it into the world.
   * This is the "nerve impulse" — raw data becomes world deformation.
   */
  ingestReading(reading: SensorReading): void {
    this.fusion.ingest(reading);
  }

  /**
   * Execute one perception tick.
   *
   * This is the full perception pipeline:
   * 1. Fuse all sensor imprints
   * 2. Detect contradictions and black holes
   * 3. Update world model with new pressure zones
   * 4. Inject perception tension into the field
   * 5. R15: Spike entropy if sensors are missing
   */
  tick(): PerceptionTickResult {
    this.fusion.tick();

    // Step 1: Fuse all sensors
    const perception = this.fusion.fuse();

    // Step 2: Update world model from perception
    let zonesUpdated = 0;

    if (perception.magnitude > 0.01) {
      // Convert the fused perception into a pressure zone
      const zoneId = `perc-${++this.zoneCounter}`;

      if (perception.confidence > 0.5) {
        // High confidence → create/update repulsive zone (obstacle detected)
        const isRepulsive = perception.magnitude > 0.5;
        if (isRepulsive) {
          this.world.addObstacle(
            zoneId,
            perception.direction,
            perception.magnitude,
            perception.direction,
            `perception-obstacle-${this.zoneCounter}`,
          );
        } else {
          this.world.addGoal(
            zoneId,
            perception.direction,
            perception.magnitude,
            perception.direction,
            `perception-feature-${this.zoneCounter}`,
          );
        }
        zonesUpdated++;
      }
    }

    // Step 3: Inject perception as tension into the field
    if (norm(perception.direction) > 1e-10) {
      this.field.emit({
        source: this.ownerAgentId,
        tenantId: this.tenantId,
        force: scale(perception.direction, perception.magnitude * perception.confidence),
        signature: perception.direction,
        intensity: perception.confidence * 2,
        decayRate: 3,
        emittedAt: monotonicNow(),
        ticksRemaining: 10,
      });
    }

    // Step 4: R15 — Black hole detection and entropy injection
    let entropyInjected = 0;
    const r15Triggered = perception.blackHoleCount > 0;

    if (r15Triggered) {
      // Each black hole injects entropy proportional to its importance
      const entropyPerHole = this.blackHoleEntropyMultiplier;
      entropyInjected = perception.blackHoleCount * entropyPerHole;

      // Inject entropy tension — directionless, just uncertainty
      const entropySignature = zeros(this.dim);
      // Spread entropy across all dimensions (maximum uncertainty)
      for (let i = 0; i < Math.min(9, this.dim); i++) {
        entropySignature[i] = 1 / 3; // Equal distribution = max entropy
      }

      this.field.emit({
        source: this.ownerAgentId,
        tenantId: this.tenantId,
        force: zeros(this.dim), // No direction — pure entropy
        signature: normalize(entropySignature),
        intensity: entropyInjected * 5,
        decayRate: 2,
        emittedAt: monotonicNow(),
        ticksRemaining: 15,
      });

      // Also add entropy zones to world model at sensor locations
      for (let i = 0; i < perception.blackHoleCount; i++) {
        this.world.addUnknownRegion(
          `blackhole-${this.zoneCounter}-${i}`,
          zeros(this.dim), // Unknown position — center of space
          entropyPerHole * 2,
          `sensor-dropout-${i}`,
        );
        zonesUpdated++;
      }
    }

    // Step 5: Handle contradictions — they also increase local entropy
    if (perception.contradictions.length > 0) {
      for (const contradiction of perception.contradictions) {
        // Inject contradiction as opposing tensions (creates turbulence)
        this.field.emit({
          source: this.ownerAgentId,
          tenantId: this.tenantId,
          force: scale(contradiction.conflictAxis, contradiction.severity),
          signature: contradiction.conflictAxis,
          intensity: contradiction.severity * 3,
          decayRate: 5,
          emittedAt: monotonicNow(),
          ticksRemaining: 8,
        });

        entropyInjected += contradiction.severity * 0.1;
      }
    }

    this.world.tick();

    return {
      perception,
      zonesUpdated,
      blackHoles: [], // Sensor IDs handled internally
      r15Triggered,
      entropyInjected,
      contradictionCount: perception.contradictions.length,
    };
  }
}
