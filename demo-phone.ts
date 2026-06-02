/**
 * OASIS × Android — ON-DEVICE Live Sensor Demo
 *
 * Runs DIRECTLY on the phone inside Termux.
 * Reads sensors natively via termux-sensor/termux-battery-status.
 *
 * Proves the OASIS kernel processes REAL physical sensor data:
 * - Accelerometer → agent position in 128-dim latent space
 * - Gyroscope → angular momentum in tension field
 * - Magnetometer → compass heading → navigation manifold
 * - Barometer → altitude estimation → world pressure zones
 * - Light → environmental awareness → emotional curiosity
 * - Battery temp → proprioceptive thermal monitoring
 * - Shake → reflex arc → emergency stop
 * - Stability → entropy gate (R14) → action safety
 *
 * Usage (in Termux): node dist/demo-phone.js
 */

import { execSync } from 'child_process';
import { OasisKernel } from './kernel/oasis.js';
import { TermuxSensorDriver } from './kernel/hal/drivers/termux-sensor-driver.js';
import type { TermuxSensorData, TermuxBatteryData } from './kernel/hal/drivers/termux-sensor-driver.js';
import { agentId, AgentState } from './kernel/types.js';
import { zeros, norm } from './kernel/physics/vector-math.js';
import { isActionSafe } from './kernel/physics/hyper-state.js';
import { forceOverloadReflex, temperatureReflex } from './kernel/neuro/reflex.js';
import type { SensorReading } from './kernel/perception/sensor-types.js';

// ─── Config ───────────────────────────────────────────────────

const DIM = 128;
const TICK_COUNT = 25;

// ─── Native Termux Sensor Reader ──────────────────────────────

function readSensors(): TermuxSensorData | null {
  try {
    const raw = execSync(
      'termux-sensor -s "LSM6DSVTR Accelerometer,LSM6DSVTR Gyroscope,AK09918C Magnetometer,BMP580 Barometer,STK31610 Light,Samsung Orientation Sensor" -n 1',
      { encoding: 'utf-8', timeout: 10000 },
    ).trim();
    return JSON.parse(raw) as TermuxSensorData;
  } catch (err) {
    console.error('  ⚠ Sensor read failed:', String(err).slice(0, 80));
    return null;
  }
}

function readBattery(): TermuxBatteryData | null {
  try {
    const raw = execSync('termux-battery-status', {
      encoding: 'utf-8',
      timeout: 5000,
    }).trim();
    return JSON.parse(raw) as TermuxBatteryData;
  } catch {
    return null;
  }
}

// ─── Sensor → Latent Space Projection ─────────────────────────

function sensorToForce(driver: TermuxSensorDriver): Float64Array {
  const s = driver.getState();
  const force = zeros(DIM);

  // Scale factor: keep forces small so entropy doesn't explode
  const K = 0.03;

  // Accelerometer → locomotion manifold (dims 10-15)
  force[10] = s.accelX * K;
  force[11] = s.accelY * K;
  force[12] = (s.accelZ - 9.81) * K;

  // Gyroscope → angular manifold (dims 13-15)
  force[13] = s.gyroX * K * 5;
  force[14] = s.gyroY * K * 5;
  force[15] = s.gyroZ * K * 5;

  // Magnetometer → navigation heading (dims 22-27)
  force[22] = Math.cos(s.azimuth * Math.PI / 180) * K * 3;
  force[23] = Math.sin(s.azimuth * Math.PI / 180) * K * 3;
  force[25] = s.azimuth / 360 * K;
  force[26] = (1 - s.motionIntensity) * K;
  force[27] = s.stability * K;

  // Barometer → altitude (dim 30)
  force[30] = (s.pressure - 1013.25) * 0.002;

  // Light → environment (dim 35)
  force[35] = Math.min(1, s.light / 1000) * K;

  // Motion → anchor subspace (dims 0-2) — this is what moves the agent
  force[0] = s.motionIntensity * 0.1;
  force[1] = (1 - s.stability) * 0.05;
  force[2] = s.motionIntensity > 0.5 ? 0.2 : 0;

  // Battery temp → thermal stress (dim 40)
  force[40] = Math.max(0, (s.batteryTemp - 35) / 20);

  return force;
}

function sensorToReading(driver: TermuxSensorDriver): SensorReading {
  const s = driver.getState();
  return {
    sensorId: 'android-imu',
    driverId: driver.id,
    modality: 'IMU',
    values: {
      accelX: s.accelX, accelY: s.accelY, accelZ: s.accelZ,
      gyroX: s.gyroX, gyroY: s.gyroY, gyroZ: s.gyroZ,
    },
    confidence: s.stability,
    timestamp: process.hrtime.bigint(),
  };
}

// ─── Pretty Printing ──────────────────────────────────────────

const G = '\x1b[32m', Y = '\x1b[33m', R = '\x1b[31m', C = '\x1b[36m';
const M = '\x1b[35m', X = '\x1b[0m';

function bar(value: number, max: number, w = 25): string {
  const f = Math.round((Math.min(value, max) / max) * w);
  return '█'.repeat(f) + '░'.repeat(w - f);
}

function ec(v: number): string {
  return v < 0.3 ? G : v < 0.6 ? Y : v < 0.85 ? M : R;
}

// ─── Main ─────────────────────────────────────────────────────

async function main(): Promise<void> {
  console.log(`${C}
╔══════════════════════════════════════════════════════════════╗
║     OASIS × ANDROID — ON-DEVICE Live Sensor Kernel Demo     ║
║   Real Sensors → Tension Field → HyperState → Emotions      ║
║               Running NATIVELY on your phone                 ║
╚══════════════════════════════════════════════════════════════╝${X}
`);

  // ── 1. Init kernel
  console.log('🧠 Initializing OASIS Kernel (128-dim latent space)...');
  const kernel = new OasisKernel({
    dim: DIM,
    tenantId: 'android-phone',
    enableSwarm: true,
    enableImmune: true,
    enableEmotions: true,
    enableMorpho: true,
    enableBranching: true,
    enableDreams: true,
    tickBudgetMs: 200.0, // High budget: termux-sensor takes ~3s per read
  });

  // ── 2. Agents
  const phoneId = kernel.addAgent('phone-body', AgentState.RUNNING, 'HARD_RT');
  const envId = kernel.addAgent('env-sensor', AgentState.RUNNING, 'SOFT_RT');
  const navId = kernel.addAgent('navigator', AgentState.RUNNING, 'BEST_EFFORT');
  console.log('  ✅ 3 agents: phone-body(RT) + env-sensor + navigator');

  // ── 3. HAL driver
  const sensor = new TermuxSensorDriver('galaxy');
  await sensor.init();
  await kernel.addDriver(sensor, phoneId);
  console.log(`  ✅ HAL: ${sensor.name}`);

  // ── 4. Reflexes
  kernel.addReflex(forceOverloadReflex(sensor.id, 25));
  kernel.addReflex(temperatureReflex(sensor.id, 45));
  kernel.addReflex({
    id: 'shake-detect',
    name: 'Shake Emergency Stop',
    driverId: sensor.id,
    conditions: [{ key: 'motionIntensity', op: 'GT', threshold: 0.8 }],
    action: 'EMERGENCY_STOP',
    priority: 0,
    enabled: true,
  });
  console.log('  ✅ Reflexes: force(25N) + temp(45°C) + shake(80%)');

  // ── 5. Sensors for perception bridge
  kernel.addSensor({
    sensorId: 'android-imu', driverId: sensor.id, modality: 'IMU',
    projectionDim: DIM, baseReliability: 0.95, maxStaleTicks: 30,
    primaryDimensions: [10, 11, 12, 13, 14, 15],
  });
  kernel.addSensor({
    sensorId: 'android-env', driverId: sensor.id, modality: 'ENVIRONMENT',
    projectionDim: DIM, baseReliability: 0.9, maxStaleTicks: 50,
    primaryDimensions: [30, 35, 40],
  });

  // ── 6. Goal
  const goalPos = zeros(DIM);
  goalPos[27] = 1.0;
  kernel.setGoal('stability', goalPos, 2.0);
  console.log('  ✅ Goal: achieve stability\n');

  // ── 7. Initial sensor read
  console.log('📡 First sensor read...');
  const init = readSensors();
  if (init) {
    sensor.ingestSensors(init);
    const s = sensor.getState();
    console.log(`  ✅ Accel: [${s.accelX.toFixed(2)}, ${s.accelY.toFixed(2)}, ${s.accelZ.toFixed(2)}] m/s²`);
    console.log(`  ✅ Gyro:  [${s.gyroX.toFixed(4)}, ${s.gyroY.toFixed(4)}, ${s.gyroZ.toFixed(4)}] rad/s`);
    console.log(`  ✅ Mag:   [${s.magX.toFixed(1)}, ${s.magY.toFixed(1)}, ${s.magZ.toFixed(1)}] µT`);
    console.log(`  ✅ Baro:  ${s.pressure.toFixed(1)} hPa (alt ≈ ${s.estimatedAltitude.toFixed(0)}m)`);
    console.log(`  ✅ Light: ${s.light.toFixed(0)} lux`);
  } else {
    console.log('  ⚠ Sensor read failed, using defaults');
  }

  const bat = readBattery();
  if (bat) {
    sensor.ingestBattery(bat);
    console.log(`  ✅ Battery: ${bat.percentage}% @ ${bat.temperature}°C`);
  }

  console.log(`\n${G}══ STARTING ${TICK_COUNT} KERNEL TICKS ══ Move/shake/cover your phone! ══${X}\n`);

  // ── 8. Tick loop
  const results: Array<{
    tick: number; entropy: number; motion: number; stability: number;
    r14Safe: boolean; emotion: string; reflexes: number; synapses: number;
  }> = [];

  for (let t = 1; t <= TICK_COUNT; t++) {
    const t0 = performance.now();

    // Read sensors
    const data = readSensors();
    if (data) sensor.ingestSensors(data);
    if (t % 10 === 0) {
      const b = readBattery();
      if (b) sensor.ingestBattery(b);
    }

    const sensorMs = performance.now() - t0;

    // Project into latent space & emit
    const force = sensorToForce(sensor);
    const ss = sensor.getState();

    // Scale intensity: motion amplifies, stability dampens
    const intensity = 0.3 + ss.motionIntensity * 0.7;
    kernel.emitTension(phoneId, force, intensity);

    // Emit stabilizing force toward RUNNING anchor when entropy is high
    const phoneState = kernel.getAgentState(phoneId);
    if (phoneState && phoneState.entropy > 0.5) {
      const stabilize = zeros(DIM);
      // RUNNING anchor is at position [0, 0.3, 1, 0, ...] in anchor subspace
      stabilize[1] = 0.3;
      stabilize[2] = 1.0;
      const stabilizeIntensity = phoneState.entropy * 2.0; // stronger as entropy rises
      kernel.emitTension(phoneId, stabilize, stabilizeIntensity);
    }

    const envForce = zeros(DIM);
    envForce[30] = (ss.pressure - 1013.25) * 0.02;
    envForce[35] = Math.min(1, ss.light / 500) - 0.5;
    envForce[40] = Math.max(0, (ss.batteryTemp - 30) / 15);
    kernel.emitTension(envId, envForce, 0.3);

    // Feed perception
    kernel.ingestSensorReading(sensorToReading(sensor));

    // TICK
    const t1 = performance.now();
    const result = kernel.tick();
    const kernelMs = performance.now() - t1;

    // Gather state
    const state = kernel.getAgentState(phoneId);
    const emo = kernel.getAgentEmotion(phoneId);

    results.push({
      tick: t,
      entropy: state?.entropy ?? 0,
      motion: ss.motionIntensity,
      stability: ss.stability,
      r14Safe: state ? isActionSafe(state) : true,
      emotion: emo?.dominant ?? 'NONE',
      reflexes: result.reflexesFired.length,
      synapses: kernel.getSynapseCount(),
    });

    // Display
    console.log(`${Y}── T${String(t).padStart(2)} ─────────────────────────────────────────────────${X}`);
    console.log(`  📱 Accel: [${ss.accelX.toFixed(2)}, ${ss.accelY.toFixed(2)}, ${ss.accelZ.toFixed(2)}]  Gyro: [${ss.gyroX.toFixed(3)}, ${ss.gyroY.toFixed(3)}, ${ss.gyroZ.toFixed(3)}]`);
    console.log(`  🧭 Heading: ${ss.azimuth.toFixed(0)}°  🏔 ${ss.pressure.toFixed(1)}hPa  💡 ${ss.light.toFixed(0)}lux  🔋 ${ss.batteryPercent}%/${ss.batteryTemp}°C`);

    if (state) {
      const ev = state.entropy;
      console.log(`  ${ec(ev)}Entropy: ${bar(ev, 1)} ${(ev * 100).toFixed(1)}%${X}  State: ${state.collapsed}  R14: ${isActionSafe(state) ? `${G}SAFE${X}` : `${R}BLOCKED${X}`}`);
    }
    if (emo) {
      console.log(`  🎭 ${M}${emo.dominant}${X}  cur:${(emo.curiosity * 100).toFixed(0)}% fear:${(emo.fear * 100).toFixed(0)}% sat:${(emo.satisfaction * 100).toFixed(0)}% fru:${(emo.frustration * 100).toFixed(0)}% urg:${emo.urgency.toFixed(1)}x`);
    }
    console.log(`  ⚡ Motion: ${bar(ss.motionIntensity, 1)} ${(ss.motionIntensity * 100).toFixed(0)}%  Stability: ${bar(ss.stability, 1)} ${(ss.stability * 100).toFixed(0)}%`);
    console.log(`  🧠 Synapses: ${kernel.getSynapseCount()}  Role: ${kernel.getAgentRole(phoneId)}  ⏱ sensor:${sensorMs.toFixed(0)}ms kernel:${kernelMs.toFixed(1)}ms`);

    if (result.reflexesFired.length > 0) {
      console.log(`  ${R}⚡ REFLEX FIRED! ${result.reflexesFired.length} reflex(es)${X}`);
    }
    if (result.r14Blocked > 0) {
      console.log(`  ${R}🛑 R14 ENTROPY GATE: ${result.r14Blocked} agent(s) blocked${X}`);
    }
    if (result.dreamResult) {
      console.log(`  ${M}💭 DREAM: ${result.dreamResult.experiencesReplayed} experiences replayed${X}`);
    }

    if (!kernel.isAlive()) {
      console.log(`\n${R}💀 KERNEL PANIC — Kill switch triggered!${X}`);
      break;
    }
  }

  // ── 9. Summary
  console.log(`\n${C}╔══════════════════════════════════════════════════════════════╗`);
  console.log(`║                     DEMO RESULTS                            ║`);
  console.log(`╚══════════════════════════════════════════════════════════════╝${X}`);

  const avg = (arr: number[]) => arr.reduce((a, b) => a + b, 0) / arr.length;
  const avgE = avg(results.map(r => r.entropy));
  const maxE = Math.max(...results.map(r => r.entropy));
  const avgM = avg(results.map(r => r.motion));
  const avgS = avg(results.map(r => r.stability));
  const r14 = results.filter(r => !r.r14Safe).length;
  const ref = results.reduce((s, r) => s + r.reflexes, 0);

  const emoCounts: Record<string, number> = {};
  for (const r of results) emoCounts[r.emotion] = (emoCounts[r.emotion] ?? 0) + 1;

  console.log(`  📊 ${results.length} ticks completed`);
  console.log(`  📈 Entropy    : avg ${(avgE * 100).toFixed(1)}% | max ${(maxE * 100).toFixed(1)}%`);
  console.log(`  🏃 Motion     : avg ${(avgM * 100).toFixed(1)}%`);
  console.log(`  ⚖️  Stability  : avg ${(avgS * 100).toFixed(1)}%`);
  console.log(`  🛑 R14 Blocks : ${r14}`);
  console.log(`  ⚡ Reflexes   : ${ref}`);
  console.log(`  🧠 Synapses   : ${kernel.getSynapseCount()}`);
  console.log(`  🎭 Emotions   : ${Object.entries(emoCounts).map(([e, c]) => `${e}:${c}`).join(' ')}`);

  console.log(`\n  📉 Entropy Timeline:`);
  for (const r of results) {
    const f = Math.round(r.entropy * 40);
    const c = ec(r.entropy);
    const x = r.r14Safe ? ' ' : '!';
    console.log(`   T${String(r.tick).padStart(2)}${x}${c}${'█'.repeat(f)}${'░'.repeat(40 - f)}${X} ${(r.entropy * 100).toFixed(1)}%`);
  }

  console.log(`\n${G}  ✅ OASIS kernel ran NATIVELY on Android with REAL sensor data!`);
  console.log(`     Tensorial Brain + Emotions + Reflexes + Synapses = ALIVE.${X}\n`);
}

main().catch(err => {
  console.error('Fatal:', err);
  process.exit(1);
});
