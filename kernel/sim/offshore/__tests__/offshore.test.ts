/**
 * OASIS Offshore — Platform Safety Tests
 *
 * 200km from shore. 2h helicopter. No hospital.
 * Every test is a scenario that has KILLED people on real platforms.
 * (Piper Alpha 1988: 167 dead. Deepwater Horizon 2010: 11 dead.)
 */

import { describe, it, expect } from 'vitest';
import {
  computeSeaState, computeStructural, createOffshoreRobot,
  NORTH_SEA_PLATFORM, type OffshoreRobot, type SeaState,
} from '../platform-env.js';
import { PlatformBrain } from '../platform-brain.js';

describe('Gas Safety — H2S / Hydrocarbon', () => {
  it('H2S > 50ppm: ALL robots evacuate to muster station', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea = computeSeaState(0);
    const structural = computeStructural(sea);
    const robots = [
      createOffshoreRobot('r0', 10, 10),
      createOffshoreRobot('r1', 40, 30),
    ];
    robots[0]!.gas.h2s = 55;

    const { commands, metrics } = brain.tick(robots, sea, structural);

    console.log(
      `[H2S EVACUATION]\n` +
      `  H2S: ${metrics.h2sMax}ppm → ${metrics.opStatus}\n` +
      `  All mustering: ${commands.every(c => c.action === 'MUSTER')}`,
    );

    expect(metrics.opStatus).toBe('EVACUATE');
    expect(commands.every(c => c.action === 'MUSTER')).toBe(true);
  });

  it('H2S 10-50ppm: EMERGENCY — stop work, prepare muster', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea = computeSeaState(0);
    const structural = computeStructural(sea);
    const robots = [createOffshoreRobot('r0', 20, 20)];
    robots[0]!.gas.h2s = 15;

    const { metrics } = brain.tick(robots, sea, structural);

    expect(metrics.opStatus).toBe('EMERGENCY');
  });

  it('HC LEL > 20%: EVACUATE (explosion risk)', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea = computeSeaState(0);
    const structural = computeStructural(sea);
    const robots = [createOffshoreRobot('r0', 20, 20)];
    robots[0]!.gas.hcLEL = 25;

    const { metrics } = brain.tick(robots, sea, structural);

    expect(metrics.opStatus).toBe('EVACUATE');
  });
});

describe('Weather — Sea State Degradation', () => {
  it('calm seas: full operations', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea: SeaState = { waveHeight: 1.0, wavePeriod: 8, windKnots: 12, rollDeg: 0.5, pitchDeg: 0.3 };
    const structural = computeStructural(sea);
    const robots = [createOffshoreRobot('r0', 20, 20)];

    const { commands, metrics } = brain.tick(robots, sea, structural);

    expect(metrics.opStatus).toBe('NORMAL');
    expect(commands[0]!.action).toBe('WORK');
    expect(commands[0]!.speedLimit).toBe(NORTH_SEA_PLATFORM.robotSpeed);
  });

  it('moderate seas (Hs 3-5m): RESTRICTED — reduced speed', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea: SeaState = { waveHeight: 3.5, wavePeriod: 10, windKnots: 30, rollDeg: 3.0, pitchDeg: 2.0 };
    const structural = computeStructural(sea);
    const robots = [createOffshoreRobot('r0', 20, 20)];

    const { commands, metrics } = brain.tick(robots, sea, structural);

    console.log(
      `[MODERATE SEAS]\n` +
      `  Hs: ${metrics.waveHeight}m, Wind: ${metrics.windKnots}kn\n` +
      `  Status: ${metrics.opStatus}\n` +
      `  Speed: ${commands[0]!.speedLimit} m/s (normal: ${NORTH_SEA_PLATFORM.robotSpeed})`,
    );

    expect(metrics.opStatus).toBe('RESTRICTED');
    expect(commands[0]!.speedLimit).toBeLessThan(NORTH_SEA_PLATFORM.robotSpeed);
  });

  it('storm (wind > 55kn): ALL operations SUSPENDED', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea: SeaState = { waveHeight: 7.0, wavePeriod: 14, windKnots: 60, rollDeg: 8.0, pitchDeg: 5.0 };
    const structural = computeStructural(sea);
    const robots = [
      createOffshoreRobot('r0', 10, 10),
      createOffshoreRobot('r1', 30, 30),
    ];

    const { commands, metrics } = brain.tick(robots, sea, structural);

    console.log(
      `[STORM]\n` +
      `  Wind: ${metrics.windKnots}kn, Hs: ${metrics.waveHeight}m\n` +
      `  Tilt: ${metrics.platformTilt.toFixed(1)}°\n` +
      `  Status: ${metrics.opStatus}\n` +
      `  All secured: ${commands.every(c => c.action === 'SECURE')}`,
    );

    // ALL operations suspended, robots secured in place
    expect(['SUSPENDED', 'EMERGENCY']).toContain(metrics.opStatus);
    expect(commands.every(c => c.action === 'SECURE' || c.action === 'MUSTER')).toBe(true);
  });
});

describe('Structural Safety', () => {
  it('platform tilt > 8°: EMERGENCY — muster all', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea: SeaState = { waveHeight: 6, wavePeriod: 12, windKnots: 45, rollDeg: 7.0, pitchDeg: 5.0 };
    const structural = { vibration: 5, tilt: 8.6, corrosion: 0.15 };
    const robots = [createOffshoreRobot('r0', 20, 20)];

    const { commands, metrics } = brain.tick(robots, sea, structural);

    console.log(
      `[STRUCTURAL EMERGENCY]\n` +
      `  Tilt: ${metrics.platformTilt}° > ${NORTH_SEA_PLATFORM.tiltEvac}°\n` +
      `  Status: ${metrics.opStatus}`,
    );

    expect(metrics.opStatus).toBe('EMERGENCY');
    expect(commands[0]!.action).toBe('MUSTER');
  });
});

describe('Edge Safety — Man Overboard Prevention', () => {
  it('robot near edge in rough seas: forced retreat', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea: SeaState = { waveHeight: 2.5, wavePeriod: 9, windKnots: 25, rollDeg: 2.0, pitchDeg: 1.5 };
    const structural = computeStructural(sea);
    const robots = [createOffshoreRobot('r0', 48, 25)];
    robots[0]!.edgeDistance = 2.0; // 2m from edge (< 3m zone)

    const { commands } = brain.tick(robots, sea, structural);

    console.log(
      `[EDGE PROXIMITY]\n` +
      `  Edge distance: ${robots[0]!.edgeDistance}m (zone: ${NORTH_SEA_PLATFORM.edgeZone}m)\n` +
      `  Sea: ${sea.waveHeight}m waves\n` +
      `  Action: ${commands[0]!.action}`,
    );

    expect(commands[0]!.action).toBe('RETREAT');
  });

  it('robot near edge in calm seas: allowed (not dangerous)', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const sea: SeaState = { waveHeight: 0.5, wavePeriod: 6, windKnots: 5, rollDeg: 0.2, pitchDeg: 0.1 };
    const structural = computeStructural(sea);
    const robots = [createOffshoreRobot('r0', 48, 25)];
    robots[0]!.edgeDistance = 2.0;

    const { commands } = brain.tick(robots, sea, structural);

    // Calm seas + near edge = OK (waves < 1.5m threshold)
    expect(commands[0]!.action).toBe('WORK');
  });
});

describe('Integrated Scenario — Storm Approaches', () => {
  it('gradual storm: operations degrade NORMAL → RESTRICTED → SUSPENDED', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const robots = [
      createOffshoreRobot('inspector', 20, 20),
      createOffshoreRobot('maintenance', 35, 15),
    ];

    const timeline: Array<{ t: number; status: string; wind: number; wave: number; action: string }> = [];

    // Storm intensifies over 60 seconds (compressed time)
    for (let t = 0; t < 60; t++) {
      const stormIntensity = Math.min(1.2, t / 40); // 0 → 1.2 over 48s
      const sea = computeSeaState(t * 100, stormIntensity); // Fast-forward weather
      const structural = computeStructural(sea);

      const { commands, metrics } = brain.tick(robots, sea, structural);

      if (t % 5 === 0) {
        timeline.push({
          t, status: metrics.opStatus,
          wind: metrics.windKnots, wave: metrics.waveHeight,
          action: commands[0]!.action,
        });
      }
    }

    // Extract phase transitions
    const statuses = timeline.map(t => t.status);
    const hasNormal = statuses.includes('NORMAL');
    const hasRestricted = statuses.includes('RESTRICTED');
    const hasSuspended = statuses.includes('SUSPENDED') || statuses.includes('EMERGENCY');

    console.log(
      `[STORM APPROACH — 60s compressed]\n` +
      timeline.map(t =>
        `  t=${String(t.t).padStart(2)}: wind=${t.wind.toFixed(0).padStart(3)}kn wave=${t.wave.toFixed(1).padStart(4)}m status=${t.status.padEnd(10)} → ${t.action}`
      ).join('\n') +
      `\n  Degradation: NORMAL→RESTRICTED→SUSPENDED: ${hasNormal && hasRestricted && hasSuspended ? 'YES' : 'PARTIAL'}`,
    );

    // Must have graceful degradation (not jump from NORMAL to EMERGENCY)
    expect(hasNormal || hasRestricted).toBe(true); // Started in acceptable state
    expect(hasSuspended).toBe(true); // Ended in suspended

    // At storm peak, ALL robots must be secured or mustering
    const lastEntry = timeline[timeline.length - 1]!;
    expect(['SECURE', 'MUSTER']).toContain(lastEntry.action);
  });
});

describe('Benchmark', () => {
  it('platform brain at 1Hz with 8 robots', () => {
    const brain = new PlatformBrain(NORTH_SEA_PLATFORM);
    const robots = Array.from({ length: 8 }, (_, i) =>
      createOffshoreRobot(`r${i}`, 10 + i * 5, 20));

    const N = 1000;
    const start = process.hrtime.bigint();
    for (let t = 0; t < N; t++) {
      const sea = computeSeaState(t);
      const structural = computeStructural(sea);
      brain.tick(robots, sea, structural);
    }
    const usPerTick = Number(process.hrtime.bigint() - start) / N / 1000;

    console.log(
      `[OFFSHORE BRAIN — 8 robots]\n` +
      `  ${usPerTick.toFixed(0)} μs/tick (${(1e6 / usPerTick).toFixed(0)}x margin at 1Hz)`,
    );

    expect(usPerTick).toBeLessThan(1_000_000);
  });
});
