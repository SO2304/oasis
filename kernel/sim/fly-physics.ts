/**
 * OASIS Kernel — Fly-Scale Drone Physics
 *
 * A housefly is the most agile flyer in nature:
 * - 200Hz wingbeat
 * - 30ms reaction time (visual cortex → motor)
 * - 10g acceleration (98 m/s²)
 * - 3000°/s banked turns
 * - Hovers, reverses, barrel-rolls, lands on ceilings
 *
 * This module simulates a drone with fly-grade dynamics.
 * The physics model is a 6-DOF rigid body with:
 * - 4 rotors (quadcopter) modeled as force+torque vectors
 * - Aerodynamic drag proportional to v²
 * - Gravity
 * - Angular momentum (gyroscopic effects)
 *
 * All units are SI: meters, seconds, radians, Newtons.
 * Precision: sub-millimeter position, sub-degree attitude.
 */

// ─── State ──────────────────────────────────────────────────────

export interface DroneState {
  /** Position [x, y, z] in meters */
  pos: Float64Array; // 3
  /** Velocity [vx, vy, vz] in m/s */
  vel: Float64Array; // 3
  /** Euler angles [roll, pitch, yaw] in radians */
  att: Float64Array; // 3
  /** Angular velocity [wx, wy, wz] in rad/s */
  omega: Float64Array; // 3
  /** Motor speeds [m0, m1, m2, m3] in rad/s (0-1 normalized) */
  motors: Float64Array; // 4
  /** Timestamp in seconds */
  time: number;
}

// ─── Config ─────────────────────────────────────────────────────

export interface FlyDroneConfig {
  /** Total mass in kg */
  readonly mass: number;
  /** Arm length (center to motor) in meters */
  readonly armLength: number;
  /** Max thrust per motor in Newtons */
  readonly maxThrustPerMotor: number;
  /** Drag coefficient */
  readonly dragCoeff: number;
  /** Moment of inertia [Ixx, Iyy, Izz] in kg·m² */
  readonly inertia: Float64Array;
  /** Motor response time constant (how fast motors reach target) */
  readonly motorTau: number;
  /** Gravity in m/s² */
  readonly gravity: number;
}

/** Fly-scale micro-drone (30mm, 12g) */
export const FLY_DRONE_CONFIG: FlyDroneConfig = {
  mass: 0.012,          // 12 grams
  armLength: 0.015,     // 15mm center-to-motor
  maxThrustPerMotor: 0.04, // 40mN per motor (total ~13g thrust at hover)
  dragCoeff: 0.001,     // Low drag at this scale
  inertia: new Float64Array([1e-7, 1e-7, 2e-7]), // Tiny inertia
  motorTau: 0.005,      // 5ms motor response (fast micro-motors)
  gravity: 9.81,
};

/** Larger indoor drone (250mm, 500g) for comparison */
export const INDOOR_DRONE_CONFIG: FlyDroneConfig = {
  mass: 0.5,
  armLength: 0.125,
  maxThrustPerMotor: 3.0,
  dragCoeff: 0.05,
  inertia: new Float64Array([0.001, 0.001, 0.002]),
  motorTau: 0.02,
  gravity: 9.81,
};

// ─── Physics Engine ─────────────────────────────────────────────

export function createDroneState(): DroneState {
  return {
    pos: new Float64Array(3),
    vel: new Float64Array(3),
    att: new Float64Array(3),
    omega: new Float64Array(3),
    motors: new Float64Array([0.5, 0.5, 0.5, 0.5]), // Hover thrust
    time: 0,
  };
}

/**
 * Step the physics simulation by dt seconds.
 *
 * Motor layout (top view):
 *     M0(CW)    M1(CCW)
 *        \      /
 *         CENTER
 *        /      \
 *     M3(CCW)   M2(CW)
 *
 * Forces and torques in BODY frame, then rotated to world.
 * Integration: semi-implicit Euler (good enough at 333Hz+).
 */
export function stepPhysics(
  state: DroneState,
  motorCommands: Float64Array,
  dt: number,
  config: FlyDroneConfig,
  externalForce?: Float64Array, // [fx, fy, fz] in Newtons (wind, etc.)
): void {
  const { mass, armLength, maxThrustPerMotor, dragCoeff, inertia, motorTau, gravity } = config;

  // 1. Motor dynamics (first-order lag)
  for (let i = 0; i < 4; i++) {
    const target = Math.max(0, Math.min(1, motorCommands[i]!));
    state.motors[i]! += (target - state.motors[i]!) * (dt / motorTau);
  }

  // 2. Compute thrust per motor
  const t0 = state.motors[0]! * maxThrustPerMotor;
  const t1 = state.motors[1]! * maxThrustPerMotor;
  const t2 = state.motors[2]! * maxThrustPerMotor;
  const t3 = state.motors[3]! * maxThrustPerMotor;

  const totalThrust = t0 + t1 + t2 + t3;

  // 3. Torques from differential thrust (body frame)
  const tauRoll = armLength * (t1 + t2 - t0 - t3); // Right - Left
  const tauPitch = armLength * (t0 + t1 - t2 - t3); // Front - Rear
  const tauYaw = 0.01 * (t0 - t1 + t2 - t3); // CW - CCW reaction torque

  // 4. Angular acceleration (body frame): α = I⁻¹ × τ
  const alphaRoll = tauRoll / inertia[0]!;
  const alphaPitch = tauPitch / inertia[1]!;
  const alphaYaw = tauYaw / inertia[2]!;

  // 5. Update angular velocity
  state.omega[0]! += alphaRoll * dt;
  state.omega[1]! += alphaPitch * dt;
  state.omega[2]! += alphaYaw * dt;

  // Angular damping (simplified air resistance on rotation)
  for (let i = 0; i < 3; i++) {
    state.omega[i]! *= (1 - 0.1 * dt);
  }

  // 6. Update attitude
  state.att[0]! += state.omega[0]! * dt; // roll
  state.att[1]! += state.omega[1]! * dt; // pitch
  state.att[2]! += state.omega[2]! * dt; // yaw

  // Wrap yaw to [-π, π]
  while (state.att[2]! > Math.PI) state.att[2]! -= 2 * Math.PI;
  while (state.att[2]! < -Math.PI) state.att[2]! += 2 * Math.PI;

  // 7. Thrust vector in world frame (rotate by roll/pitch)
  const cr = Math.cos(state.att[0]!), sr = Math.sin(state.att[0]!);
  const cp = Math.cos(state.att[1]!), sp = Math.sin(state.att[1]!);

  const thrustWorld = [
    totalThrust * sp,          // X component (pitch → forward)
    totalThrust * (-sr * cp),  // Y component (roll → lateral)
    totalThrust * cr * cp,     // Z component (vertical)
  ];

  // 8. Forces: thrust + gravity + drag + external (wind)
  const extFx = externalForce ? externalForce[0]! : 0;
  const extFy = externalForce ? externalForce[1]! : 0;
  const extFz = externalForce ? externalForce[2]! : 0;

  const ax = thrustWorld[0]! / mass + extFx / mass - dragCoeff * state.vel[0]! * Math.abs(state.vel[0]!);
  const ay = thrustWorld[1]! / mass + extFy / mass - dragCoeff * state.vel[1]! * Math.abs(state.vel[1]!);
  const az = thrustWorld[2]! / mass + extFz / mass - gravity - dragCoeff * state.vel[2]! * Math.abs(state.vel[2]!);

  // 9. Semi-implicit Euler integration
  state.vel[0]! += ax * dt;
  state.vel[1]! += ay * dt;
  state.vel[2]! += az * dt;

  state.pos[0]! += state.vel[0]! * dt;
  state.pos[1]! += state.vel[1]! * dt;
  state.pos[2]! += state.vel[2]! * dt;

  // 10. Ground collision (simple)
  if (state.pos[2]! < 0) {
    state.pos[2] = 0;
    state.vel[2] = 0;
    state.vel[0]! *= 0.5; // Friction
    state.vel[1]! *= 0.5;
  }

  state.time += dt;
}

/** Compute speed magnitude in m/s */
export function speed(state: DroneState): number {
  return Math.sqrt(
    state.vel[0]! ** 2 + state.vel[1]! ** 2 + state.vel[2]! ** 2,
  );
}

/** Compute altitude in meters */
export function altitude(state: DroneState): number {
  return state.pos[2]!;
}

/** Distance from origin in meters */
export function distanceFromOrigin(state: DroneState): number {
  return Math.sqrt(
    state.pos[0]! ** 2 + state.pos[1]! ** 2 + state.pos[2]! ** 2,
  );
}
