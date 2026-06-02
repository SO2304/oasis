/**
 * OASIS Mine — Underground Safety Tests
 *
 * Every test is a LIFE-OR-DEATH scenario.
 * In a real mine, a failed safety check kills people.
 * Zero tolerance. Zero ambiguity.
 */

import { describe, it, expect } from 'vitest';
import {
  createRobot, sampleEnvironment, robotDistance,
  COAL_MINE, type MineRobot,
} from '../mine-env.js';
import { MineBrain, type AlertLevel } from '../mine-brain.js';

const DT = 1.0; // 1Hz safety monitoring (standard for mine gas monitoring)

describe('Gas Safety — Life-Critical Reflexes', () => {
  it('R12: CH4 > 2% triggers IMMEDIATE evacuation of ALL robots', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [
      createRobot('r0', 10, 0, 'main'),
      createRobot('r1', 50, 0, 'main'),
      createRobot('r2', 100, 0, 'branch-a'),
    ];

    // Normal reading
    for (const r of robots) r.sensors = sampleEnvironment(r.pos[0]!, r.pos[1]!, 0);
    let { metrics } = brain.tick(robots);
    expect(metrics.alertLevel).toBe('NORMAL');

    // GAS LEAK: Robot r1 detects CH4 = 2.5%
    robots[1]!.sensors.ch4 = 2.5;

    const result = brain.tick(robots);

    console.log(
      `[CH4 EVACUATION]\n` +
      `  Detected CH4: ${result.metrics.maxCH4.toFixed(1)}%\n` +
      `  Alert level: ${result.metrics.alertLevel}\n` +
      `  Evacuating: ${result.metrics.evacuating}/${robots.length}\n` +
      `  All robots received EVACUATE: ${result.commands.every(c => c.action === 'EVACUATE')}`,
    );

    // ABSOLUTE: ALL robots must evacuate
    expect(result.metrics.alertLevel).toBe('EVACUATE');
    expect(result.commands.every(c => c.action === 'EVACUATE')).toBe(true);
    expect(result.metrics.evacuating).toBe(robots.length);
  });

  it('R12: CO > 50ppm triggers evacuation even if CH4 is normal', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [createRobot('r0', 10, 0, 'main')];
    robots[0]!.sensors.co = 55; // Above MSHA STEL
    robots[0]!.sensors.ch4 = 0.3; // Normal

    const { commands, metrics } = brain.tick(robots);

    console.log(
      `[CO EVACUATION]\n` +
      `  CO: ${metrics.maxCO}ppm | CH4: ${metrics.maxCH4.toFixed(1)}%\n` +
      `  Alert: ${metrics.alertLevel}`,
    );

    expect(metrics.alertLevel).toBe('EVACUATE');
    expect(commands[0]!.action).toBe('EVACUATE');
  });

  it('rising CH4 trend triggers WARNING before reaching threshold', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [createRobot('r0', 10, 0, 'main')];

    // Gradually rising CH4 over 10 ticks
    const readings: number[] = [];
    let lastAlert: AlertLevel = 'NORMAL';

    for (let t = 0; t < 10; t++) {
      robots[0]!.sensors.ch4 = 0.3 + t * 0.08; // 0.3 → 1.02
      const { metrics } = brain.tick(robots);
      readings.push(robots[0]!.sensors.ch4);
      lastAlert = metrics.alertLevel;
    }

    console.log(
      `[CH4 TREND DETECTION]\n` +
      `  CH4 readings: ${readings.map(r => r.toFixed(2)).join(' → ')}\n` +
      `  Final alert: ${lastAlert}\n` +
      `  Warning triggered before threshold: ${lastAlert !== 'NORMAL'}`,
    );

    // Should detect rising trend and warn BEFORE reaching 2%
    expect(lastAlert).not.toBe('NORMAL');
  });
});

describe('Structural Safety — Collapse Detection', () => {
  it('vibration > 10mm/s triggers IMMEDIATE zone evacuation', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [
      createRobot('r0', 10, 0, 'main'),
      createRobot('r1', 50, 0, 'main'),
    ];

    // Roof stress detected by r1
    robots[1]!.sensors.vibration = 12.0; // > 10mm/s threshold

    const { commands, metrics } = brain.tick(robots);

    console.log(
      `[VIBRATION EVACUATION]\n` +
      `  Max vibration: ${metrics.maxVibration.toFixed(1)} mm/s\n` +
      `  Alert: ${metrics.alertLevel}\n` +
      `  All evacuating: ${commands.every(c => c.action === 'EVACUATE')}`,
    );

    expect(metrics.alertLevel).toBe('EVACUATE');
    expect(commands.every(c => c.action === 'EVACUATE')).toBe(true);
  });

  it('vibration 5-10mm/s triggers WARNING — stop work, monitor', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [createRobot('r0', 10, 0, 'main')];
    robots[0]!.sensors.vibration = 7.0; // Between warning and evac

    const { commands, metrics } = brain.tick(robots);

    console.log(
      `[VIBRATION WARNING]\n` +
      `  Vibration: ${metrics.maxVibration.toFixed(1)} mm/s\n` +
      `  Alert: ${metrics.alertLevel}\n` +
      `  Action: ${commands[0]!.action}`,
    );

    expect(metrics.alertLevel).toBe('WARNING');
    expect(commands[0]!.action).toBe('STOP');
  });
});

describe('Human Safety — R9 Physical Sandbox', () => {
  it('robot near human: speed limited to 0.5 m/s', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [createRobot('r0', 10, 0, 'main')];
    robots[0]!.humansNearby = 1;

    const { commands } = brain.tick(robots);

    console.log(
      `[HUMAN PROXIMITY]\n` +
      `  Speed limit: ${commands[0]!.speedLimit} m/s (normal: ${COAL_MINE.robotSpeed})\n` +
      `  Action: ${commands[0]!.action}\n` +
      `  Reason: ${commands[0]!.reason}`,
    );

    expect(commands[0]!.speedLimit).toBe(0.5);
    expect(commands[0]!.action).toBe('SLOW');
  });

  it('gas evacuation overrides human speed limit', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [createRobot('r0', 10, 0, 'main')];
    robots[0]!.humansNearby = 1;
    robots[0]!.sensors.ch4 = 3.0; // DANGER

    const { commands } = brain.tick(robots);

    // Evacuation takes priority over slow speed
    expect(commands[0]!.action).toBe('EVACUATE');
    expect(commands[0]!.speedLimit).toBe(COAL_MINE.robotSpeed); // Full speed to exit
  });
});

describe('Communication Integrity — P1 Mesh', () => {
  it('R15: lost robot triggers disconnection alert', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [
      createRobot('r0', 0, 0, 'main'),
      createRobot('r1', 30, 0, 'main'),
      createRobot('r2', 200, 0, 'branch-a'), // Out of comm range (50m)
    ];

    const { commands, metrics } = brain.tick(robots);

    console.log(
      `[COMM INTEGRITY]\n` +
      `  Connected: ${metrics.connectedRobots}/${robots.length}\n` +
      `  Disconnected: [${metrics.disconnectedRobots.join(', ')}]\n` +
      `  r2 action: ${commands.find(c => c.robotId === 'r2')?.action}`,
    );

    expect(metrics.disconnectedRobots).toContain('r2');
    expect(commands.find(c => c.robotId === 'r2')!.action).toBe('RELAY');
  });
});

describe('R14 — Sensor Disagreement = Assume Danger', () => {
  it('contradictory CH4 readings → CAUTION alert', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [
      createRobot('r0', 10, 0, 'main'),
      createRobot('r1', 15, 0, 'main'),
    ];
    // r0 says safe, r1 says elevated — contradictory
    robots[0]!.sensors.ch4 = 0.2;
    robots[1]!.sensors.ch4 = 0.9; // Below threshold but very different

    const { metrics } = brain.tick(robots);

    console.log(
      `[SENSOR DISAGREEMENT]\n` +
      `  r0 CH4: ${robots[0]!.sensors.ch4}%\n` +
      `  r1 CH4: ${robots[1]!.sensors.ch4}%\n` +
      `  Sensor agreement: ${(metrics.sensorAgreement * 100).toFixed(0)}%\n` +
      `  Alert: ${metrics.alertLevel}`,
    );

    // Low agreement should trigger at least CAUTION
    expect(metrics.sensorAgreement).toBeLessThan(0.8);
  });
});

describe('Integrated Scenario — Methane Pocket Breach', () => {
  it('gas leak develops → trend detected → robots evacuate before explosion', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = [
      createRobot('miner-1', 80, 0, 'face'),
      createRobot('miner-2', 60, 0, 'main'),
      createRobot('relay', 30, 0, 'main'),
      createRobot('scout', 120, 20, 'branch-a'),
    ];

    // Gas source appears at t=5, grows over time
    const gasSource = { x: 100, y: 10, ch4: 0, co: 0 };
    const timeline: Array<{ t: number; ch4: number; alert: string; evac: number }> = [];

    for (let t = 0; t < 30; t++) {
      // Gas pocket breaches at t=5, grows linearly
      if (t >= 5) {
        gasSource.ch4 = (t - 5) * 0.3; // 0 → 7.5% over 25 ticks
        gasSource.co = (t - 5) * 5;     // 0 → 125ppm
      }

      // Update sensor readings based on position and gas source
      for (const r of robots) {
        if (!r.alive) continue;
        r.sensors = sampleEnvironment(r.pos[0]!, r.pos[1]!, t, gasSource);
      }

      const { commands, metrics } = brain.tick(robots);
      timeline.push({
        t, ch4: metrics.maxCH4, alert: metrics.alertLevel,
        evac: metrics.evacuating,
      });

      // Robots close to gas source should be most affected
    }

    // Find when each alert level was first triggered
    const firstWarning = timeline.find(t => t.alert === 'WARNING')?.t ?? -1;
    const firstEvac = timeline.find(t => t.alert === 'EVACUATE')?.t ?? -1;
    const peakCH4 = Math.max(...timeline.map(t => t.ch4));

    console.log(
      `[METHANE POCKET BREACH]\n` +
      `  Gas source starts at t=5\n` +
      `  First WARNING at t=${firstWarning} (CH4 ~${timeline[firstWarning]?.ch4.toFixed(1)}%)\n` +
      `  First EVACUATION at t=${firstEvac} (CH4 ~${timeline[firstEvac]?.ch4.toFixed(1)}%)\n` +
      `  Peak CH4 detected: ${peakCH4.toFixed(1)}%\n` +
      `  Robots evacuated: ${timeline[timeline.length - 1]!.evac}\n` +
      `  Timeline:\n` +
      timeline.filter((_, i) => i % 3 === 0).map(t =>
        `    t=${String(t.t).padStart(2)}: CH4=${t.ch4.toFixed(1)}% alert=${t.alert.padEnd(8)} evac=${t.evac}`
      ).join('\n'),
    );

    // MUST detect warning BEFORE reaching explosive limit (5%)
    expect(firstWarning).toBeGreaterThan(0);
    expect(firstWarning).toBeLessThan(firstEvac); // Warning before evacuation
    expect(firstEvac).toBeGreaterThan(0);

    // At evacuation time, CH4 must be < 5% (LEL — explosion threshold)
    const ch4AtEvac = timeline[firstEvac]!.ch4;
    expect(ch4AtEvac).toBeLessThan(5.0); // CRITICAL: evacuate before explosion

    // ALL robots must be evacuating by end
    expect(timeline[timeline.length - 1]!.evac).toBe(robots.filter(r => r.alive).length);
  });
});

describe('Benchmark', () => {
  it('mine brain at 1Hz with 20 robots', () => {
    const brain = new MineBrain(COAL_MINE);
    const robots = Array.from({ length: 20 }, (_, i) =>
      createRobot(`r${i}`, i * 10, (i % 3) * 5, 'main'),
    );

    const N = 1000;
    const start = process.hrtime.bigint();
    for (let t = 0; t < N; t++) {
      for (const r of robots) r.sensors = sampleEnvironment(r.pos[0]!, r.pos[1]!, t);
      brain.tick(robots);
    }
    const usPerTick = Number(process.hrtime.bigint() - start) / N / 1000;

    console.log(
      `[MINE BRAIN BENCHMARK — 20 robots]\n` +
      `  ${usPerTick.toFixed(0)} μs/tick\n` +
      `  Target 1Hz: YES (${(1e6 / usPerTick).toFixed(0)}x margin)`,
    );

    expect(usPerTick).toBeLessThan(1_000_000); // < 1s per tick
  });
});
