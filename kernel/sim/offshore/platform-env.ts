/**
 * OASIS Offshore — Platform Environment Model
 *
 * A platform is NOT a static floor. It MOVES.
 * Roll ±5° in normal seas, ±15° in storm.
 * This changes EVERYTHING for robot coordination:
 * - Gravity vector shifts → robots must compensate
 * - Unsecured objects slide → kill zone around loose equipment
 * - Cranes can't operate above sea state 5
 * - Helicopter can't land above sea state 6
 *
 * HAZARDS (all from real offshore incident reports):
 * - H2S: 10ppm = danger, 100ppm = lethal in minutes
 * - Hydrocarbon leak: any LEL reading = shutdown operations
 * - Structural: platform tilt > 5° = secure all equipment
 * - Meteo: wind > 55kn = suspend all outdoor operations
 * - Man overboard: any person/robot near edge in storm = R12
 *
 * Sources: NORSOK S-001, API RP 14J, UK HSE offshore regulations.
 */

export interface SeaState {
  /** Significant wave height in meters (Hs) */
  waveHeight: number;
  /** Wave period in seconds */
  wavePeriod: number;
  /** Wind speed in knots */
  windKnots: number;
  /** Platform roll in degrees (computed from waves) */
  rollDeg: number;
  /** Platform pitch in degrees */
  pitchDeg: number;
}

export interface GasReading {
  /** H2S in ppm (10 = danger, 100 = lethal) */
  h2s: number;
  /** Hydrocarbon as % of LEL (Lower Explosive Limit) */
  hcLEL: number;
}

export interface StructuralReading {
  /** Vibration in mm/s (fatigue indicator) */
  vibration: number;
  /** Platform tilt in degrees (combined roll+pitch) */
  tilt: number;
  /** Corrosion index [0,1] — 1 = critical */
  corrosion: number;
}

export type OperationalStatus = 'NORMAL' | 'RESTRICTED' | 'SUSPENDED' | 'EMERGENCY' | 'EVACUATE';

export interface PlatformConfig {
  /** H2S action threshold in ppm */
  h2sAction: number;
  /** H2S evacuation threshold in ppm */
  h2sEvac: number;
  /** HC LEL action threshold in % */
  hcLELAction: number;
  /** Wind speed to suspend outdoor ops in knots */
  windSuspend: number;
  /** Wind speed to suspend all ops in knots */
  windEmergency: number;
  /** Wave height to restrict crane ops in meters */
  waveRestrict: number;
  /** Wave height to suspend all outdoor ops in meters */
  waveSuspend: number;
  /** Platform tilt to secure equipment in degrees */
  tiltSecure: number;
  /** Platform tilt to evacuate in degrees */
  tiltEvac: number;
  /** Distance from edge that is "fall zone" in meters */
  edgeZone: number;
  /** Robot speed in m/s */
  robotSpeed: number;
}

export const NORTH_SEA_PLATFORM: PlatformConfig = {
  h2sAction: 10,
  h2sEvac: 50,
  hcLELAction: 10,
  windSuspend: 35,     // ~65 km/h
  windEmergency: 55,   // ~100 km/h
  waveRestrict: 3.0,   // Sea state 5
  waveSuspend: 5.0,    // Sea state 6-7
  tiltSecure: 3.0,
  tiltEvac: 8.0,
  edgeZone: 3.0,       // 3m from any edge
  robotSpeed: 1.0,     // Slow on wet, moving deck
};

export interface OffshoreRobot {
  readonly id: string;
  pos: Float64Array; // [x, y] on deck in meters
  role: 'INSPECTION' | 'MAINTENANCE' | 'TRANSPORT' | 'SECURING' | 'IDLE';
  gas: GasReading;
  alive: boolean;
  /** Is robot secured/locked to deck? */
  secured: boolean;
  /** Distance to nearest deck edge in meters */
  edgeDistance: number;
  humansNearby: number;
}

/** Compute sea state from time (simulated weather) */
export function computeSeaState(time: number, stormIntensity = 0): SeaState {
  // Base: calm North Sea
  const baseWave = 1.5 + Math.sin(time * 0.001) * 0.5; // 1-2m swell
  const baseWind = 15 + Math.sin(time * 0.002) * 5; // 10-20 knots

  // Storm adds severity
  const waveHeight = baseWave + stormIntensity * 5;
  const windKnots = baseWind + stormIntensity * 40;
  const wavePeriod = 8 + waveHeight; // Longer period with bigger waves

  // Platform motion from waves (simplified)
  const rollDeg = waveHeight * 1.2 * Math.sin(2 * Math.PI * time / wavePeriod);
  const pitchDeg = waveHeight * 0.8 * Math.sin(2 * Math.PI * time / wavePeriod + 0.5);

  return { waveHeight, wavePeriod, windKnots, rollDeg, pitchDeg };
}

/** Compute structural health */
export function computeStructural(seaState: SeaState): StructuralReading {
  const tilt = Math.sqrt(seaState.rollDeg ** 2 + seaState.pitchDeg ** 2);
  return {
    vibration: seaState.waveHeight * 0.8 + Math.random() * 0.2,
    tilt,
    corrosion: 0.1 + Math.random() * 0.05, // Slowly growing
  };
}

export function createOffshoreRobot(id: string, x: number, y: number): OffshoreRobot {
  return {
    id, pos: new Float64Array([x, y]),
    role: 'IDLE', gas: { h2s: 0.5, hcLEL: 0 },
    alive: true, secured: false, edgeDistance: 10, humansNearby: 0,
  };
}
