/**
 * OASIS Swarm — Drone Agent Model
 *
 * Each drone is a node in the nervous system.
 * OASIS doesn't fly the drone — it coordinates WHERE drones go,
 * WHO covers WHAT area, and WHEN to reorganize.
 *
 * 3D positions. Communication range. Battery. Sensor coverage.
 * The drone's autopilot handles low-level flight (that's the HAL).
 * OASIS handles high-level coordination (that's the brain).
 */

export interface DroneAgent {
  readonly id: string;
  /** Position [x, y, z] in meters */
  pos: Float64Array;
  /** Velocity [vx, vy, vz] in m/s */
  vel: Float64Array;
  /** Assigned role */
  role: 'SCOUT' | 'MAPPER' | 'RELAY' | 'IDLE';
  /** Battery level [0, 1] */
  battery: number;
  /** Is this drone operational? */
  alive: boolean;
  /** Communication range in meters */
  commRange: number;
  /** Sensor coverage radius in meters (ground projection) */
  sensorRadius: number;
  /** Assigned waypoint [x, y, z] or null */
  waypoint: Float64Array | null;
  /** Time in seconds */
  time: number;
}

export interface SwarmConfig {
  /** Minimum separation between drones in meters */
  minSeparation: number;
  /** Maximum altitude in meters (geofence ceiling — R4) */
  maxAltitude: number;
  /** Minimum altitude in meters */
  minAltitude: number;
  /** Geofence boundary [minX, minY, maxX, maxY] in meters */
  geofence: [number, number, number, number];
  /** Communication range in meters */
  commRange: number;
  /** Cruise speed in m/s */
  cruiseSpeed: number;
  /** Battery drain rate per second */
  batteryDrainRate: number;
  /** Battery level below which drone must return to base */
  batteryMinimum: number;
}

export const SURVEY_SWARM: SwarmConfig = {
  minSeparation: 5,
  maxAltitude: 120, // Legal limit in most countries
  minAltitude: 10,
  geofence: [-200, -200, 200, 200], // 400m × 400m area
  commRange: 150,
  cruiseSpeed: 8, // ~29 km/h
  batteryDrainRate: 0.001, // ~16 min flight time
  batteryMinimum: 0.15,
};

export function createDrone(id: string, x: number, y: number, z: number): DroneAgent {
  return {
    id, pos: new Float64Array([x, y, z]),
    vel: new Float64Array(3), role: 'IDLE', battery: 1.0,
    alive: true, commRange: SURVEY_SWARM.commRange,
    sensorRadius: 30, waypoint: null, time: 0,
  };
}

/** Move drone toward its waypoint at cruise speed */
export function stepDrone(
  drone: DroneAgent, commandVel: Float64Array, dt: number, config: SwarmConfig,
): void {
  if (!drone.alive) return;

  // Apply velocity (limited by cruise speed)
  const speed = Math.sqrt(commandVel[0]! ** 2 + commandVel[1]! ** 2 + commandVel[2]! ** 2);
  const scale = speed > config.cruiseSpeed ? config.cruiseSpeed / speed : 1;

  for (let i = 0; i < 3; i++) {
    drone.vel[i] = commandVel[i]! * scale;
    drone.pos[i]! += drone.vel[i]! * dt;
  }

  // R4: Geofence enforcement
  drone.pos[0] = Math.max(config.geofence[0], Math.min(config.geofence[2], drone.pos[0]!));
  drone.pos[1] = Math.max(config.geofence[1], Math.min(config.geofence[3], drone.pos[1]!));
  drone.pos[2] = Math.max(config.minAltitude, Math.min(config.maxAltitude, drone.pos[2]!));

  // Battery drain
  drone.battery -= config.batteryDrainRate * dt;
  if (drone.battery <= 0) { drone.battery = 0; drone.alive = false; }

  drone.time += dt;
}

/** Euclidean distance between two drones */
export function droneDistance(a: DroneAgent, b: DroneAgent): number {
  return Math.sqrt(
    (a.pos[0]! - b.pos[0]!) ** 2 +
    (a.pos[1]! - b.pos[1]!) ** 2 +
    (a.pos[2]! - b.pos[2]!) ** 2,
  );
}

/** Can two drones communicate? */
export function canCommunicate(a: DroneAgent, b: DroneAgent): boolean {
  return droneDistance(a, b) <= Math.min(a.commRange, b.commRange);
}

/** Compute coverage area of the swarm (unique ground area covered in m²) */
export function computeCoverage(
  drones: DroneAgent[], gridResolution = 5,
  bounds: [number, number, number, number] = [-200, -200, 200, 200],
): { coveredArea: number; totalArea: number; ratio: number } {
  const [minX, minY, maxX, maxY] = bounds;
  const gridW = Math.ceil((maxX - minX) / gridResolution);
  const gridH = Math.ceil((maxY - minY) / gridResolution);
  const totalCells = gridW * gridH;
  let coveredCells = 0;

  for (let gx = 0; gx < gridW; gx++) {
    for (let gy = 0; gy < gridH; gy++) {
      const cx = minX + (gx + 0.5) * gridResolution;
      const cy = minY + (gy + 0.5) * gridResolution;

      for (const d of drones) {
        if (!d.alive) continue;
        const dist = Math.sqrt((d.pos[0]! - cx) ** 2 + (d.pos[1]! - cy) ** 2);
        if (dist <= d.sensorRadius) { coveredCells++; break; }
      }
    }
  }

  const cellArea = gridResolution ** 2;
  return {
    coveredArea: coveredCells * cellArea,
    totalArea: totalCells * cellArea,
    ratio: coveredCells / totalCells,
  };
}
