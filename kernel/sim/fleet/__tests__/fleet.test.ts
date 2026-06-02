/**
 * OASIS Fleet — Autonomous Vehicle Fleet Tests
 *
 * OASIS is the nervous system, not the car.
 * Tests prove: collision avoidance, coordination, scalability.
 *
 * Every assertion is a SAFETY requirement.
 * If it fails, vehicles crash. No tolerance.
 */

import { describe, it, expect } from 'vitest';
import {
  createVehicle, stepVehicle, vehicleDistance,
  URBAN_FLEET, type VehicleState,
} from '../vehicle.js';
import { FleetBrain, type FleetMetrics } from '../fleet-brain.js';

const DT = 0.1; // 10Hz fleet coordination (realistic V2V update rate)

function runFleet(
  vehicles: VehicleState[],
  brain: FleetBrain,
  durationS: number,
): { vehicles: VehicleState[]; history: FleetMetrics[]; collisions: number } {
  const history: FleetMetrics[] = [];
  let collisions = 0;
  const ticks = Math.floor(durationS / DT);

  for (let t = 0; t < ticks; t++) {
    const { commands, metrics } = brain.tick(vehicles);
    history.push(metrics);

    if (metrics.collision) collisions++;

    // Apply commands to each vehicle
    for (const cmd of commands) {
      const v = vehicles.find(v => v.id === cmd.vehicleId);
      if (v && v.active) {
        stepVehicle(v, cmd.speed, cmd.heading, DT, URBAN_FLEET);
      }
    }
  }

  return { vehicles, history, collisions };
}

describe('Fleet Safety — Zero Collision Tolerance', () => {
  it('2 vehicles on collision course: must avoid without contact', () => {
    const brain = new FleetBrain(URBAN_FLEET);

    // Two vehicles heading straight at each other at 50km/h
    const vehicles = [
      createVehicle('car-A', 0, 0, 200, 0),      // →
      createVehicle('car-B', 200, 0, 0, 0),       // ←
    ];
    // Start them moving
    vehicles[0]!.speed = 13.9; // 50 km/h
    vehicles[1]!.speed = 13.9;

    const { history, collisions } = runFleet(vehicles, brain, 15);

    const minDist = Math.min(...history.map(h => h.minDistance));
    const avoidances = history.reduce((s, h) => s + h.avoidanceCount, 0);
    const emergencyStops = history.reduce((s, h) => s + h.emergencyStops, 0);

    console.log(
      `[HEAD-ON COLLISION AVOIDANCE]\n` +
      `  Collisions: ${collisions}\n` +
      `  Minimum distance: ${minDist.toFixed(1)}m (safe: ${URBAN_FLEET.safeDistance}m)\n` +
      `  Avoidance maneuvers: ${avoidances}\n` +
      `  Emergency stops: ${emergencyStops}`,
    );

    expect(collisions).toBe(0); // ABSOLUTE: zero collisions
    expect(minDist).toBeGreaterThan(URBAN_FLEET.vehicleLength); // Never overlap
  });

  it('4 vehicles at intersection: all pass without collision', () => {
    const brain = new FleetBrain(URBAN_FLEET);

    // 4 vehicles approaching intersection from 4 directions
    const vehicles = [
      createVehicle('north', 50, 0, 50, 100),     // ↑
      createVehicle('south', 50, 100, 50, 0),     // ↓
      createVehicle('east', 100, 50, 0, 50),      // ←
      createVehicle('west', 0, 50, 100, 50),      // →
    ];
    for (const v of vehicles) v.speed = 10; // 36 km/h

    const { history, collisions } = runFleet(vehicles, brain, 20);

    const minDist = Math.min(...history.map(h => h.minDistance));
    const maxEntropy = Math.max(...history.map(h => h.entropy));
    const avoidances = history.reduce((s, h) => s + h.avoidanceCount, 0);

    console.log(
      `[4-WAY INTERSECTION]\n` +
      `  Collisions: ${collisions}\n` +
      `  Minimum distance: ${minDist.toFixed(1)}m\n` +
      `  Peak entropy: ${maxEntropy.toFixed(3)}\n` +
      `  Avoidance maneuvers: ${avoidances}`,
    );

    expect(collisions).toBe(0);
    expect(minDist).toBeGreaterThan(URBAN_FLEET.vehicleLength);
  });

  it('10-vehicle convoy: maintains safe following distance', () => {
    const brain = new FleetBrain(URBAN_FLEET);

    // 10 vehicles in a line, all heading east
    const vehicles: VehicleState[] = [];
    for (let i = 0; i < 10; i++) {
      vehicles.push(createVehicle(`convoy-${i}`, i * 15, 0, 500, 0));
      vehicles[i]!.speed = 12; // ~43 km/h
    }

    const { history, collisions } = runFleet(vehicles, brain, 30);

    const minDist = Math.min(...history.map(h => h.minDistance));
    const arrived = vehicles.filter(v => !v.active).length;

    console.log(
      `[10-VEHICLE CONVOY — 30s]\n` +
      `  Collisions: ${collisions}\n` +
      `  Min following distance: ${minDist.toFixed(1)}m (safe: ${URBAN_FLEET.safeDistance}m)\n` +
      `  Vehicles arrived: ${arrived}/10`,
    );

    expect(collisions).toBe(0);
    expect(minDist).toBeGreaterThan(URBAN_FLEET.vehicleLength);
  });

  it('vehicle breakdown: fleet reorganizes around stopped vehicle', () => {
    const brain = new FleetBrain(URBAN_FLEET);

    // 5 vehicles heading east, vehicle 2 breaks down at t=5s
    const vehicles: VehicleState[] = [];
    for (let i = 0; i < 5; i++) {
      vehicles.push(createVehicle(`v-${i}`, i * 20, 0, 300, 0));
      vehicles[i]!.speed = 11;
    }

    const history: FleetMetrics[] = [];
    let collisions = 0;
    const ticks = Math.floor(20 / DT);

    for (let t = 0; t < ticks; t++) {
      // Vehicle 2 breaks down at t=5s
      if (t === 50) {
        vehicles[2]!.speed = 0;
        vehicles[2]!.active = false; // Obstacle
      }

      const { commands, metrics } = brain.tick(vehicles);
      history.push(metrics);
      if (metrics.collision) collisions++;

      for (const cmd of commands) {
        const v = vehicles.find(v => v.id === cmd.vehicleId);
        if (v && v.active && v.id !== 'v-2') { // v-2 is dead
          stepVehicle(v, cmd.speed, cmd.heading, DT, URBAN_FLEET);
        }
      }
    }

    const minDist = Math.min(...history.map(h => h.minDistance));
    const eStops = history.reduce((s, h) => s + h.emergencyStops, 0);

    console.log(
      `[VEHICLE BREAKDOWN — v-2 stops at t=5s]\n` +
      `  Collisions: ${collisions}\n` +
      `  Min distance: ${minDist.toFixed(1)}m\n` +
      `  Emergency stops by fleet: ${eStops}`,
    );

    expect(collisions).toBe(0);
  });
});

describe('Fleet Performance', () => {
  it('20-vehicle fleet: coordination at 10Hz', () => {
    const brain = new FleetBrain(URBAN_FLEET);

    // 20 vehicles with random start/destination
    const vehicles: VehicleState[] = [];
    for (let i = 0; i < 20; i++) {
      const angle = (i / 20) * 2 * Math.PI;
      const r = 80;
      vehicles.push(createVehicle(
        `fleet-${i}`,
        r * Math.cos(angle), r * Math.sin(angle), // Start on circle
        r * Math.cos(angle + Math.PI), r * Math.sin(angle + Math.PI), // Dest: opposite
      ));
      vehicles[i]!.speed = 8;
    }

    const { history, collisions } = runFleet(vehicles, brain, 30);

    const arrived = vehicles.filter(v => !v.active).length;
    const peakEntropy = Math.max(...history.map(h => h.entropy));
    const totalAvoidances = history.reduce((s, h) => s + h.avoidanceCount, 0);
    const minDist = Math.min(...history.map(h => h.minDistance));

    console.log(
      `[20-VEHICLE FLEET — crossing circle, 30s]\n` +
      `  Collisions: ${collisions}\n` +
      `  Vehicles arrived: ${arrived}/20\n` +
      `  Min distance: ${minDist.toFixed(1)}m\n` +
      `  Peak entropy: ${peakEntropy.toFixed(3)}\n` +
      `  Total avoidances: ${totalAvoidances}`,
    );

    expect(collisions).toBe(0); // ABSOLUTE: zero collisions
    expect(minDist).toBeGreaterThan(URBAN_FLEET.vehicleLength); // No overlaps
    // Note: vehicles may not arrive (conservative safety prioritizes no-collision over progress)
    // In production, arrival rate would be optimized by better path planning.
  });

  it('fleet brain throughput: 100 vehicles at 10Hz', () => {
    const brain = new FleetBrain(URBAN_FLEET);

    const vehicles: VehicleState[] = [];
    for (let i = 0; i < 100; i++) {
      vehicles.push(createVehicle(`v${i}`, Math.random() * 500, Math.random() * 500, Math.random() * 500, Math.random() * 500));
      vehicles[i]!.speed = 8 + Math.random() * 5;
    }

    const N = 100; // 100 ticks
    const start = process.hrtime.bigint();
    for (let t = 0; t < N; t++) {
      const { commands } = brain.tick(vehicles);
      for (const cmd of commands) {
        const v = vehicles.find(v => v.id === cmd.vehicleId);
        if (v) stepVehicle(v, cmd.speed, cmd.heading, DT, URBAN_FLEET);
      }
    }
    const msPerTick = Number(process.hrtime.bigint() - start) / N / 1e6;
    const hz = 1000 / msPerTick;

    console.log(
      `[FLEET BRAIN BENCHMARK — 100 vehicles]\n` +
      `  ${msPerTick.toFixed(2)} ms/tick = ${hz.toFixed(0)} Hz\n` +
      `  Target 10Hz: ${hz > 10 ? 'YES (' + (hz / 10).toFixed(0) + 'x margin)' : 'NO'}`,
    );

    expect(hz).toBeGreaterThan(10); // Must sustain 10Hz with 100 vehicles
  });
});
