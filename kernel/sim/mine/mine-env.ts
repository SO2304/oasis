/**
 * OASIS Mine — Underground Environment Model
 *
 * A coal mine is a graph of TUNNELS, not an open space.
 * Robots move along tunnels. Sensors sample the environment.
 * The nervous system monitors gas, vibration, and connectivity.
 *
 * HAZARDS (all real, all measured):
 * - CH4 (methane): explosive at 5-15% concentration. LEL = 5%.
 *   Action threshold: 1% (20% of LEL). Evacuation: 2%.
 * - CO (carbon monoxide): lethal at 1200ppm, dangerous at 50ppm.
 *   Action threshold: 25ppm. Evacuation: 50ppm.
 * - Vibration: precursor to roof collapse. Normal < 2mm/s.
 *   Warning: > 5mm/s. Evacuation: > 10mm/s.
 * - Temperature: > 40°C is dangerous for equipment.
 *
 * Sources: MSHA (Mine Safety & Health Administration) standards.
 */

export interface TunnelSegment {
  readonly id: string;
  /** Start position [x, y] in meters */
  readonly start: Float64Array;
  /** End position [x, y] in meters */
  readonly end: Float64Array;
  /** Width in meters */
  readonly width: number;
  /** Connected segment IDs */
  readonly connections: string[];
}

export interface GasReading {
  /** Methane concentration in % (LEL = 5%) */
  ch4: number;
  /** Carbon monoxide in ppm */
  co: number;
  /** Temperature in °C */
  temperature: number;
  /** Vibration in mm/s (roof stability) */
  vibration: number;
  /** Oxygen level in % (normal: 20.9%) */
  o2: number;
}

export interface MineConfig {
  /** CH4 action threshold in % */
  ch4ActionThreshold: number;
  /** CH4 evacuation threshold in % */
  ch4EvacThreshold: number;
  /** CO action threshold in ppm */
  coActionThreshold: number;
  /** CO evacuation threshold in ppm */
  coEvacThreshold: number;
  /** Vibration warning in mm/s */
  vibrationWarning: number;
  /** Vibration evacuation in mm/s */
  vibrationEvac: number;
  /** Communication range in meters (reduced underground) */
  commRange: number;
  /** Robot speed in m/s */
  robotSpeed: number;
}

export const COAL_MINE: MineConfig = {
  ch4ActionThreshold: 1.0,   // 20% of LEL
  ch4EvacThreshold: 2.0,     // 40% of LEL → EVACUATE
  coActionThreshold: 25,     // MSHA 8-hour TWA
  coEvacThreshold: 50,       // MSHA STEL
  vibrationWarning: 5.0,     // mm/s
  vibrationEvac: 10.0,       // mm/s → roof may collapse
  commRange: 50,             // Severely limited underground
  robotSpeed: 1.5,           // Slow in tunnels
};

export interface MineRobot {
  readonly id: string;
  pos: Float64Array;
  currentTunnel: string;
  role: 'MINER' | 'SCOUT' | 'RELAY' | 'EVACUATING' | 'DISABLED';
  sensors: GasReading;
  battery: number;
  alive: boolean;
  /** Humans detected nearby */
  humansNearby: number;
}

export function createRobot(id: string, x: number, y: number, tunnel: string): MineRobot {
  return {
    id, pos: new Float64Array([x, y]), currentTunnel: tunnel,
    role: 'MINER', sensors: { ch4: 0.3, co: 5, temperature: 28, vibration: 1.0, o2: 20.8 },
    battery: 1.0, alive: true, humansNearby: 0,
  };
}

/** Simulate environmental gas reading at a position */
export function sampleEnvironment(
  x: number, y: number, time: number,
  gasSource?: { x: number; y: number; ch4: number; co: number },
  collapseZone?: { x: number; y: number; radius: number },
): GasReading {
  // Baseline
  let ch4 = 0.3 + Math.sin(time * 0.1) * 0.1; // Natural seepage
  let co = 5 + Math.sin(time * 0.2) * 2;
  let temp = 28 + Math.sin(time * 0.05) * 2;
  let vibration = 1.0 + Math.random() * 0.5;
  let o2 = 20.8;

  // Gas source proximity
  if (gasSource) {
    const dist = Math.sqrt((x - gasSource.x) ** 2 + (y - gasSource.y) ** 2);
    if (dist < 80) {
      const falloff = 1 / (1 + dist * 0.1);
      ch4 += gasSource.ch4 * falloff;
      co += gasSource.co * falloff;
      o2 -= ch4 * 0.5; // CH4 displaces O2
    }
  }

  // Collapse zone (vibration increases)
  if (collapseZone) {
    const dist = Math.sqrt((x - collapseZone.x) ** 2 + (y - collapseZone.y) ** 2);
    if (dist < collapseZone.radius) {
      vibration += (collapseZone.radius - dist) * 2;
    }
  }

  return { ch4, co, temperature: temp, vibration, o2 };
}

export function robotDistance(a: MineRobot, b: MineRobot): number {
  return Math.sqrt((a.pos[0]! - b.pos[0]!) ** 2 + (a.pos[1]! - b.pos[1]!) ** 2);
}
