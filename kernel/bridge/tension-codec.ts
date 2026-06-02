/**
 * OASIS Kernel — Tension Codec
 *
 * THE GROUNDING PROBLEM: How does a 128-dim tension vector
 * become "turn left at 0.5 rad/s"?
 *
 * This is the hardest problem in OASIS. Abstract latent
 * representations must map to concrete physical actions,
 * and physical sensor data must map back to latent tensions.
 *
 * INNOVATION: Semantic Projection Manifolds
 *
 * Instead of a fixed linear mapping (dim 0 = x, dim 1 = y),
 * the codec uses NAMED MANIFOLDS — semantic subspaces within
 * the 128-dim latent space that correspond to physical quantities.
 *
 * Each manifold is a set of dimensions + a projection function:
 * - LOCOMOTION manifold: dims 10-15 → linear/angular velocity
 * - MANIPULATION manifold: dims 16-21 → joint angles/forces
 * - NAVIGATION manifold: dims 22-27 → goal position/heading
 * - PERCEPTION manifold: dims 28-33 → sensor readings
 *
 * Manifolds can OVERLAP (some dims affect both locomotion and
 * navigation). This captures real physical coupling — you can't
 * navigate without moving.
 *
 * The codec is BIDIRECTIONAL:
 * - ENCODE: ROS 2 message → latent tension (physical → abstract)
 * - DECODE: latent tension → ROS 2 command (abstract → physical)
 */

import {
  type Vec,
  zeros,
  norm,
  normalize,
  scale,
  dot,
} from '../physics/vector-math.js';

// ─── ROS 2 Standard Message Types (simplified) ──────────────────

/** geometry_msgs/Twist — velocity command */
export interface TwistMsg {
  linear: { x: number; y: number; z: number };
  angular: { x: number; y: number; z: number };
}

/** nav_msgs/Odometry — position + velocity */
export interface OdometryMsg {
  position: { x: number; y: number; z: number };
  orientation: { roll: number; pitch: number; yaw: number };
  linearVelocity: { x: number; y: number; z: number };
  angularVelocity: { x: number; y: number; z: number };
}

/** sensor_msgs/LaserScan — LiDAR data */
export interface LaserScanMsg {
  angleMin: number;
  angleMax: number;
  angleIncrement: number;
  ranges: number[];
  intensities: number[];
}

/** sensor_msgs/JointState — arm/manipulator state */
export interface JointStateMsg {
  names: string[];
  positions: number[];
  velocities: number[];
  efforts: number[];
}

// ─── Semantic Manifold ──────────────────────────────────────────

export interface SemanticManifold {
  readonly name: string;
  /** Which latent dimensions this manifold occupies */
  readonly dims: number[];
  /** Scale factors for each dim (physical units per latent unit) */
  readonly scales: number[];
  /** Clamping limits per dim [min, max] */
  readonly limits: Array<[number, number]>;
}

// ─── Pre-defined Manifolds ──────────────────────────────────────

export const LOCOMOTION_MANIFOLD: SemanticManifold = {
  name: 'LOCOMOTION',
  dims: [10, 11, 12, 13, 14, 15],
  // [linear.x, linear.y, linear.z, angular.x, angular.y, angular.z]
  scales: [1.0, 1.0, 0.5, 0.5, 0.5, 1.0], // m/s and rad/s
  limits: [[-2, 2], [-1, 1], [-0.5, 0.5], [-1, 1], [-1, 1], [-2, 2]],
};

export const MANIPULATION_MANIFOLD: SemanticManifold = {
  name: 'MANIPULATION',
  dims: [16, 17, 18, 19, 20, 21],
  // [j0_pos, j1_pos, j2_pos, j3_pos, j4_pos, j5_pos] — 6-DOF arm
  scales: [1.0, 1.0, 1.0, 1.0, 1.0, 1.0], // radians
  limits: [[-Math.PI, Math.PI], [-Math.PI, Math.PI], [-Math.PI, Math.PI],
           [-Math.PI, Math.PI], [-Math.PI, Math.PI], [-Math.PI, Math.PI]],
};

export const NAVIGATION_MANIFOLD: SemanticManifold = {
  name: 'NAVIGATION',
  dims: [22, 23, 24, 25, 26, 27],
  // [goal.x, goal.y, goal.z, heading, urgency, confidence]
  scales: [10.0, 10.0, 5.0, Math.PI, 1.0, 1.0], // meters, rad, unitless
  limits: [[-100, 100], [-100, 100], [-10, 10], [-Math.PI, Math.PI], [0, 1], [0, 1]],
};

// ─── Tension Codec ──────────────────────────────────────────────

export class TensionCodec {
  private readonly dim: number;
  private readonly manifolds = new Map<string, SemanticManifold>();

  constructor(dim: number) {
    this.dim = dim;
    // Register default manifolds
    this.registerManifold(LOCOMOTION_MANIFOLD);
    this.registerManifold(MANIPULATION_MANIFOLD);
    this.registerManifold(NAVIGATION_MANIFOLD);
  }

  /** Register a custom semantic manifold */
  registerManifold(manifold: SemanticManifold): void {
    this.manifolds.set(manifold.name, manifold);
  }

  // ─── ENCODE: Physical → Latent ──────────────────────────

  /**
   * Encode a Twist command into a latent tension vector.
   * This is what happens when ROS 2 sends a velocity command
   * to an OASIS agent.
   */
  encodeTwist(twist: TwistMsg): Vec {
    const m = this.manifolds.get('LOCOMOTION')!;
    const vec = zeros(this.dim);

    const values = [
      twist.linear.x, twist.linear.y, twist.linear.z,
      twist.angular.x, twist.angular.y, twist.angular.z,
    ];

    for (let i = 0; i < m.dims.length && i < values.length; i++) {
      const d = m.dims[i]!;
      const s = m.scales[i]!;
      const [min, max] = m.limits[i]!;
      // Normalize to latent scale and clamp
      vec[d] = Math.max(min, Math.min(max, values[i]!)) / s;
    }

    return vec;
  }

  /**
   * Encode odometry (position + velocity) into a latent tension.
   * This is how proprioceptive data enters the latent space.
   */
  encodeOdometry(odom: OdometryMsg): Vec {
    const nav = this.manifolds.get('NAVIGATION')!;
    const loco = this.manifolds.get('LOCOMOTION')!;
    const vec = zeros(this.dim);

    // Position → navigation manifold
    const navVals = [
      odom.position.x, odom.position.y, odom.position.z,
      odom.orientation.yaw, 0.5, 1.0,
    ];
    for (let i = 0; i < nav.dims.length && i < navVals.length; i++) {
      vec[nav.dims[i]!] = navVals[i]! / nav.scales[i]!;
    }

    // Velocity → locomotion manifold
    const locoVals = [
      odom.linearVelocity.x, odom.linearVelocity.y, odom.linearVelocity.z,
      odom.angularVelocity.x, odom.angularVelocity.y, odom.angularVelocity.z,
    ];
    for (let i = 0; i < loco.dims.length && i < locoVals.length; i++) {
      vec[loco.dims[i]!] = locoVals[i]! / loco.scales[i]!;
    }

    return vec;
  }

  /**
   * Encode a LiDAR scan into a latent tension.
   * Compressed: the full scan becomes a directional repulsion vector.
   */
  encodeLaserScan(scan: LaserScanMsg): Vec {
    const vec = zeros(this.dim);
    if (scan.ranges.length === 0) return vec;

    // Find closest obstacle and its angle
    let minRange = Infinity;
    let minAngle = 0;

    for (let i = 0; i < scan.ranges.length; i++) {
      const range = scan.ranges[i]!;
      if (range > 0.01 && range < minRange) {
        minRange = range;
        minAngle = scan.angleMin + i * scan.angleIncrement;
      }
    }

    // Convert to repulsive tension in locomotion manifold
    // Closer obstacle = stronger repulsion away from that direction
    if (minRange < 10) {
      const repulsion = 1 / (minRange * minRange + 0.1);
      const m = this.manifolds.get('LOCOMOTION')!;

      // Push away from obstacle direction
      if (m.dims[0] !== undefined) vec[m.dims[0]] = -repulsion * Math.cos(minAngle);
      if (m.dims[1] !== undefined) vec[m.dims[1]] = -repulsion * Math.sin(minAngle);
    }

    return vec;
  }

  /**
   * Encode joint states into a latent tension.
   */
  encodeJointState(joints: JointStateMsg): Vec {
    const m = this.manifolds.get('MANIPULATION')!;
    const vec = zeros(this.dim);

    for (let i = 0; i < joints.positions.length && i < m.dims.length; i++) {
      vec[m.dims[i]!] = joints.positions[i]! / m.scales[i]!;
    }

    return vec;
  }

  // ─── DECODE: Latent → Physical ──────────────────────────

  /**
   * Decode a latent tension vector into a Twist command.
   * This is what happens when OASIS sends a motor command
   * to ROS 2 actuators.
   */
  decodeTwist(tension: Vec): TwistMsg {
    const m = this.manifolds.get('LOCOMOTION')!;

    const values: number[] = [];
    for (let i = 0; i < m.dims.length; i++) {
      const d = m.dims[i]!;
      const s = m.scales[i]!;
      const [min, max] = m.limits[i]!;
      const raw = (tension[d] ?? 0) * s;
      values.push(Math.max(min, Math.min(max, raw)));
    }

    return {
      linear: { x: values[0] ?? 0, y: values[1] ?? 0, z: values[2] ?? 0 },
      angular: { x: values[3] ?? 0, y: values[4] ?? 0, z: values[5] ?? 0 },
    };
  }

  /**
   * Decode a latent tension into joint commands.
   */
  decodeJointState(tension: Vec, jointNames: string[]): JointStateMsg {
    const m = this.manifolds.get('MANIPULATION')!;

    const positions: number[] = [];
    const velocities: number[] = [];
    const efforts: number[] = [];

    for (let i = 0; i < jointNames.length && i < m.dims.length; i++) {
      const d = m.dims[i]!;
      const s = m.scales[i]!;
      const [min, max] = m.limits[i]!;
      const raw = (tension[d] ?? 0) * s;
      positions.push(Math.max(min, Math.min(max, raw)));
      velocities.push(0); // Velocity control is through locomotion manifold
      efforts.push(0);
    }

    return { names: jointNames, positions, velocities, efforts };
  }

  /**
   * Extract navigation goal from a latent tension.
   */
  decodeNavigationGoal(tension: Vec): {
    position: { x: number; y: number; z: number };
    heading: number;
    urgency: number;
    confidence: number;
  } {
    const m = this.manifolds.get('NAVIGATION')!;

    const values: number[] = [];
    for (let i = 0; i < m.dims.length; i++) {
      const d = m.dims[i]!;
      const s = m.scales[i]!;
      values.push((tension[d] ?? 0) * s);
    }

    return {
      position: { x: values[0] ?? 0, y: values[1] ?? 0, z: values[2] ?? 0 },
      heading: values[3] ?? 0,
      urgency: Math.max(0, Math.min(1, values[4] ?? 0)),
      confidence: Math.max(0, Math.min(1, values[5] ?? 0)),
    };
  }

  /** Get a registered manifold */
  getManifold(name: string): SemanticManifold | undefined {
    return this.manifolds.get(name);
  }

  /** List all registered manifolds */
  getManifoldNames(): string[] {
    return [...this.manifolds.keys()];
  }
}
