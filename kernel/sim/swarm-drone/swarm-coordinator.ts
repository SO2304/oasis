/**
 * OASIS Swarm — Coordinator (Nervous System)
 *
 * NOT a central controller. A DISTRIBUTED nervous system that
 * computes what each drone SHOULD do based on local information.
 *
 * Applies OASIS principles:
 * - P1: Drones discover neighbors by comm range (mesh)
 * - P4: 3D geofencing, altitude limits
 * - R2: No direct drone-to-drone commands (tension field)
 * - R14: Entropy gate (ambiguous situation → hold position)
 * - R15: Lost drone → entropy spike → swarm fills the gap
 * - Claim 6: Morphogenesis — drones self-specialize
 *
 * THREE BEHAVIORS (each drone computes locally):
 * 1. SEPARATION: Don't collide (reflex, always active)
 * 2. COVERAGE: Maximize sensed area (Voronoi-like spreading)
 * 3. CONNECTIVITY: Stay within comm range of at least 1 neighbor
 */

import type { DroneAgent, SwarmConfig } from './drone-agent.js';
import { droneDistance, canCommunicate } from './drone-agent.js';

export interface SwarmCommand {
  readonly droneId: string;
  readonly velocity: Float64Array;
  readonly role: DroneAgent['role'];
  readonly reason: 'SEPARATE' | 'SPREAD' | 'CONNECT' | 'RETURN_BASE' | 'HOLD' | 'PATROL';
}

export interface SwarmMetrics {
  activeDrones: number;
  minSeparation: number;
  networkConnected: boolean;
  disconnectedDrones: string[];
  coverageRatio: number;
  meanBattery: number;
  collisions: number;
  entropy: number;
}

export class SwarmCoordinator {
  private readonly config: SwarmConfig;
  private readonly basePos: Float64Array;

  constructor(config: SwarmConfig, baseX = 0, baseY = 0) {
    this.config = config;
    this.basePos = new Float64Array([baseX, baseY, 0]);
  }

  tick(drones: DroneAgent[]): { commands: SwarmCommand[]; metrics: SwarmMetrics } {
    const alive = drones.filter(d => d.alive);
    const commands: SwarmCommand[] = [];

    // ── METRICS ──────────────────────────────────────
    let minSep = Infinity;
    let collisions = 0;
    const disconnected: string[] = [];

    // Pairwise distances
    for (let i = 0; i < alive.length; i++) {
      for (let j = i + 1; j < alive.length; j++) {
        const dist = droneDistance(alive[i]!, alive[j]!);
        minSep = Math.min(minSep, dist);
        if (dist < 1.0) collisions++; // Physical contact: < 1m for a drone
      }
    }

    // Network connectivity (BFS from first drone)
    const connected = new Set<string>();
    if (alive.length > 0) {
      const queue = [alive[0]!.id];
      connected.add(alive[0]!.id);
      while (queue.length > 0) {
        const current = queue.shift()!;
        const currentDrone = alive.find(d => d.id === current)!;
        for (const other of alive) {
          if (!connected.has(other.id) && canCommunicate(currentDrone, other)) {
            connected.add(other.id);
            queue.push(other.id);
          }
        }
      }
    }
    for (const d of alive) {
      if (!connected.has(d.id)) disconnected.push(d.id);
    }

    // Entropy: based on collisions + disconnections
    const entropy = alive.length > 1
      ? (collisions + disconnected.length) / alive.length
      : 0;

    const meanBattery = alive.length > 0
      ? alive.reduce((s, d) => s + d.battery, 0) / alive.length
      : 0;

    // ── COMMANDS ─────────────────────────────────────
    for (const drone of alive) {
      const vel = new Float64Array(3);
      let reason: SwarmCommand['reason'] = 'PATROL';
      let role: DroneAgent['role'] = drone.role === 'IDLE' ? 'SCOUT' : drone.role;

      // ── REFLEX: Battery low → return to base ──────
      if (drone.battery < this.config.batteryMinimum) {
        const toBase = [
          this.basePos[0]! - drone.pos[0]!,
          this.basePos[1]! - drone.pos[1]!,
          this.config.minAltitude - drone.pos[2]!,
        ];
        const dist = Math.sqrt(toBase[0]! ** 2 + toBase[1]! ** 2 + toBase[2]! ** 2);
        if (dist > 1) {
          vel[0] = toBase[0]! / dist * this.config.cruiseSpeed;
          vel[1] = toBase[1]! / dist * this.config.cruiseSpeed;
          vel[2] = toBase[2]! / dist * this.config.cruiseSpeed * 0.5;
        }
        reason = 'RETURN_BASE';
        role = 'IDLE';
        commands.push({ droneId: drone.id, velocity: vel, role, reason });
        continue;
      }

      // ── R14: High entropy → hold position ─────────
      if (entropy > 0.5) {
        reason = 'HOLD';
        commands.push({ droneId: drone.id, velocity: vel, role, reason });
        continue;
      }

      // ── UNIFIED POTENTIAL FIELD (Lennard-Jones inspired) ──
      //
      // Instead of 3 separate behaviors with ad-hoc blending,
      // ONE force law that produces all three effects:
      //
      //   F(d) = A/d³ - B/d     (modified Lennard-Jones)
      //          ↑repel   ↑attract
      //
      // d < d_eq: repulsion dominates (separation)
      // d = d_eq: equilibrium (optimal spacing)
      // d > d_eq: attraction dominates (connectivity)
      // d > commRange: zero (out of range)
      //
      // d_eq is chosen so drones naturally spread to maximize
      // coverage while maintaining communication links.
      //
      // This is a SINGLE force computation per neighbor pair.
      // No mode switching. No priority blending. Pure physics.

      const dEq = this.config.commRange * 0.4; // Equilibrium: 40% of comm range
      const A = dEq * dEq * dEq * 2; // Repulsion coefficient (tuned so F(minSep) is strong)
      const B = dEq; // Attraction coefficient

      let forceX = 0, forceY = 0, forceZ = 0;
      let neighborCount = 0;

      for (const other of alive) {
        if (other.id === drone.id) continue;
        const dist = droneDistance(drone, other);
        if (dist < 0.5 || dist > this.config.commRange * 1.2) continue;

        neighborCount++;

        // Direction from other to this drone
        const dx = (drone.pos[0]! - other.pos[0]!) / dist;
        const dy = (drone.pos[1]! - other.pos[1]!) / dist;
        const dz = (drone.pos[2]! - other.pos[2]!) / dist;

        // Lennard-Jones force: F = A/d³ - B/d
        const repel = A / (dist * dist * dist);
        const attract = B / dist;
        const netForce = repel - attract;

        // Net: positive = push away, negative = pull toward
        forceX += dx * netForce;
        forceY += dy * netForce;
        forceZ += dz * netForce * 0.3; // Weaker in Z (altitude)
      }

      // Disconnected: strong pull toward nearest
      if (!connected.has(drone.id) && neighborCount === 0) {
        let nearestDist = Infinity;
        let nearest: DroneAgent | null = null;
        for (const other of alive) {
          if (other.id === drone.id) continue;
          const d = droneDistance(drone, other);
          if (d < nearestDist) { nearestDist = d; nearest = other; }
        }
        if (nearest && nearestDist > 0.5) {
          forceX += (nearest.pos[0]! - drone.pos[0]!) / nearestDist * 5;
          forceY += (nearest.pos[1]! - drone.pos[1]!) / nearestDist * 5;
          reason = 'CONNECT';
        }
      }

      // Normalize and apply cruise speed
      const forceMag = Math.sqrt(forceX ** 2 + forceY ** 2 + forceZ ** 2);
      if (forceMag > 0.01) {
        const speed = Math.min(this.config.cruiseSpeed, forceMag * 2);
        vel[0] = forceX / forceMag * speed;
        vel[1] = forceY / forceMag * speed;
        vel[2] = forceZ / forceMag * speed * 0.3;
        reason = forceMag > 3 ? 'SEPARATE' : 'SPREAD';
      }

      // Morphogenesis (Claim 6): Assign role based on position
      if (neighborCount === 0) role = 'RELAY'; // Isolated → become relay
      else if (neighborCount >= 3) role = 'MAPPER'; // Well-connected → map
      else role = 'SCOUT'; // Edge of network → scout

      commands.push({ droneId: drone.id, velocity: vel, role, reason });
    }

    return {
      commands,
      metrics: {
        activeDrones: alive.length,
        minSeparation: minSep === Infinity ? 0 : minSep,
        networkConnected: disconnected.length === 0,
        disconnectedDrones: disconnected,
        coverageRatio: 0, // Computed by caller
        meanBattery: meanBattery,
        collisions,
        entropy,
      },
    };
  }
}
