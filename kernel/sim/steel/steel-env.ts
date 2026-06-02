/**
 * OASIS Steel Mill — Environment Model
 *
 * A steel mill has ZONES with strict access rules.
 * Unlike a mine (tunnel graph) or a field (open space),
 * a steel mill is a 2D floor plan with THERMAL ZONES
 * and MOVING OVERHEAD HAZARDS (crane/ladle).
 *
 * Zone types:
 * - GREEN: Safe. Robots work freely.
 * - YELLOW: Elevated temp (40-80°C). Time-limited access.
 * - RED: Active casting/pouring. NO ENTRY. EVER.
 * - CRANE: Moving overhead load. Clear the floor below.
 *
 * Production cycle (40 min):
 * 1. CHARGE (5 min): Scrap loaded into furnace. Zone = YELLOW.
 * 2. MELT (20 min): Furnace active. Zone = RED (1500°C).
 * 3. TAP (5 min): Metal poured into ladle. Zone = RED + CRANE.
 * 4. CAST (10 min): Ladle moves to caster. CRANE path = RED.
 *
 * All temperatures from real steel mill data (ArcelorMittal safety manual).
 */

export interface Zone {
  readonly id: string;
  /** Mutable: changes with production phase */
  type: 'GREEN' | 'YELLOW' | 'RED' | 'CRANE_PATH';
  /** Zone boundary [minX, minY, maxX, maxY] in meters */
  readonly bounds: [number, number, number, number];
  /** Ground temperature in °C */
  temperature: number;
  /** Is an overhead crane currently above this zone? */
  craneAbove: boolean;
  /** Is molten metal present? */
  moltenMetal: boolean;
  /** Max allowed time in zone in seconds (YELLOW zones) */
  readonly maxExposureS: number;
}

export type ProductionPhase = 'CHARGE' | 'MELT' | 'TAP' | 'CAST' | 'IDLE';

export interface MillState {
  currentPhase: ProductionPhase;
  phaseTimeS: number;
  cycleCount: number;
  zones: Zone[];
  cranePos: Float64Array; // [x, y] of crane center
  craneMoving: boolean;
  craneLoadTons: number;
}

export interface SteelRobot {
  readonly id: string;
  pos: Float64Array;
  role: 'TRANSPORT' | 'INSPECTION' | 'MAINTENANCE' | 'EVACUATING' | 'IDLE';
  currentZone: string;
  /** Time spent in current YELLOW zone in seconds */
  yellowExposureS: number;
  /** Internal temperature of robot in °C */
  internalTemp: number;
  alive: boolean;
  humansNearby: number;
  /** Carrying a load? (e.g., scrap, tools) */
  carryingLoad: boolean;
}

export interface SteelConfig {
  /** Max robot internal temperature before shutdown */
  maxRobotTemp: number;
  /** Max time in YELLOW zone in seconds */
  maxYellowExposure: number;
  /** Safe distance from molten metal in meters */
  meltSafeDistance: number;
  /** Crane clearance radius in meters */
  craneClearRadius: number;
  /** Robot speed in m/s */
  robotSpeed: number;
  /** Human safe distance from robots in meters */
  humanSafeDistance: number;
}

export const STEEL_MILL_CONFIG: SteelConfig = {
  maxRobotTemp: 85,
  maxYellowExposure: 120,  // 2 min max in YELLOW
  meltSafeDistance: 15,     // 15m from molten metal
  craneClearRadius: 10,     // 10m under crane
  robotSpeed: 2.0,
  humanSafeDistance: 3,
};

/** Create a standard steel mill layout */
export function createMillLayout(): MillState {
  return {
    currentPhase: 'IDLE',
    phaseTimeS: 0,
    cycleCount: 0,
    cranePos: new Float64Array([50, 25]),
    craneMoving: false,
    craneLoadTons: 0,
    zones: [
      { id: 'furnace', type: 'GREEN', bounds: [40, 10, 60, 30], temperature: 35,
        craneAbove: false, moltenMetal: false, maxExposureS: Infinity },
      { id: 'casting', type: 'GREEN', bounds: [70, 10, 90, 30], temperature: 30,
        craneAbove: false, moltenMetal: false, maxExposureS: Infinity },
      { id: 'storage', type: 'GREEN', bounds: [0, 0, 30, 40], temperature: 25,
        craneAbove: false, moltenMetal: false, maxExposureS: Infinity },
      { id: 'crane-path', type: 'GREEN', bounds: [30, 15, 90, 25], temperature: 30,
        craneAbove: false, moltenMetal: false, maxExposureS: Infinity },
      { id: 'exit', type: 'GREEN', bounds: [0, 15, 10, 25], temperature: 22,
        craneAbove: false, moltenMetal: false, maxExposureS: Infinity },
    ],
  };
}

/** Advance the production cycle by dt seconds */
export function stepProduction(mill: MillState, dt: number): void {
  mill.phaseTimeS += dt;

  const furnace = mill.zones.find(z => z.id === 'furnace')!;
  const casting = mill.zones.find(z => z.id === 'casting')!;
  const cranePath = mill.zones.find(z => z.id === 'crane-path')!;

  switch (mill.currentPhase) {
    case 'IDLE':
      furnace.type = 'GREEN';
      casting.type = 'GREEN';
      cranePath.type = 'GREEN';
      furnace.temperature = 35;
      furnace.moltenMetal = false;
      furnace.craneAbove = false;
      casting.temperature = 30;
      casting.moltenMetal = false;
      casting.craneAbove = false;
      cranePath.craneAbove = false;
      cranePath.temperature = 30;
      mill.craneMoving = false;
      mill.craneLoadTons = 0;
      mill.cranePos[0] = 50; mill.cranePos[1] = 25; // Reset crane
      break;

    case 'CHARGE':
      furnace.type = 'YELLOW';
      furnace.temperature = 45 + mill.phaseTimeS * 2; // Heating up
      break;

    case 'MELT':
      furnace.type = 'RED';
      furnace.temperature = Math.min(1500, 200 + mill.phaseTimeS * 5);
      furnace.moltenMetal = true;
      break;

    case 'TAP':
      furnace.type = 'RED';
      furnace.temperature = 1500;
      furnace.moltenMetal = true;
      furnace.craneAbove = true;
      mill.craneMoving = true;
      mill.craneLoadTons = 150;
      // Crane path becomes RED
      cranePath.type = 'CRANE_PATH';
      cranePath.craneAbove = true;
      break;

    case 'CAST':
      furnace.type = 'YELLOW';
      furnace.temperature = 200;
      furnace.moltenMetal = false;
      furnace.craneAbove = false;
      casting.type = 'RED';
      casting.temperature = 1200;
      casting.moltenMetal = true;
      mill.craneMoving = true;
      // Move crane toward casting
      mill.cranePos[0] = Math.min(80, mill.cranePos[0]! + dt * 2);
      break;
  }
}

/** Transition to next phase */
export function advancePhase(mill: MillState): void {
  const phases: ProductionPhase[] = ['IDLE', 'CHARGE', 'MELT', 'TAP', 'CAST'];
  const idx = phases.indexOf(mill.currentPhase);
  mill.currentPhase = phases[(idx + 1) % phases.length]!;
  mill.phaseTimeS = 0;
  if (mill.currentPhase === 'IDLE') mill.cycleCount++;
}

export function createSteelRobot(id: string, x: number, y: number): SteelRobot {
  return {
    id, pos: new Float64Array([x, y]), role: 'IDLE', currentZone: 'storage',
    yellowExposureS: 0, internalTemp: 30, alive: true, humansNearby: 0,
    carryingLoad: false,
  };
}

/** Check which zone a position is in */
export function getZoneAt(pos: Float64Array, zones: Zone[]): Zone | null {
  for (const z of zones) {
    if (pos[0]! >= z.bounds[0] && pos[0]! <= z.bounds[2] &&
        pos[1]! >= z.bounds[1] && pos[1]! <= z.bounds[3]) {
      return z;
    }
  }
  return null;
}

/** Distance from a point to the nearest edge of a zone */
export function distanceToZone(pos: Float64Array, zone: Zone): number {
  const cx = Math.max(zone.bounds[0], Math.min(zone.bounds[2], pos[0]!));
  const cy = Math.max(zone.bounds[1], Math.min(zone.bounds[3], pos[1]!));
  return Math.sqrt((pos[0]! - cx) ** 2 + (pos[1]! - cy) ** 2);
}
