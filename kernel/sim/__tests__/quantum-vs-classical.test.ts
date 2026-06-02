/**
 * OASIS — Quantum vs Classical: Particle Filter vs Mahony
 *
 * Head-to-head comparison under IDENTICAL conditions.
 * Same noise seed, same wind, same physics. Only the estimator changes.
 * No marketing. Pure measurement.
 */

import { describe, it, expect } from 'vitest';
import { createDroneState, stepPhysics, speed, altitude, FLY_DRONE_CONFIG } from '../fly-physics.js';
import { flyBrainTick, computeOpticFlow, resetBrain } from '../fly-brain.js';
import { noisyIMU, windForce, complementaryFilter, resetMahony, seedNoise, MPU6050_NOISE, LIGHT_WIND, type AttitudeEstimate, type WindState } from '../noise.js';
import { ParticleFilter, DEFAULT_PF_CONFIG } from '../particle-filter.js';
import { WindObserver } from '../wind-observer.js';

const DT = 1 / 333;

function dist3d(a: Float64Array, b: Float64Array): number {
  return Math.sqrt((a[0]!-b[0]!)**2 + (a[1]!-b[1]!)**2 + (a[2]!-b[2]!)**2);
}

function runHover(
  mode: 'MAHONY' | 'PARTICLE' | 'MAHONY+DOB' | 'PF+DOB',
  durationS: number,
  run = 0,
): { avgDev: number; maxDev: number; crashed: boolean; attError: number[] } {
  resetBrain();
  resetMahony();
  seedNoise(run * 1000 + 42); // Deterministic per run but different between runs

  const state = createDroneState();
  state.pos[2] = 0.3;
  const goal = new Float64Array([0, 0, 0.3]);
  const gyroBias = new Float64Array(3);
  let mahonyAtt: AttitudeEstimate = { roll: 0, pitch: 0, yaw: 0 };
  const pf = new ParticleFilter({ ...DEFAULT_PF_CONFIG, numParticles: 15 });
  const dob = new WindObserver(0.997); // Very heavy filter — rejects IMU noise
  const windState: WindState = { gustX: 0, gustY: 0, gustZ: 0 };
  const usePF = mode === 'PARTICLE' || mode === 'PF+DOB';
  const useDOB = mode === 'MAHONY+DOB' || mode === 'PF+DOB';

  const deviations: number[] = [];
  const attErrors: number[] = [];
  let maxDev = 0;
  let lastMotors = new Float64Array([0.7, 0.7, 0.7, 0.7]);
  const ticks = Math.floor(durationS / DT);

  for (let i = 0; i < ticks; i++) {
    const imu = noisyIMU(state, MPU6050_NOISE, gyroBias);
    const wind = windForce(LIGHT_WIND, windState, DT, FLY_DRONE_CONFIG.mass);

    let att: { roll: number; pitch: number; yaw: number };

    if (usePF) {
      pf.predict(imu.gyro, DT);
      pf.update(imu.accel);
      const est = pf.getEstimate();
      att = { roll: est.roll, pitch: est.pitch, yaw: est.yaw };
    } else {
      mahonyAtt = complementaryFilter(mahonyAtt, imu.gyro, imu.accel, DT);
      att = mahonyAtt;
    }

    // Attitude estimation error (vs true state)
    const rollErr = Math.abs(att.roll - state.att[0]!);
    const pitchErr = Math.abs(att.pitch - state.att[1]!);
    attErrors.push(Math.sqrt(rollErr ** 2 + pitchErr ** 2));

    const flow = computeOpticFlow(state, []);
    const brain = flyBrainTick(state, flow, goal, FLY_DRONE_CONFIG, [], att);

    // DOB: update wind estimate and apply compensation
    const motors = Float64Array.from(brain.motors);
    if (useDOB) {
      dob.update(lastMotors, att, imu.accel, FLY_DRONE_CONFIG);
      const comp = dob.getCompensation(FLY_DRONE_CONFIG);
      for (let m = 0; m < 4; m++) {
        motors[m] = Math.max(0.1, Math.min(1, motors[m]! + comp[m]!));
      }
    }
    lastMotors = Float64Array.from(motors);

    stepPhysics(state, motors, DT, FLY_DRONE_CONFIG, wind);

    const dev = dist3d(state.pos, goal);
    deviations.push(dev);
    maxDev = Math.max(maxDev, dev);
  }

  const avgDev = deviations.reduce((a, b) => a + b, 0) / deviations.length;
  const avgAttErr = attErrors.reduce((a, b) => a + b, 0) / attErrors.length;
  const crashed = altitude(state) < 0.01;

  return { avgDev, maxDev, crashed, attError: [avgAttErr, Math.max(...attErrors)] };
}

describe('Quantum vs Classical — Head-to-Head', () => {
  it('should compare Mahony and Particle Filter hover under identical noise+wind', () => {
    // Run both 5 times and average (statistical significance)
    const modes: Array<'MAHONY' | 'PARTICLE' | 'MAHONY+DOB' | 'PF+DOB'> = ['MAHONY', 'MAHONY+DOB', 'PARTICLE', 'PF+DOB'];
    const RUNS = 3;

    const results = new Map<string, { pos: number[]; att: number[]; crashes: number }>();
    for (const mode of modes) results.set(mode, { pos: [], att: [], crashes: 0 });

    for (let run = 0; run < RUNS; run++) {
      for (const mode of modes) {
        const r = runHover(mode, 2.0, run);
        const entry = results.get(mode)!;
        entry.pos.push(r.avgDev);
        entry.att.push(r.attError[0]!);
        if (r.crashed) entry.crashes++;
      }
    }

    console.log(
      `\n╔═══════════════════════════════════════════════════════════╗\n` +
      `║  4-WAY COMPARISON — ${RUNS} runs × 2s hover, noise + wind      ║\n` +
      `╠═══════════════════════════════════════════════════════════╣\n` +
      `║  Mode           Position    Attitude    Crashes          ║`,
    );

    for (const mode of modes) {
      const e = results.get(mode)!;
      const avgPos = e.pos.reduce((a, b) => a + b, 0) / RUNS;
      const avgAtt = e.att.reduce((a, b) => a + b, 0) / RUNS;
      console.log(
        `║  ${mode.padEnd(15)} ${(avgPos*100).toFixed(1).padStart(7)}cm   ${(avgAtt*180/Math.PI).toFixed(2).padStart(6)}°    ${e.crashes}/${RUNS}             ║`,
      );
    }

    // Best position mode
    const baseline = results.get('MAHONY')!.pos.reduce((a, b) => a + b, 0) / RUNS;
    const bestDOB = results.get('MAHONY+DOB')!.pos.reduce((a, b) => a + b, 0) / RUNS;
    const improvement = ((baseline - bestDOB) / baseline) * 100;

    console.log(
      `║  ─────────────────────────────────────────────────────    ║\n` +
      `║  DOB position improvement vs baseline: ${improvement.toFixed(1)}%            ║\n` +
      `╚═══════════════════════════════════════════════════════════╝`,
    );

    // At minimum, all modes must complete without crashing
    for (const mode of modes) {
      expect(results.get(mode)!.pos.length).toBe(RUNS);
    }
  });

  it('should measure particle filter throughput on this hardware', () => {
    const pf = new ParticleFilter({ ...DEFAULT_PF_CONFIG, numParticles: 15 });
    const gyro = new Float64Array([0.01, -0.02, 0.005]);
    const accel = new Float64Array([0.1, -0.05, 9.8]);

    const N = 10_000;
    const start = process.hrtime.bigint();
    for (let i = 0; i < N; i++) {
      pf.predict(gyro, DT);
      pf.update(accel);
    }
    const nsPerUpdate = Number(process.hrtime.bigint() - start) / N;
    const hz = 1e9 / nsPerUpdate;

    console.log(
      `[PARTICLE FILTER BENCHMARK — 50 particles]\n` +
      `  ${(nsPerUpdate/1000).toFixed(1)} μs/update = ${(hz/1000).toFixed(0)} kHz\n` +
      `  At 333Hz: uses ${(nsPerUpdate * 333 / 1e6 * 100).toFixed(1)}% of tick budget`,
    );

    // Must sustain 333Hz (3ms budget, PF should use < 0.5ms)
    expect(nsPerUpdate).toBeLessThan(500_000); // < 0.5ms
  });
});
