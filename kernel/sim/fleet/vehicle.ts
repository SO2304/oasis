/**
 * OASIS Fleet — Vehicle Model
 *
 * OASIS is NOT the car. It's the NERVOUS SYSTEM of the fleet.
 * Each vehicle is an endpoint — like a limb. OASIS coordinates
 * the limbs, detects anomalies, and keeps them from colliding.
 *
 * Vehicle state: position, velocity, heading, destination.
 * No tire physics, no engine model. That's the car's job.
 * OASIS only needs to know WHERE they are and WHERE they're going.
 *
 * Units: meters, seconds, radians.
 */

export interface VehicleState {
  readonly id: string;
  /** Position [x, y] in meters */
  pos: Float64Array;
  /** Velocity [vx, vy] in m/s */
  vel: Float64Array;
  /** Heading in radians (0 = +X, π/2 = +Y) */
  heading: number;
  /** Speed in m/s */
  speed: number;
  /** Destination [x, y] in meters */
  destination: Float64Array;
  /** Is this vehicle active? */
  active: boolean;
  /** Timestamp in seconds */
  time: number;
}

export interface FleetConfig {
  /** Minimum safe distance between vehicles in meters */
  safeDistance: number;
  /** Maximum speed in m/s */
  maxSpeed: number;
  /** Maximum acceleration in m/s² */
  maxAccel: number;
  /** Maximum deceleration in m/s² (braking) */
  maxDecel: number;
  /** Vehicle length in meters (for collision box) */
  vehicleLength: number;
  /** Communication range in meters */
  commRange: number;
}

export const URBAN_FLEET: FleetConfig = {
  safeDistance: 8.0,     // 8m minimum gap
  maxSpeed: 13.9,        // 50 km/h
  maxAccel: 3.0,         // 0-50 in ~4.6s
  maxDecel: 8.0,         // Emergency braking
  vehicleLength: 4.5,    // Standard car
  commRange: 200,        // V2V range
};

export function createVehicle(
  id: string,
  x: number, y: number,
  destX: number, destY: number,
): VehicleState {
  return {
    id,
    pos: new Float64Array([x, y]),
    vel: new Float64Array(2),
    heading: Math.atan2(destY - y, destX - x),
    speed: 0,
    destination: new Float64Array([destX, destY]),
    active: true,
    time: 0,
  };
}

/**
 * Step vehicle physics. Simple kinematic model.
 * The vehicle tries to reach its destination at cruise speed,
 * adjusting heading and speed based on OASIS commands.
 *
 * @param commandSpeed Target speed from OASIS (m/s)
 * @param commandHeading Target heading from OASIS (radians)
 */
export function stepVehicle(
  v: VehicleState,
  commandSpeed: number,
  commandHeading: number,
  dt: number,
  config: FleetConfig,
): void {
  // Speed control (limited acceleration/deceleration)
  const speedDiff = commandSpeed - v.speed;
  const maxDelta = speedDiff > 0
    ? config.maxAccel * dt
    : config.maxDecel * dt;
  v.speed += Math.max(-maxDelta, Math.min(maxDelta, speedDiff));
  v.speed = Math.max(0, Math.min(config.maxSpeed, v.speed));

  // Heading control (max turn rate proportional to speed)
  const maxTurnRate = v.speed > 0.5 ? 0.5 : 2.0; // rad/s
  const headingDiff = normalizeAngle(commandHeading - v.heading);
  v.heading += Math.max(-maxTurnRate * dt, Math.min(maxTurnRate * dt, headingDiff));

  // Update velocity and position
  v.vel[0] = v.speed * Math.cos(v.heading);
  v.vel[1] = v.speed * Math.sin(v.heading);
  v.pos[0]! += v.vel[0]! * dt;
  v.pos[1]! += v.vel[1]! * dt;

  // Check arrival
  const distToDest = Math.sqrt(
    (v.destination[0]! - v.pos[0]!) ** 2 +
    (v.destination[1]! - v.pos[1]!) ** 2,
  );
  if (distToDest < 2.0) v.active = false; // Arrived

  v.time += dt;
}

function normalizeAngle(a: number): number {
  while (a > Math.PI) a -= 2 * Math.PI;
  while (a < -Math.PI) a += 2 * Math.PI;
  return a;
}

/** Distance between two vehicles in meters */
export function vehicleDistance(a: VehicleState, b: VehicleState): number {
  return Math.sqrt(
    (a.pos[0]! - b.pos[0]!) ** 2 + (a.pos[1]! - b.pos[1]!) ** 2,
  );
}

/** Time to collision between two vehicles (seconds, Infinity if no collision) */
export function timeToCollision(a: VehicleState, b: VehicleState): number {
  const dx = b.pos[0]! - a.pos[0]!;
  const dy = b.pos[1]! - a.pos[1]!;
  const dvx = b.vel[0]! - a.vel[0]!;
  const dvy = b.vel[1]! - a.vel[1]!;

  // Quadratic: |pos + vel*t|² = safeDistance²
  const A = dvx * dvx + dvy * dvy;
  const B = 2 * (dx * dvx + dy * dvy);
  const dist = Math.sqrt(dx * dx + dy * dy);

  if (A < 1e-10) return Infinity; // Same velocity
  if (B >= 0) return Infinity; // Moving apart

  // Time when distance is minimal
  const tMin = -B / (2 * A);
  if (tMin < 0) return Infinity;

  // Distance at tMin
  const dxMin = dx + dvx * tMin;
  const dyMin = dy + dvy * tMin;
  const distMin = Math.sqrt(dxMin * dxMin + dyMin * dyMin);

  return distMin < 8.0 ? tMin : Infinity; // 8m safety envelope
}
