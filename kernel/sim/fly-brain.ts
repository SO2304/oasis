/**
 * OASIS Kernel — Fly Brain Controller
 *
 * HOW A FLY'S BRAIN WORKS (simplified):
 *
 * 1. OPTIC FLOW: The fly doesn't see "objects". It sees FLOW —
 *    patterns of motion across its retina. Expansion = approaching.
 *    Lateral flow = drifting. The entire visual system is a
 *    motion detector, not an image recognizer.
 *
 * 2. LOOMING DETECTOR: If the optic flow expands rapidly in one
 *    region, something is approaching FAST. The fly initiates
 *    an escape within 30ms — BEFORE it identifies what it is.
 *    This is the biological reflex arc.
 *
 * 3. HALTERES (gyroscopes): The fly has biological gyroscopes
 *    (modified wings) that detect rotation with extreme precision.
 *    Feedback loop: haltere → wing muscle correction at 200Hz.
 *
 * This module implements these three systems as an OASIS controller
 * that reads DroneState (simulated IMU + optic flow) and produces
 * motor commands. No PID tuning. No model predictive control.
 * Pure biomimetic control via the OASIS nervous system.
 */

import type { DroneState, FlyDroneConfig } from './fly-physics.js';
import { speed } from './fly-physics.js';
import { OasisKernel } from '../oasis.js';
import { agentId, AgentState } from '../types.js';
import { zeros, norm } from '../physics/vector-math.js';
import { proximityReflex } from '../neuro/reflex.js';

// ─── Optic Flow ─────────────────────────────────────────────────

export interface OpticFlow {
  /** Expansion rate (negative = approaching, positive = receding) */
  expansion: number;
  /** Lateral flow [left-right, up-down] */
  lateral: Float64Array;
  /** Rotation rate detected by "halteres" */
  rotation: Float64Array;
}

/**
 * Compute optic flow from drone state and nearby obstacles.
 * This is what the fly's compound eyes "see".
 */
/** Previous obstacle positions for velocity estimation */
const prevObsPositions = new Map<number, Float64Array>();

export function computeOpticFlow(
  state: DroneState,
  obstacles: Array<{ pos: Float64Array; radius: number }>,
): OpticFlow {
  let expansion = 0;
  const lateral = new Float64Array(2);

  for (let oi = 0; oi < obstacles.length; oi++) {
    const obs = obstacles[oi]!;
    const dx = obs.pos[0]! - state.pos[0]!;
    const dy = obs.pos[1]! - state.pos[1]!;
    const dz = obs.pos[2]! - state.pos[2]!;
    const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);

    if (dist < 0.001 || dist > 2.0) continue;

    // Estimate obstacle velocity from position history
    const prev = prevObsPositions.get(oi);
    let obsVx = 0, obsVy = 0, obsVz = 0;
    if (prev) {
      obsVx = (obs.pos[0]! - prev[0]!) * 333; // Δpos × tick rate
      obsVy = (obs.pos[1]! - prev[1]!) * 333;
      obsVz = (obs.pos[2]! - prev[2]!) * 333;
    }
    prevObsPositions.set(oi, Float64Array.from(obs.pos));

    // Relative velocity (drone - obstacle)
    const relVx = state.vel[0]! - obsVx;
    const relVy = state.vel[1]! - obsVy;
    const relVz = state.vel[2]! - obsVz;

    // Closure rate from relative velocity
    const closureRate = -(relVx * dx + relVy * dy + relVz * dz) / dist;

    // Expansion = closure / distance² (looming)
    const loom = closureRate / (dist * dist + 0.01);
    expansion += loom;

    // Lateral flow (which side is the obstacle?)
    const angle = Math.atan2(dy, dx) - state.att[2]!; // Relative to heading
    lateral[0]! += Math.sin(angle) / (dist + 0.1);
    lateral[1]! += dz / (dist * dist + 0.1);
  }

  return {
    expansion,
    lateral,
    rotation: Float64Array.from(state.omega), // Haltere = gyroscope
  };
}

// ─── Fly Brain ──────────────────────────────────────────────────

export interface FlyBrainOutput {
  /** Motor commands [0-1] × 4 */
  motors: Float64Array;
  /** What the brain decided to do */
  action: 'HOVER' | 'ESCAPE' | 'CRUISE' | 'TURN' | 'LAND';
  /** Urgency of the action [0, 1] */
  urgency: number;
}

/**
 * The fly brain controller.
 *
 * Reads optic flow + IMU, produces motor commands.
 * Uses three layers:
 * 1. REFLEX: Looming → immediate escape (< 1 tick)
 * 2. STABILIZE: Haltere feedback → attitude correction
 * 3. NAVIGATE: Goal-seeking via optic flow balance
 */
// Integral accumulators (persistent across ticks — like biological muscle tone)
const integralX = { value: 0 };
const integralY = { value: 0 };
const integralZ = { value: 0 };
// Attitude integral (compensates persistent wind torque)
const integralRoll = { value: 0 };
const integralPitch = { value: 0 };

/** Reset controller state between flights */
export function resetBrain(): void {
  integralX.value = 0;
  integralY.value = 0;
  integralZ.value = 0;
  integralRoll.value = 0;
  integralPitch.value = 0;
}

export function flyBrainTick(
  state: DroneState,
  flow: OpticFlow,
  goal: Float64Array | null,
  config: FlyDroneConfig,
  obstacles: Array<{ pos: Float64Array; radius: number }> = [],
  estimatedAtt?: { roll: number; pitch: number; yaw: number },
): FlyBrainOutput {
  const att = estimatedAtt ?? { roll: state.att[0]!, pitch: state.att[1]!, yaw: state.att[2]! };
  const motors = new Float64Array(4);
  let action: FlyBrainOutput['action'] = 'HOVER';
  let urgency = 0;

  // ── LAYER 1: HOVER BASELINE ────────────────────────
  // Thrust to counteract gravity, COMPENSATED for tilt.
  // When tilted, need more thrust to maintain vertical component:
  // thrust_needed = mg / (cos(roll) * cos(pitch))
  const tiltComp = 1 / (Math.cos(att.roll) * Math.cos(att.pitch) + 0.01);
  const hoverThrust = (config.mass * config.gravity) / (4 * config.maxThrustPerMotor) * Math.min(1.5, tiltComp);
  motors[0] = hoverThrust;
  motors[1] = hoverThrust;
  motors[2] = hoverThrust;
  motors[3] = hoverThrust;

  // ── LAYER 2: STABILIZATION (attitude PD — HIGHEST PRIORITY) ──
  // The attitude controller has ENVELOPE PROTECTION:
  // If the drone is tilted > 15°, navigation is suppressed entirely.
  // This prevents the navigation controller from destabilizing the drone.
  // PREDICTIVE STABILIZATION (quantum-inspired):
  // Instead of reacting to current tilt, PREDICT the tilt 50ms in the future
  // using current angular velocity. If the predicted tilt is dangerous,
  // apply correction NOW — before the instability develops.
  // This is the "branching" insight applied at the reflex level.
  const predictHorizon = 0.015; // 15ms lookahead (5 ticks at 333Hz)
  const predictedRoll = att.roll + flow.rotation[0]! * predictHorizon;
  const predictedPitch = att.pitch + flow.rotation[1]! * predictHorizon;

  const tiltMag = Math.sqrt(predictedRoll ** 2 + predictedPitch ** 2);
  const Kp = Math.min(1.2, 0.4 + tiltMag * 2.0);
  const Kd = Math.min(0.3, 0.08 + tiltMag * 0.4);

  // Integral: accumulates attitude error to counter persistent wind
  integralRoll.value = Math.max(-0.15, Math.min(0.15, integralRoll.value + att.roll * 0.005));
  integralPitch.value = Math.max(-0.15, Math.min(0.15, integralPitch.value + att.pitch * 0.005));

  // Correct toward PREDICTED zero, not current zero
  const rollCorr = -predictedRoll * Kp - flow.rotation[0]! * Kd - integralRoll.value;
  const pitchCorr = -predictedPitch * Kp - flow.rotation[1]! * Kd - integralPitch.value;
  const yawCorr = -flow.rotation[2]! * 0.05;

  // Envelope check: how tilted are we?
  const tiltDeg = Math.sqrt(att.roll ** 2 + att.pitch ** 2) * 180 / Math.PI;
  const navScale = tiltDeg > 10 ? 0 : tiltDeg > 4 ? 0.2 : 0.7; // Conservative: nav suppressed early

  // Apply stabilization (always, full authority)
  motors[0]! -= rollCorr; motors[3]! -= rollCorr;
  motors[1]! += rollCorr; motors[2]! += rollCorr;
  motors[0]! += pitchCorr; motors[1]! += pitchCorr;
  motors[2]! -= pitchCorr; motors[3]! -= pitchCorr;
  motors[0]! += yawCorr; motors[2]! += yawCorr;
  motors[1]! -= yawCorr; motors[3]! -= yawCorr;

  // ── LAYER 2b: PROXIMITY REFLEX (antenna mechanoreceptors) ───
  // Independent of optic flow — pure distance check.
  // Flies have mechanosensory hairs that detect nearby objects
  // via air pressure changes, even without visual motion.
  // This fires BEFORE the looming detector and has zero latency.
  {
    let minObsDist = Infinity;
    for (const obs of obstacles) {
      const d = Math.sqrt(
        (obs.pos[0]! - state.pos[0]!)**2 +
        (obs.pos[1]! - state.pos[1]!)**2 +
        (obs.pos[2]! - state.pos[2]!)**2,
      );
      minObsDist = Math.min(minObsDist, d);
    }
    if (minObsDist < 0.08) { // 8cm = antenna range
      action = 'ESCAPE';
      urgency = 1;
      const boost = 0.5;
      motors[0]! += boost;
      motors[1]! += boost;
      motors[2]! += boost;
      motors[3]! += boost;
    }
  }

  // ── LAYER 3: ESCAPE REFLEX (looming detection) ─────
  if (action !== 'ESCAPE' && flow.expansion > 0.5) {
    // LOOMING! Something approaching fast.
    // ESCAPE: Maximum evasion. Reset integrals (fresh start after dodge).
    integralX.value = 0; integralY.value = 0; integralZ.value = 0;
    action = 'ESCAPE';
    urgency = Math.min(1, flow.expansion / 5);

    // Escape UP + sideways (insects always escape vertically first)
    const escapeBoost = 0.5 * urgency;
    motors[0]! += escapeBoost;
    motors[1]! += escapeBoost;
    motors[2]! += escapeBoost;
    motors[3]! += escapeBoost;

    // Bank HARD away from the looming direction
    const escapeLateral = flow.lateral[0]! > 0 ? -0.4 : 0.4;
    motors[0]! += escapeLateral;
    motors[3]! += escapeLateral;
    motors[1]! -= escapeLateral;
    motors[2]! -= escapeLateral;
  }

  // ── LAYER 4: NAVIGATION (goal-seeking) ─────────────
  else if (goal) {
    const dx = goal[0]! - state.pos[0]!;
    const dy = goal[1]! - state.pos[1]!;
    const dz = goal[2]! - state.pos[2]!;
    const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);

    if (dist > 0.002) {
      // Altitude PID — CRITICAL for stability
      // Uses true altitude (barometer has ~1m resolution on real drones,
      // but ultrasonic rangefinder gives sub-cm for micro-drones at < 5m)
      const altErr = dz;
      integralZ.value = Math.max(-1.0, Math.min(1.0, integralZ.value + altErr * 0.05));
      const altVelDamp = -state.vel[2]! * 1.2;
      const altCorr = Math.max(-0.3, Math.min(0.3, altErr * 5.0 + integralZ.value + altVelDamp));
      motors[0]! += altCorr;
      motors[1]! += altCorr;
      motors[2]! += altCorr;
      motors[3]! += altCorr;

      // Horizontal PID in body frame
      const cy = Math.cos(att.yaw), sy = Math.sin(att.yaw);
      const bodyX = dx * cy + dy * sy;
      const bodyY = -dx * sy + dy * cy;
      const bodyVx = state.vel[0]! * cy + state.vel[1]! * sy;
      const bodyVy = -state.vel[0]! * sy + state.vel[1]! * cy;

      // Accumulate integral (anti-windup clamped)
      // Tight anti-windup: small integral prevents runaway
      integralX.value = Math.max(-0.05, Math.min(0.05, integralX.value + bodyX * 0.001));
      integralY.value = Math.max(-0.05, Math.min(0.05, integralY.value + bodyY * 0.001));

      // PID: proportional + integral + derivative, SCALED by envelope protection
      const rawPitch = bodyX * 0.5 + integralX.value - bodyVx * 0.3;
      const rawRoll = bodyY * 0.5 + integralY.value - bodyVy * 0.3;
      const pitchCmd = Math.max(-0.1, Math.min(0.1, rawPitch)) * navScale;
      const rollCmd = Math.max(-0.1, Math.min(0.1, rawRoll)) * navScale;

      motors[0]! += pitchCmd;
      motors[1]! += pitchCmd;
      motors[2]! -= pitchCmd;
      motors[3]! -= pitchCmd;

      motors[1]! += rollCmd;
      motors[2]! += rollCmd;
      motors[0]! -= rollCmd;
      motors[3]! -= rollCmd;

      action = dist > 0.1 ? 'CRUISE' : 'HOVER';
      urgency = Math.min(1, dist);
    }

    // Yaw toward goal
    const goalYaw = Math.atan2(dy, dx);
    const yawErr = goalYaw - att.yaw;
    // Normalize to [-π, π]
    const normYawErr = Math.atan2(Math.sin(yawErr), Math.cos(yawErr));
    if (Math.abs(normYawErr) > 0.05) {
      const yawCmd = Math.max(-0.05, Math.min(0.05, normYawErr * 0.3));
      motors[0]! += yawCmd;
      motors[2]! += yawCmd;
      motors[1]! -= yawCmd;
      motors[3]! -= yawCmd;
      if (Math.abs(normYawErr) > 0.3) action = 'TURN';
    }
  }

  // ── CLAMP with minimum thrust floor ────────────────
  // Never let any motor go fully to zero — maintains a minimum
  // downward thrust to prevent freefall during corrections
  const motorFloor = hoverThrust * 0.3; // 30% of hover as minimum
  for (let i = 0; i < 4; i++) {
    motors[i] = Math.max(motorFloor, Math.min(1, motors[i]!));
  }

  return { motors, action, urgency };
}
