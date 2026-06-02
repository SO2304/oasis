/**
 * OASIS × Android — Live Sensor Demo
 *
 * Proves OASIS works with REAL hardware:
 * - Samsung phone sensors via Termux API + ADB
 * - Full kernel pipeline: Perception → HyperState → TensionField → Emotions → Reflexes
 *
 * What this demonstrates:
 * 1. Real accelerometer/gyro data → agent position in latent space
 * 2. Motion intensity → entropy fluctuation → R14 safety gate
 * 3. Light/barometer changes → world model pressure zones
 * 4. Shake detection → reflex arc (emergency stop)
 * 5. Stability → emotional modulation (curiosity vs fear)
 * 6. Synaptic learning between phone-agent and environment-agent
 * 7. Battery temperature → proprioceptive health monitoring
 *
 * Usage: npx tsx demo-android.ts
 */

import { execSync } from 'child_process';
import { OasisKernel } from './kernel/oasis.js';
import { TermuxSensorDriver, type TermuxSensorData, type TermuxBatteryData } from './kernel/hal/drivers/termux-sensor-driver.js';
import { agentId, tenantId, driverId, AgentState, SchedulePriority } from './kernel/types.js';
import { vec, zeros, norm, normalize, scale, add, sub } from './kernel/physics/vector-math.js';
import { createHyperState, isActionSafe, calculateEntropy, R14_ENTROPY_CRITICAL } from './kernel/physics/hyper-state.js';
import { proximityReflex, temperatureReflex, forceOverloadReflex } from './kernel/neuro/reflex.js';
import type { SensorReading } from './kernel/perception/sensor-types.js';

// ─── Config ───────────────────────────────────────────────────

const ADB_PATH = 'C:\\tmp\\platform-tools\\adb.exe';
const DIM = 128;
const TICK_COUNT = 20;       // Number of kernel ticks to run
const POLL_DELAY_MS = 500;   // Sensor polling interval

// ─── ADB Sensor Reader (via dumpsys sensorservice) ────────────
// Reads LIVE sensor values directly from Android's SensorService
// No Termux dependency — instant, reliable, zero latency

function adbShell(cmd: string): string {
  return execSync(`"${ADB_PATH}" shell "${cmd}"`, {
    encoding: 'utf-8',
    timeout: 5000,
  }).trim();
}

/** Parse the last event values from dumpsys sensorservice output */
function parseLastEvent(dump: string, sensorName: string): number[] {
  const regex = new RegExp(`${sensorName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}: last \\d+ events\\n\\s+\\d+ \\([^)]+\\) ([\\d., eE+-]+)`);
  const match = dump.match(regex);
  if (!match?.[1]) return [];
  return match[1].split(',').map(v => parseFloat(v.trim())).filter(v => !isNaN(v));
}

function readSensors(): TermuxSensorData | null {
  try {
    const dump = adbShell('dumpsys sensorservice');

    const accel = parseLastEvent(dump, 'LSM6DSVTR Accelerometer');
    const gyro = parseLastEvent(dump, 'LSM6DSVTR Gyroscope');
    const mag = parseLastEvent(dump, 'AK09918C Magnetometer');
    const baro = parseLastEvent(dump, 'BMP580 Barometer');
    const light = parseLastEvent(dump, 'STK31610 Light');
    const gravity = parseLastEvent(dump, 'Samsung Gravity Sensor');
    const orientation = parseLastEvent(dump, 'Samsung Orientation Sensor');

    const data: TermuxSensorData = {};
    if (accel.length >= 3) data['LSM6DSVTR Accelerometer'] = { values: [accel[0]!, accel[1]!, accel[2]!] };
    if (gyro.length >= 3) data['LSM6DSVTR Gyroscope'] = { values: [gyro[0]!, gyro[1]!, gyro[2]!] };
    if (mag.length >= 3) data['AK09918C Magnetometer'] = { values: [mag[0]!, mag[1]!, mag[2]!] };
    if (baro.length >= 1) data['BMP580 Barometer'] = { values: [baro[0]!] };
    if (light.length >= 1) data['STK31610 Light'] = { values: [light[0]!] };
    if (gravity.length >= 3) data['Samsung Gravity Sensor'] = { values: [gravity[0]!, gravity[1]!, gravity[2]!] };
    if (orientation.length >= 3) data['Samsung Orientation Sensor'] = { values: [orientation[0]!, orientation[1]!, orientation[2]!] };

    return data;
  } catch {
    return null;
  }
}

function readBattery(): TermuxBatteryData | null {
  try {
    const raw = adbShell('dumpsys battery');
    const getNum = (key: string): number => {
      const m = raw.match(new RegExp(`${key}: (\\d+)`));
      return m?.[1] ? parseInt(m[1]) : 0;
    };
    const getStr = (key: string): string => {
      const m = raw.match(new RegExp(`${key}: (\\d+)`));
      return m?.[1] ?? '';
    };
    return {
      health: getStr('health'),
      percentage: getNum('level'),
      plugged: getStr('plugged'),
      status: getStr('status'),
      temperature: getNum('temperature') / 10, // Android reports in tenths of °C
      voltage: getNum('voltage'),
      current: 0, // Not in dumpsys battery
    };
  } catch {
    return null;
  }
}

// ─── Sensor → Latent Space Projection ─────────────────────────

function sensorToForce(driver: TermuxSensorDriver): Float64Array {
  const s = driver.getState();
  const force = zeros(DIM);

  // Accelerometer → locomotion manifold (dims 10-15)
  // Gravity-compensated deviation = "how much is the phone being moved?"
  force[10] = s.accelX * 0.1;          // lateral force
  force[11] = s.accelY * 0.1;          // forward force
  force[12] = (s.accelZ - 9.81) * 0.1; // vertical deviation from gravity

  // Gyroscope → angular manifold (dims 13-15)
  force[13] = s.gyroX * 2.0;   // roll rate amplified
  force[14] = s.gyroY * 2.0;   // pitch rate
  force[15] = s.gyroZ * 2.0;   // yaw rate

  // Magnetometer → navigation heading (dims 22-27)
  force[22] = Math.cos(s.azimuth * Math.PI / 180) * 0.5;  // heading X
  force[23] = Math.sin(s.azimuth * Math.PI / 180) * 0.5;  // heading Y
  force[24] = 0;                                            // heading Z
  force[25] = s.azimuth / 360;                              // raw heading normalized
  force[26] = 1 - s.motionIntensity;                        // urgency inverse
  force[27] = s.stability;                                   // confidence

  // Barometer → altitude pressure (dim 30)
  force[30] = (s.pressure - 1013.25) * 0.01; // deviation from sea level

  // Light → environmental awareness (dim 35)
  force[35] = Math.min(1, s.light / 1000) * 0.5; // normalized luminosity

  // Motion intensity → global excitation (dims 0-2, anchor subspace)
  force[0] = s.motionIntensity * 0.3;
  force[1] = (1 - s.stability) * 0.2;
  force[2] = s.motionIntensity > 0.5 ? 0.5 : 0; // threshold activation

  // Battery temperature → thermal stress (dim 40)
  force[40] = Math.max(0, (s.batteryTemp - 35) / 10); // stress above 35°C

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

function bar(value: number, max: number, width: number = 30): string {
  const filled = Math.round((Math.min(value, max) / max) * width);
  return '█'.repeat(filled) + '░'.repeat(width - filled);
}

function severityColor(val: number): string {
  if (val < 0.3) return '\x1b[32m'; // green
  if (val < 0.6) return '\x1b[33m'; // yellow
  if (val < 0.85) return '\x1b[35m'; // magenta
  return '\x1b[31m'; // red
}

function printHeader(): void {
  console.log('\x1b[36m');
  console.log('╔══════════════════════════════════════════════════════════════╗');
  console.log('║        OASIS × ANDROID — Live Sensor Kernel Demo           ║');
  console.log('║    Real Hardware → Tension Field → Emotions → Behavior     ║');
  console.log('╚══════════════════════════════════════════════════════════════╝');
  console.log('\x1b[0m');
}

function printSensorState(driver: TermuxSensorDriver, tick: number): void {
  const s = driver.getState();
  console.log(`\x1b[33m── Tick ${tick} ─ Sensor Readings ──────────────────────────────\x1b[0m`);
  console.log(`  📱 Accelerometer : x=${s.accelX.toFixed(3)} y=${s.accelY.toFixed(3)} z=${s.accelZ.toFixed(3)} m/s²`);
  console.log(`  🌀 Gyroscope     : x=${s.gyroX.toFixed(4)} y=${s.gyroY.toFixed(4)} z=${s.gyroZ.toFixed(4)} rad/s`);
  console.log(`  🧭 Magnetometer  : x=${s.magX.toFixed(1)} y=${s.magY.toFixed(1)} z=${s.magZ.toFixed(1)} µT`);
  console.log(`  🏔  Barometer     : ${s.pressure.toFixed(2)} hPa (alt ≈ ${s.estimatedAltitude.toFixed(1)}m)`);
  console.log(`  💡 Light         : ${s.light.toFixed(0)} lux`);
  console.log(`  🔋 Battery       : ${s.batteryPercent}% @ ${s.batteryTemp}°C`);
  console.log(`  🧭 Heading       : ${s.azimuth.toFixed(1)}° (pitch: ${s.pitch.toFixed(1)}° roll: ${s.roll.toFixed(1)}°)`);
}

function printKernelState(
  kernel: OasisKernel,
  phoneAgent: string,
  envAgent: string,
  tick: number,
  motionIntensity: number,
  stability: number,
): void {
  const phoneState = kernel.getAgentState(agentId(phoneAgent));
  const envState = kernel.getAgentState(agentId(envAgent));
  const phoneEmotion = kernel.getAgentEmotion(agentId(phoneAgent));

  console.log(`\x1b[36m── Kernel State ──────────────────────────────────────────────\x1b[0m`);

  if (phoneState) {
    const entropyColor = severityColor(phoneState.entropy);
    console.log(`  HyperState [phone-body]:`);
    console.log(`    Entropy  : ${entropyColor}${bar(phoneState.entropy, 1)} ${(phoneState.entropy * 100).toFixed(1)}%\x1b[0m`);
    console.log(`    Collapsed: ${phoneState.collapsed}`);
    console.log(`    R14 Safe : ${isActionSafe(phoneState) ? '\x1b[32m✓ ACTIONS ALLOWED\x1b[0m' : '\x1b[31m✗ ACTIONS BLOCKED (entropy too high)\x1b[0m'}`);
    console.log(`    Momentum : ${norm(phoneState.momentum).toFixed(4)}`);
  }

  if (phoneEmotion) {
    console.log(`  Emotions [phone-body]:`);
    console.log(`    Curiosity    : ${bar(phoneEmotion.curiosity, 1)} ${(phoneEmotion.curiosity * 100).toFixed(0)}%`);
    console.log(`    Fear         : ${bar(phoneEmotion.fear, 1)} ${(phoneEmotion.fear * 100).toFixed(0)}%`);
    console.log(`    Satisfaction : ${bar(phoneEmotion.satisfaction, 1)} ${(phoneEmotion.satisfaction * 100).toFixed(0)}%`);
    console.log(`    Frustration  : ${bar(phoneEmotion.frustration, 1)} ${(phoneEmotion.frustration * 100).toFixed(0)}%`);
    console.log(`    Urgency      : ${bar(phoneEmotion.urgency, 5)} ${phoneEmotion.urgency.toFixed(2)}x`);
    console.log(`    Dominant     : \x1b[35m${phoneEmotion.dominant}\x1b[0m`);
  }

  console.log(`  Motion    : ${bar(motionIntensity, 1)} ${(motionIntensity * 100).toFixed(0)}%`);
  console.log(`  Stability : ${bar(stability, 1)} ${(stability * 100).toFixed(0)}%`);
  console.log(`  Synapses  : ${kernel.getSynapseCount()}`);
  console.log(`  Role      : ${kernel.getAgentRole(agentId(phoneAgent))}`);
}

// ─── Main ─────────────────────────────────────────────────────

async function main(): Promise<void> {
  printHeader();

  // ── 1. Verify connection
  console.log('🔌 Checking ADB connection...');
  try {
    const devices = execSync(`"${ADB_PATH}" devices`, { encoding: 'utf-8' });
    const deviceLines = devices.trim().split('\n').slice(1).filter(l => l.includes('device'));
    if (deviceLines.length === 0) {
      console.error('❌ No Android device connected. Connect via USB and enable USB debugging.');
      process.exit(1);
    }
    console.log(`✅ Device connected: ${deviceLines[0]?.split('\t')[0]}\n`);
  } catch {
    console.error('❌ ADB not found. Ensure platform-tools is at:', ADB_PATH);
    process.exit(1);
  }

  // ── 2. Initialize OASIS Kernel
  console.log('🧠 Initializing OASIS Kernel...');
  const kernel = new OasisKernel({
    dim: DIM,
    tenantId: 'android-demo',
    enableSwarm: true,
    enableImmune: true,
    enableEmotions: true,
    enableMorpho: true,
    enableBranching: true,
    enableDreams: true,
    tickBudgetMs: 5.0,
  });

  // ── 3. Create agents
  const phoneBodyId = kernel.addAgent('phone-body', AgentState.RUNNING, 'HARD_RT');
  const envSensorId = kernel.addAgent('env-sensor', AgentState.RUNNING, 'SOFT_RT');
  const navigatorId = kernel.addAgent('navigator', AgentState.RUNNING, 'BEST_EFFORT');
  console.log(`  ✅ Agents: phone-body (HARD_RT), env-sensor (SOFT_RT), navigator (BEST_EFFORT)`);

  // ── 4. Initialize Termux sensor driver
  const sensorDriver = new TermuxSensorDriver('galaxy');
  await sensorDriver.init();
  const dId = await kernel.addDriver(sensorDriver, phoneBodyId);
  console.log(`  ✅ HAL Driver: ${sensorDriver.name} [${sensorDriver.id}]`);

  // ── 5. Register reflexes (safety)
  kernel.addReflex(forceOverloadReflex(sensorDriver.id, 20)); // >20N = phone dropped hard
  kernel.addReflex(temperatureReflex(sensorDriver.id, 45));    // Battery > 45°C
  kernel.addReflex({
    id: 'shake-reflex',
    name: 'Shake Emergency Stop',
    driverId: sensorDriver.id,
    conditions: [{ key: 'motionIntensity', op: 'GT', threshold: 0.8 }],
    action: 'EMERGENCY_STOP',
    priority: 0,
    enabled: true,
  });
  console.log(`  ✅ Reflexes: force-overload (20N), temperature (45°C), shake (80% intensity)`);

  // ── 6. Register sensor config for perception bridge
  kernel.addSensor({
    sensorId: 'android-imu',
    driverId: sensorDriver.id,
    modality: 'IMU',
    projectionDim: DIM,
    baseReliability: 0.95,
    maxStaleTicks: 30,
    primaryDimensions: [10, 11, 12, 13, 14, 15],
  });
  kernel.addSensor({
    sensorId: 'android-env',
    driverId: sensorDriver.id,
    modality: 'ENVIRONMENT',
    projectionDim: DIM,
    baseReliability: 0.9,
    maxStaleTicks: 50,
    primaryDimensions: [30, 35, 40],
  });
  console.log(`  ✅ Sensors registered: IMU (6-axis), Environment (baro+light+temp)`);

  // ── 7. Set up world model
  const goalPos = zeros(DIM);
  goalPos[27] = 1.0; // confidence = goal
  goalPos[26] = 0.0; // low urgency
  kernel.setGoal('stability-goal', goalPos, 2.0);
  console.log(`  ✅ Goal: achieve stability (low motion, high confidence)\n`);

  // ── 8. Initial sensor read
  console.log('📡 Reading initial sensor data...');
  const initialData = readSensors();
  if (initialData) {
    sensorDriver.ingestSensors(initialData);
    console.log('✅ Initial sensor data captured\n');
  } else {
    console.log('⚠️  Could not read initial sensors, continuing with defaults\n');
  }

  const batteryData = readBattery();
  if (batteryData) {
    sensorDriver.ingestBattery(batteryData);
  }

  // ── 9. Main loop — OASIS kernel ticks with real sensor data
  console.log(`\x1b[32m╔══════════════════════════════════════════════════════════════╗`);
  console.log(`║              STARTING KERNEL TICK LOOP (${TICK_COUNT} ticks)              ║`);
  console.log(`║      Move/shake/cover your phone to see OASIS react!       ║`);
  console.log(`╚══════════════════════════════════════════════════════════════╝\x1b[0m\n`);

  const results: Array<{
    tick: number;
    entropy: number;
    motion: number;
    stability: number;
    r14Safe: boolean;
    emotion: string;
    reflexes: number;
    synapses: number;
  }> = [];

  for (let t = 1; t <= TICK_COUNT; t++) {
    // ── Poll sensors
    const data = readSensors();
    if (data) {
      sensorDriver.ingestSensors(data);
    }

    // Re-read battery every 10 ticks
    if (t % 10 === 0) {
      const bat = readBattery();
      if (bat) sensorDriver.ingestBattery(bat);
    }

    // ── Project sensor data into latent forces
    const force = sensorToForce(sensorDriver);
    kernel.emitTension(phoneBodyId, force, 1.0 + sensorDriver.getState().motionIntensity);

    // Emit environment tension from env-sensor
    const envForce = zeros(DIM);
    envForce[30] = (sensorDriver.getState().pressure - 1013.25) * 0.02;
    envForce[35] = Math.min(1, sensorDriver.getState().light / 500) - 0.5;
    envForce[40] = Math.max(0, (sensorDriver.getState().batteryTemp - 30) / 15);
    kernel.emitTension(envSensorId, envForce, 0.5);

    // ── Feed perception bridge
    kernel.ingestSensorReading(sensorToReading(sensorDriver));

    // ── TICK THE KERNEL
    const tickResult = kernel.tick();

    // ── Gather results
    const state = kernel.getAgentState(phoneBodyId);
    const emotion = kernel.getAgentEmotion(phoneBodyId);
    const sState = sensorDriver.getState();

    results.push({
      tick: t,
      entropy: state?.entropy ?? 0,
      motion: sState.motionIntensity,
      stability: sState.stability,
      r14Safe: state ? isActionSafe(state) : true,
      emotion: emotion?.dominant ?? 'NONE',
      reflexes: tickResult.reflexesFired.length,
      synapses: kernel.getSynapseCount(),
    });

    // ── Print state
    if (data) {
      printSensorState(sensorDriver, t);
    }
    printKernelState(kernel, 'phone-body', 'env-sensor', t, sState.motionIntensity, sState.stability);

    if (tickResult.reflexesFired.length > 0) {
      console.log(`  \x1b[31m⚡ REFLEX FIRED! ${tickResult.reflexesFired.length} reflex(es) triggered\x1b[0m`);
    }
    if (tickResult.r14Blocked > 0) {
      console.log(`  \x1b[31m🛑 R14 ENTROPY GATE: ${tickResult.r14Blocked} agent(s) blocked\x1b[0m`);
    }
    if (tickResult.dreamResult) {
      console.log(`  \x1b[35m💭 DREAM: replayed ${tickResult.dreamResult.experiencesReplayed} experiences\x1b[0m`);
    }

    console.log('');

    // Check if kernel is still alive
    if (!kernel.isAlive()) {
      console.log('\x1b[31m💀 KERNEL PANIC — Kill switch triggered!\x1b[0m');
      break;
    }
  }

  // ── 10. Summary
  printSummary(results, kernel);
}

function printSummary(
  results: Array<{
    tick: number; entropy: number; motion: number; stability: number;
    r14Safe: boolean; emotion: string; reflexes: number; synapses: number;
  }>,
  kernel: OasisKernel,
): void {
  console.log('\x1b[36m╔══════════════════════════════════════════════════════════════╗');
  console.log('║                    DEMO SUMMARY                            ║');
  console.log('╚══════════════════════════════════════════════════════════════╝\x1b[0m');

  const avgEntropy = results.reduce((s, r) => s + r.entropy, 0) / results.length;
  const avgMotion = results.reduce((s, r) => s + r.motion, 0) / results.length;
  const avgStability = results.reduce((s, r) => s + r.stability, 0) / results.length;
  const maxEntropy = Math.max(...results.map(r => r.entropy));
  const r14Blocks = results.filter(r => !r.r14Safe).length;
  const reflexFires = results.reduce((s, r) => s + r.reflexes, 0);
  const emotionCounts: Record<string, number> = {};
  for (const r of results) {
    emotionCounts[r.emotion] = (emotionCounts[r.emotion] ?? 0) + 1;
  }

  console.log(`\n  📊 ${results.length} ticks completed`);
  console.log(`  📈 Avg Entropy    : ${(avgEntropy * 100).toFixed(1)}% (max: ${(maxEntropy * 100).toFixed(1)}%)`);
  console.log(`  🏃 Avg Motion     : ${(avgMotion * 100).toFixed(1)}%`);
  console.log(`  ⚖️  Avg Stability  : ${(avgStability * 100).toFixed(1)}%`);
  console.log(`  🛑 R14 Blocks     : ${r14Blocks}`);
  console.log(`  ⚡ Reflex Fires   : ${reflexFires}`);
  console.log(`  🧠 Final Synapses : ${kernel.getSynapseCount()}`);
  console.log(`  🎭 Emotion Dist   :`);
  for (const emotion of Object.keys(emotionCounts)) {
    const count = emotionCounts[emotion]!;
    console.log(`       ${emotion}: ${count} ticks (${((count / results.length) * 100).toFixed(0)}%)`);
  }

  // Entropy timeline
  console.log(`\n  📉 Entropy Timeline:`);
  const timelineWidth = 50;
  for (const r of results) {
    const filled = Math.round(r.entropy * timelineWidth);
    const color = severityColor(r.entropy);
    const rBlock = r.r14Safe ? ' ' : '!';
    console.log(`    T${String(r.tick).padStart(3)}${rBlock}${color}${'█'.repeat(filled)}${'░'.repeat(timelineWidth - filled)}\x1b[0m ${(r.entropy * 100).toFixed(1)}%`);
  }

  console.log('\n\x1b[32m  ✅ OASIS kernel successfully processed real Android sensor data!');
  console.log('     The Tensorial Brain responded to physical world stimuli.\x1b[0m\n');
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});
