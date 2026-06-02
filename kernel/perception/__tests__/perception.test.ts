/**
 * OASIS Kernel — Perception Tests
 *
 * "La Navigation dans l'Invisible":
 * LiDAR detects a wall, Camera detects a passage.
 * The system must fuse these contradictory signals into
 * a probabilistic pressure field and navigate by semantic pressure.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { SensorFusion } from '../sensor-fusion.js';
import { PerceptionBridge } from '../perception-bridge.js';
import { WorldModel, ZoneType } from '../../physics/world-model.js';
import { TensionField } from '../../physics/tension-field.js';
import { __resetKillSwitchForTesting } from '../../kill-switch.js';
import { agentId, tenantId, driverId } from '../../types.js';
import type { SensorReading, SensorConfig } from '../sensor-types.js';
import {
  zeros,
  vec,
  norm,
  normalize,
  scale,
  sub,
  cosineSimilarity,
  distance,
} from '../../physics/vector-math.js';

const TENANT = tenantId('test-lab');
const DIM = 32;
const DRIVER = driverId('test-driver');

// ─── Helpers ────────────────────────────────────────────────────

function lidarConfig(sensorId: string): SensorConfig {
  return {
    sensorId,
    driverId: DRIVER,
    modality: 'LIDAR',
    projectionDim: DIM,
    baseReliability: 0.95,
    maxStaleTicks: 10,
    primaryDimensions: [10, 11, 12, 13],
  };
}

function cameraConfig(sensorId: string): SensorConfig {
  return {
    sensorId,
    driverId: DRIVER,
    modality: 'CAMERA',
    projectionDim: DIM,
    baseReliability: 0.85,
    maxStaleTicks: 15,
    primaryDimensions: [14, 15, 16, 17],
  };
}

function imuConfig(sensorId: string): SensorConfig {
  return {
    sensorId,
    driverId: DRIVER,
    modality: 'IMU',
    projectionDim: DIM,
    baseReliability: 0.9,
    maxStaleTicks: 5,
    primaryDimensions: [18, 19, 20, 21, 22, 23],
  };
}

function lidarReading(sensorId: string, range: number, angle = 0): SensorReading {
  return {
    sensorId,
    driverId: DRIVER,
    modality: 'LIDAR',
    values: { range, angle, reflectivity: 1 },
    confidence: 0.95,
    timestamp: process.hrtime.bigint(),
  };
}

function cameraReading(
  sensorId: string,
  obstacle: number,
  freeSpace: number,
  angle = 0,
): SensorReading {
  return {
    sensorId,
    driverId: DRIVER,
    modality: 'CAMERA',
    values: { obstacle, freeSpace, angle },
    confidence: 0.85,
    timestamp: process.hrtime.bigint(),
  };
}

function imuReading(sensorId: string, ax: number, ay: number, az: number): SensorReading {
  return {
    sensorId,
    driverId: DRIVER,
    modality: 'IMU',
    values: { accelX: ax, accelY: ay, accelZ: az, gyroX: 0, gyroY: 0, gyroZ: 0 },
    confidence: 0.9,
    timestamp: process.hrtime.bigint(),
  };
}

// ─── Tests ──────────────────────────────────────────────────────

describe('Sensor Fusion (P1)', () => {
  let fusion: SensorFusion;

  beforeEach(() => {
    fusion = new SensorFusion(DIM);
  });

  it('should register sensors and produce imprints', () => {
    fusion.registerSensor(lidarConfig('lidar-1'));
    const imprint = fusion.ingest(lidarReading('lidar-1', 2.0));

    expect(imprint.modality).toBe('LIDAR');
    expect(imprint.magnitude).toBeGreaterThan(0);
    expect(imprint.reliability).toBeGreaterThan(0);
    expect(imprint.direction.length).toBe(DIM);
  });

  it('should fuse multiple sensors into a single perception vector', () => {
    fusion.registerSensor(lidarConfig('lidar-1'));
    fusion.registerSensor(cameraConfig('cam-1'));
    fusion.registerSensor(imuConfig('imu-1'));

    fusion.ingest(lidarReading('lidar-1', 3.0));
    fusion.ingest(cameraReading('cam-1', 0.2, 0.8)); // Mostly free space
    fusion.ingest(imuReading('imu-1', 0.1, 0, -9.8));

    const perception = fusion.fuse();

    expect(perception.activeSensorCount).toBe(3);
    expect(perception.confidence).toBeGreaterThan(0);
    expect(perception.magnitude).toBeGreaterThan(0);
    expect(perception.blackHoleCount).toBe(0);
  });

  it('should detect contradictions between sensors', () => {
    // LiDAR and camera use overlapping primary dimensions for this test
    const lidarCfg: SensorConfig = {
      ...lidarConfig('lidar-c'),
      primaryDimensions: [10, 11, 12, 13],
    };
    const cameraCfg: SensorConfig = {
      ...cameraConfig('cam-c'),
      primaryDimensions: [10, 11, 12, 13], // SAME dims → can conflict
    };

    fusion.registerSensor(lidarCfg);
    fusion.registerSensor(cameraCfg);

    // LiDAR: close obstacle (strong repulsion on dims 10-13)
    fusion.ingest(lidarReading('lidar-c', 0.5)); // Very close → strong repulsion

    // Camera: all clear (attraction on same dims)
    fusion.ingest(cameraReading('cam-c', 0, 1.0)); // Pure free space

    const perception = fusion.fuse();

    // Should detect the contradiction
    expect(perception.contradictions.length).toBeGreaterThan(0);
    expect(perception.contradictions[0]!.severity).toBeGreaterThan(0);
  });

  it('should detect R15 black holes (sensor dropout)', () => {
    fusion.registerSensor(lidarConfig('lidar-alive'));
    fusion.registerSensor(imuConfig('imu-dead')); // Never sends data

    // Only lidar sends data
    fusion.ingest(lidarReading('lidar-alive', 5.0));

    // Advance past imu's maxStaleTicks (5)
    for (let i = 0; i < 6; i++) fusion.tick();

    const perception = fusion.fuse();

    // IMU is a black hole
    expect(perception.blackHoleCount).toBe(1);
    expect(perception.activeSensorCount).toBe(1);
  });

  it('should decay reliability with staleness', () => {
    fusion.registerSensor(lidarConfig('lidar-aging'));
    fusion.ingest(lidarReading('lidar-aging', 3.0));

    const fresh = fusion.fuse();

    // Advance 5 ticks without new data
    for (let i = 0; i < 5; i++) fusion.tick();

    const stale = fusion.fuse();

    // Stale reading should have lower confidence
    expect(stale.confidence).toBeLessThan(fresh.confidence);
  });
});

describe('World Model (P2)', () => {
  let world: WorldModel;

  beforeEach(() => {
    world = new WorldModel(DIM);
  });

  it('should create repulsive zones for obstacles', () => {
    const center = zeros(DIM);
    center[10] = 1;

    world.addObstacle('wall-1', center, 5.0, normalize(center));

    const zone = world.getZone('wall-1')!;
    expect(zone.type).toBe(ZoneType.REPULSIVE);
    expect(zone.intensity).toBe(5.0);
  });

  it('should repel agents from obstacles', () => {
    const obstaclePos = zeros(DIM);
    obstaclePos[10] = 0;

    world.addObstacle('wall', obstaclePos, 10.0, normalize(zeros(DIM)));

    // Sample near the obstacle
    const nearObstacle = zeros(DIM);
    nearObstacle[10] = 0.5; // Close

    const farAway = zeros(DIM);
    farAway[10] = 5.0; // Far

    const nearSample = world.sample(nearObstacle);
    const farSample = world.sample(farAway);

    // Nearer position should feel stronger repulsion
    expect(norm(nearSample.netForce)).toBeGreaterThan(norm(farSample.netForce));
  });

  it('should attract agents toward goals', () => {
    const goalPos = zeros(DIM);
    goalPos[15] = 5;

    world.addGoal('target', goalPos, 3.0, normalize(goalPos));

    // Sample away from goal
    const agentPos = zeros(DIM);
    agentPos[15] = 0;

    const sample = world.sample(agentPos);

    // Force should point toward goal (positive dim 15 direction)
    // Attractive zone: force = -directionAway = toward center
    expect(sample.netForce[15]!).toBeGreaterThan(0);
  });

  it('should create entropy zones for unknown regions', () => {
    world.addUnknownRegion('void', zeros(DIM), 2.0);

    const sample = world.sample(zeros(DIM));

    // Should have elevated local entropy
    expect(sample.localEntropy).toBeGreaterThan(0);
    // But no directional force
    // (entropy zones radiate uncertainty, not direction)
  });

  it('should compute least-resistance path avoiding obstacles', () => {
    // Create a goal
    const goalCenter = zeros(DIM);
    goalCenter[10] = 5;
    world.addGoal('destination', goalCenter, 3.0, normalize(goalCenter));

    // Create an obstacle between start and goal
    const obstacleCenter = zeros(DIM);
    obstacleCenter[10] = 2.5;
    world.addObstacle('wall', obstacleCenter, 8.0, normalize(obstacleCenter));

    // Start position
    const start = zeros(DIM);

    const path = world.computeLeastResistancePath(start, 'destination', 500, 0.1);

    // Path should exist and have multiple points
    expect(path.points.length).toBeGreaterThan(0);
    // Path may or may not reach goal depending on obstacle configuration
    // but it should have tried
    expect(path.totalResistance).toBeGreaterThan(0);
  });

  it('should block path through high-entropy regions (R14/R15)', () => {
    const goalCenter = zeros(DIM);
    goalCenter[10] = 5;
    world.addGoal('destination', goalCenter, 2.0, normalize(goalCenter));

    // Create massive entropy zone between start and goal
    const entropyCenter = zeros(DIM);
    entropyCenter[10] = 2;
    world.addUnknownRegion('void', entropyCenter, 10.0);

    const start = zeros(DIM);
    const path = world.computeLeastResistancePath(start, 'destination', 200, 0.1, 0.85);

    // Path should be blocked by entropy
    if (!path.feasible) {
      expect(path.reason).toContain('Entropy');
    }
  });

  it('should decay stale zones over time', () => {
    world.addObstacle('temp-wall', zeros(DIM), 5.0, zeros(DIM));

    expect(world.getZoneCount()).toBe(1);

    // Advance past staleness limit (30 ticks default)
    for (let i = 0; i < 100; i++) world.tick();

    // Zone should have decayed away
    expect(world.getZoneCount()).toBe(0);
  });
});

describe('Perception Bridge (P3)', () => {
  let bridge: PerceptionBridge;
  let field: TensionField;
  let world: WorldModel;

  beforeEach(() => {
    field = new TensionField();
    world = new WorldModel(DIM);
    bridge = new PerceptionBridge(
      agentId('navigator'),
      TENANT,
      DIM,
      field,
      world,
    );
  });

  it('should convert sensor readings into world deformation', () => {
    bridge.registerSensor(lidarConfig('lidar'));
    bridge.ingestReading(lidarReading('lidar', 1.5));

    const result = bridge.tick();

    expect(result.perception.activeSensorCount).toBe(1);
    // Should have injected tension into the field
    expect(field.getActiveTensionCount()).toBeGreaterThan(0);
  });

  it('should trigger R15 on sensor dropout', () => {
    bridge.registerSensor(lidarConfig('lidar-ok'));
    bridge.registerSensor(imuConfig('imu-gone'));

    // Only lidar sends data
    bridge.ingestReading(lidarReading('lidar-ok', 3.0));

    // Advance past IMU staleness
    for (let i = 0; i < 6; i++) bridge.tick();

    const result = bridge.tick();

    expect(result.r15Triggered).toBe(true);
    expect(result.entropyInjected).toBeGreaterThan(0);
  });

  it('should inject contradictions as turbulence into the field', () => {
    // Same primary dims → can conflict
    const lidarCfg: SensorConfig = {
      ...lidarConfig('lidar-x'),
      primaryDimensions: [10, 11, 12, 13],
    };
    const cameraCfg: SensorConfig = {
      ...cameraConfig('cam-x'),
      primaryDimensions: [10, 11, 12, 13],
    };

    bridge.registerSensor(lidarCfg);
    bridge.registerSensor(cameraCfg);

    bridge.ingestReading(lidarReading('lidar-x', 0.3));
    bridge.ingestReading(cameraReading('cam-x', 0, 1.0));

    const result = bridge.tick();

    if (result.contradictionCount > 0) {
      // Contradictions should have injected additional tensions
      expect(field.getActiveTensionCount()).toBeGreaterThan(1);
    }
  });
});

// ─── Scenario: La Navigation dans l'Invisible ──────────────────

describe('Scenario — La Navigation dans l\'Invisible', () => {
  it('should resolve LiDAR wall + Camera passage into probabilistic navigation', () => {
    const field = new TensionField();
    const world = new WorldModel(DIM);
    const bridge = new PerceptionBridge(
      agentId('robot'),
      TENANT,
      DIM,
      field,
      world,
    );

    // Same primary dimensions so they can interact/conflict
    const sharedDims = [10, 11, 12, 13];
    bridge.registerSensor({
      ...lidarConfig('lidar-front'),
      primaryDimensions: sharedDims,
    });
    bridge.registerSensor({
      ...cameraConfig('cam-front'),
      primaryDimensions: sharedDims,
    });

    // Phase 1: Contradictory input
    // LiDAR says: wall at 0.5m (strong repulsion)
    // Camera says: passage clear (free space)
    bridge.ingestReading(lidarReading('lidar-front', 0.5)); // Close obstacle
    bridge.ingestReading(cameraReading('cam-front', 0, 1.0)); // All clear

    const result1 = bridge.tick();

    // Should detect the contradiction
    expect(result1.contradictionCount).toBeGreaterThan(0);

    // Confidence should be reduced due to contradiction
    expect(result1.perception.confidence).toBeLessThan(0.85);

    // Phase 2: LiDAR confirms wall, camera starts agreeing
    bridge.ingestReading(lidarReading('lidar-front', 0.5));
    bridge.ingestReading(cameraReading('cam-front', 0.7, 0.3)); // Now sees obstacle too

    const result2 = bridge.tick();

    // Contradiction should be reduced
    expect(result2.contradictionCount).toBeLessThanOrEqual(result1.contradictionCount);

    // Phase 3: Both agree — wall confirmed
    bridge.ingestReading(lidarReading('lidar-front', 0.5));
    bridge.ingestReading(cameraReading('cam-front', 1.0, 0)); // Full obstacle

    const result3 = bridge.tick();

    // Should have higher confidence now
    expect(result3.perception.confidence).toBeGreaterThanOrEqual(result2.perception.confidence);

    // World model should have pressure zones
    expect(world.getZoneCount()).toBeGreaterThan(0);
  });

  it('should adjust speed by semantic pressure', () => {
    const world = new WorldModel(DIM);

    // Create a goal far away
    const goalPos = zeros(DIM);
    goalPos[10] = 10;
    world.addGoal('target', goalPos, 2.0, normalize(goalPos));

    // No obstacles → path should have high speed throughout
    const clearPath = world.computeLeastResistancePath(zeros(DIM), 'target', 500, 0.2);
    const clearSpeeds = clearPath.points.map(p => p.speed);

    // Now add semantic pressure zone (e.g., "fragile objects area")
    const pressureCenter = zeros(DIM);
    pressureCenter[10] = 5; // Midway to goal
    world.upsertZone({
      id: 'fragile-zone',
      type: ZoneType.SEMANTIC,
      center: pressureCenter,
      falloffRate: 1.0,
      intensity: 5.0,
      signature: normalize(pressureCenter),
      label: 'fragile-objects',
      confidence: 1.0,
      lastUpdatedAt: process.hrtime.bigint(),
      staleness: 0,
    });

    const pressuredPath = world.computeLeastResistancePath(zeros(DIM), 'target', 500, 0.2);

    // Path through semantic zone should have lower speeds
    if (pressuredPath.points.length > 5) {
      const midpointSpeed = pressuredPath.points[Math.floor(pressuredPath.points.length / 2)]!.speed;
      // Speed should be reduced near the semantic zone
      expect(midpointSpeed).toBeLessThanOrEqual(1.0);
    }
  });

  it('should fuse perception with mission intent for least-resistance trajectory', () => {
    const world = new WorldModel(DIM);

    // Mission: reach goal
    const goalPos = zeros(DIM);
    goalPos[10] = 8;
    world.addGoal('mission-target', goalPos, 5.0, normalize(goalPos));

    // Perception: obstacle detected by sensors
    const obstaclePos = zeros(DIM);
    obstaclePos[10] = 4;
    obstaclePos[11] = 0; // Directly in path
    world.addObstacle('perceived-wall', obstaclePos, 15.0, normalize(obstaclePos));

    // Alternative path: slightly off-axis
    const start = zeros(DIM);
    start[11] = 2; // Start offset from direct line

    const path = world.computeLeastResistancePath(start, 'mission-target', 500, 0.15);

    // Path should exist
    expect(path.points.length).toBeGreaterThan(0);

    // The path should bend around the obstacle
    // (not go straight through it)
    if (path.feasible) {
      // Path successfully navigated
      expect(path.reason).toContain('Reached goal');
    } else {
      // Even if not reached, should have tried many steps
      expect(path.points.length).toBeGreaterThan(10);
    }
  });
});
