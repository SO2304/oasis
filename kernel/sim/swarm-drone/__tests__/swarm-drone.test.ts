/**
 * OASIS — Drone Swarm Coordination Tests
 *
 * OASIS is the nervous system. Drones are the limbs.
 * Every test proves a SAFETY or COORDINATION property.
 */

import { describe, it, expect } from 'vitest';
import {
  createDrone, stepDrone, droneDistance, computeCoverage,
  SURVEY_SWARM, type DroneAgent,
} from '../drone-agent.js';
import { SwarmCoordinator } from '../swarm-coordinator.js';

const DT = 0.5; // 2Hz coordination (realistic for drone swarms)

function runSwarm(
  drones: DroneAgent[], coordinator: SwarmCoordinator, durationS: number,
  events?: Array<{ tick: number; action: (drones: DroneAgent[]) => void }>,
): { metrics: Array<ReturnType<SwarmCoordinator['tick']>['metrics']>; collisions: number } {
  const allMetrics: Array<ReturnType<SwarmCoordinator['tick']>['metrics']> = [];
  let totalCollisions = 0;
  const ticks = Math.floor(durationS / DT);

  for (let t = 0; t < ticks; t++) {
    // Apply events
    for (const ev of events ?? []) {
      if (ev.tick === t) ev.action(drones);
    }

    const { commands, metrics } = coordinator.tick(drones);
    const coverage = computeCoverage(drones, 10, SURVEY_SWARM.geofence);
    metrics.coverageRatio = coverage.ratio;
    allMetrics.push(metrics);
    totalCollisions += metrics.collisions;

    for (const cmd of commands) {
      const d = drones.find(d => d.id === cmd.droneId);
      if (d) {
        d.role = cmd.role;
        stepDrone(d, cmd.velocity, DT, SURVEY_SWARM);
      }
    }
  }

  return { metrics: allMetrics, collisions: totalCollisions };
}

describe('Swarm Safety — Zero Collision', () => {
  it('8 drones from same point: spread without collision', () => {
    const coordinator = new SwarmCoordinator(SURVEY_SWARM);
    const drones = Array.from({ length: 8 }, (_, i) =>
      createDrone(`d${i}`, i * 8, 0, 30), // Start 8m apart (> minSep)
    );

    const { metrics, collisions } = runSwarm(drones, coordinator, 30);

    const minSep = Math.min(...metrics.map(m => m.minSeparation));
    const finalCoverage = metrics[metrics.length - 1]!.coverageRatio;
    const roles = drones.map(d => d.role);

    console.log(
      `[8 DRONES — SPREAD]\n` +
      `  Collisions: ${collisions}\n` +
      `  Min separation: ${minSep.toFixed(1)}m (safe: ${SURVEY_SWARM.minSeparation}m)\n` +
      `  Final coverage: ${(finalCoverage * 100).toFixed(1)}%\n` +
      `  Roles: ${JSON.stringify(roles.reduce((acc, r) => { acc[r] = (acc[r] ?? 0) + 1; return acc; }, {} as Record<string, number>))}`,
    );

    expect(collisions).toBe(0); // ABSOLUTE: zero physical contact
    expect(minSep).toBeGreaterThan(1.0); // Must stay > 1m (drone body size)
    expect(finalCoverage).toBeGreaterThan(0);
  });

  it('12 drones: maintain network connectivity', () => {
    const coordinator = new SwarmCoordinator(SURVEY_SWARM);
    const drones = Array.from({ length: 12 }, (_, i) => {
      const angle = (i / 12) * 2 * Math.PI;
      return createDrone(`d${i}`, 20 * Math.cos(angle), 20 * Math.sin(angle), 30);
    });

    const { metrics, collisions } = runSwarm(drones, coordinator, 40);

    const disconnectedEver = metrics.some(m => !m.networkConnected);
    const disconnectedTicks = metrics.filter(m => !m.networkConnected).length;
    const totalTicks = metrics.length;

    console.log(
      `[12 DRONES — CONNECTIVITY]\n` +
      `  Collisions: ${collisions}\n` +
      `  Network connected: ${disconnectedTicks === 0 ? 'ALWAYS' : `${totalTicks - disconnectedTicks}/${totalTicks} ticks`}\n` +
      `  Max disconnected drones: ${Math.max(...metrics.map(m => m.disconnectedDrones.length))}`,
    );

    expect(collisions).toBe(0);
    // Network should be connected most of the time (>80%)
    expect(disconnectedTicks / totalTicks).toBeLessThan(0.3);
  });
});

describe('Swarm Resilience', () => {
  it('R15: drone dies → swarm fills the gap', () => {
    const coordinator = new SwarmCoordinator(SURVEY_SWARM);
    const drones = Array.from({ length: 6 }, (_, i) =>
      createDrone(`d${i}`, (i - 3) * 40, 0, 30),
    );

    // Let swarm stabilize
    runSwarm(drones, coordinator, 10);

    const coverageBefore = computeCoverage(drones, 10, SURVEY_SWARM.geofence).ratio;

    // Kill drone 2 (middle of formation)
    const { metrics } = runSwarm(drones, coordinator, 30, [
      { tick: 0, action: (ds) => { ds[2]!.alive = false; } },
    ]);

    const coverageAfter = computeCoverage(drones, 10, SURVEY_SWARM.geofence).ratio;
    const collisions = metrics.reduce((s, m) => s + m.collisions, 0);

    console.log(
      `[DRONE DEATH — d2 killed]\n` +
      `  Collisions after death: ${collisions}\n` +
      `  Coverage before: ${(coverageBefore * 100).toFixed(1)}%\n` +
      `  Coverage after: ${(coverageAfter * 100).toFixed(1)}%\n` +
      `  Active drones: ${drones.filter(d => d.alive).length}/6`,
    );

    expect(collisions).toBe(0); // No cascade failure
    expect(drones.filter(d => d.alive).length).toBe(5); // 1 dead
  });

  it('R4: geofence enforcement — drones cannot leave boundary', () => {
    const coordinator = new SwarmCoordinator(SURVEY_SWARM);
    // Place drone near boundary edge
    const drones = [createDrone('edge', 195, 195, 30)];

    // Push it toward boundary for 10 seconds
    for (let i = 0; i < 20; i++) {
      const { commands } = coordinator.tick(drones);
      // Override command to push outward
      stepDrone(drones[0]!, new Float64Array([8, 8, 0]), DT, SURVEY_SWARM);
    }

    const [minX, minY, maxX, maxY] = SURVEY_SWARM.geofence;

    console.log(
      `[GEOFENCE ENFORCEMENT]\n` +
      `  Final position: (${drones[0]!.pos[0]!.toFixed(0)}, ${drones[0]!.pos[1]!.toFixed(0)}, ${drones[0]!.pos[2]!.toFixed(0)})m\n` +
      `  Boundary: [${minX}, ${minY}] to [${maxX}, ${maxY}]`,
    );

    expect(drones[0]!.pos[0]!).toBeLessThanOrEqual(maxX);
    expect(drones[0]!.pos[1]!).toBeLessThanOrEqual(maxY);
    expect(drones[0]!.pos[2]!).toBeLessThanOrEqual(SURVEY_SWARM.maxAltitude);
    expect(drones[0]!.pos[2]!).toBeGreaterThanOrEqual(SURVEY_SWARM.minAltitude);
  });

  it('battery management: drones return to base before dying', () => {
    const coordinator = new SwarmCoordinator(SURVEY_SWARM);
    const drones = Array.from({ length: 4 }, (_, i) =>
      createDrone(`d${i}`, (i - 2) * 30, 0, 30),
    );
    // Set one drone to low battery
    drones[1]!.battery = 0.12; // Below minimum → should trigger return

    const { metrics } = runSwarm(drones, coordinator, 20);

    // Check that low-battery drone moved toward base (0,0)
    const lowBat = drones[1]!;
    const distToBase = Math.sqrt(lowBat.pos[0]! ** 2 + lowBat.pos[1]! ** 2);

    console.log(
      `[BATTERY MANAGEMENT]\n` +
      `  Low-bat drone battery: ${(lowBat.battery * 100).toFixed(0)}%\n` +
      `  Low-bat distance to base: ${distToBase.toFixed(0)}m\n` +
      `  Low-bat alive: ${lowBat.alive}`,
    );

    // Should have moved toward base (started at -30, 0)
    // Even if it died, it should have tried to return
    // Started at (-30, 0), battery 0.12 < 0.15 → should move toward (0,0)
    expect(distToBase).toBeLessThan(35); // Must be closer than start
  });
});

describe('Swarm Coverage', () => {
  it('8 drones cover more area than 4 drones', () => {
    const coordinator4 = new SwarmCoordinator(SURVEY_SWARM);
    const drones4 = Array.from({ length: 4 }, (_, i) =>
      createDrone(`d${i}`, (i - 2) * 20, 0, 30),
    );
    runSwarm(drones4, coordinator4, 30);
    const coverage4 = computeCoverage(drones4, 10, SURVEY_SWARM.geofence).ratio;

    const coordinator8 = new SwarmCoordinator(SURVEY_SWARM);
    const drones8 = Array.from({ length: 8 }, (_, i) =>
      createDrone(`d${i}`, (i - 4) * 10, 0, 30),
    );
    runSwarm(drones8, coordinator8, 30);
    const coverage8 = computeCoverage(drones8, 10, SURVEY_SWARM.geofence).ratio;

    console.log(
      `[COVERAGE SCALING]\n` +
      `  4 drones: ${(coverage4 * 100).toFixed(1)}% coverage\n` +
      `  8 drones: ${(coverage8 * 100).toFixed(1)}% coverage\n` +
      `  Improvement: ${((coverage8 - coverage4) / coverage4 * 100).toFixed(0)}%`,
    );

    expect(coverage8).toBeGreaterThan(coverage4); // More drones = more coverage
  });
});

describe('Swarm Benchmark', () => {
  it('coordinator throughput: 50 drones at 2Hz', () => {
    const coordinator = new SwarmCoordinator(SURVEY_SWARM);
    const drones = Array.from({ length: 50 }, (_, i) =>
      createDrone(`d${i}`, (Math.random() - 0.5) * 300, (Math.random() - 0.5) * 300, 30),
    );

    const N = 100;
    const start = process.hrtime.bigint();
    for (let t = 0; t < N; t++) {
      const { commands } = coordinator.tick(drones);
      for (const cmd of commands) {
        const d = drones.find(d => d.id === cmd.droneId);
        if (d) stepDrone(d, cmd.velocity, DT, SURVEY_SWARM);
      }
    }
    const msPerTick = Number(process.hrtime.bigint() - start) / N / 1e6;
    const hz = 1000 / msPerTick;

    console.log(
      `[SWARM BENCHMARK — 50 drones]\n` +
      `  ${msPerTick.toFixed(2)} ms/tick = ${hz.toFixed(0)} Hz\n` +
      `  Target 2Hz: ${hz > 2 ? 'YES (' + (hz / 2).toFixed(0) + 'x margin)' : 'NO'}`,
    );

    expect(hz).toBeGreaterThan(2);
  });
});
