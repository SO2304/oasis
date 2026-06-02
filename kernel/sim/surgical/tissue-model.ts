/**
 * OASIS — Soft Tissue Physics Model
 *
 * A needle entering tissue encounters:
 * 1. SKIN PUNCTURE: initial resistance peak then drop
 * 2. TISSUE DRAG: friction proportional to depth × velocity
 * 3. ORGAN DEFORMATION: tissue moves ahead of the needle tip
 * 4. RESPIRATORY MOTION: the entire field oscillates at 0.25Hz
 *
 * Parameters from published biomechanics literature:
 * - Skin puncture force: 0.5-2N (Okamura 2004)
 * - Tissue friction: 0.2-0.8 N/mm at 2mm/s (Simone & Okamura 2002)
 * - Tissue stiffness: 1-10 kPa (liver, Phipps 2005)
 * - Respiratory amplitude: 5-20mm cranio-caudal (Keall 2006)
 *
 * All units: mm, N, seconds.
 */

export interface TissueConfig {
  /** Skin puncture force in N */
  skinPunctureForce: number;
  /** Skin thickness in mm */
  skinThickness: number;
  /** Tissue friction coefficient (N per mm of depth per mm/s velocity) */
  frictionCoeff: number;
  /** Tissue stiffness in N/mm (resistance to deformation) */
  stiffness: number;
  /** Maximum deformation before tissue tears in mm */
  tearThreshold: number;
  /** Respiratory frequency in Hz */
  respirationHz: number;
  /** Respiratory amplitude in mm (peak-to-peak) */
  respirationAmplitude: number;
  /** Target position [x, y, z] in mm (relative to skin entry) */
  targetPos: Float64Array;
  /** Target radius in mm */
  targetRadius: number;
}

export const LIVER_BIOPSY: TissueConfig = {
  skinPunctureForce: 1.2,
  skinThickness: 2.0,
  frictionCoeff: 0.04,
  stiffness: 3.0,
  tearThreshold: 8.0,
  respirationHz: 0.25,
  respirationAmplitude: 12.0,
  targetPos: new Float64Array([0, 0, 45]), // 45mm deep
  targetRadius: 1.5, // 3mm diameter lesion
};

export interface NeedleState {
  /** Needle tip position [x, y, z] in mm */
  tipPos: Float64Array;
  /** Insertion depth in mm (along Z axis) */
  depth: number;
  /** Current force on needle [fx, fy, fz] in N */
  force: Float64Array;
  /** Current velocity [vx, vy, vz] in mm/s */
  velocity: Float64Array;
  /** Has skin been punctured? */
  skinPunctured: boolean;
  /** Tissue deformation ahead of needle tip in mm */
  tissueDeformation: number;
  /** Is tissue torn? (SAFETY VIOLATION) */
  tissueTorn: boolean;
  /** Time in seconds */
  time: number;
}

export interface RespiratoryState {
  /** Current respiratory phase offset in mm */
  offset: Float64Array;
  /** Current respiratory velocity in mm/s */
  velocity: Float64Array;
}

export function createNeedleState(): NeedleState {
  return {
    tipPos: new Float64Array(3),
    depth: 0,
    force: new Float64Array(3),
    velocity: new Float64Array(3),
    skinPunctured: false,
    tissueDeformation: 0,
    tissueTorn: false,
    time: 0,
  };
}

/**
 * Compute respiratory motion at current time.
 * The target and all tissue moves with breathing.
 */
export function respiratoryMotion(time: number, config: TissueConfig): RespiratoryState {
  const phase = 2 * Math.PI * config.respirationHz * time;
  // Breathing is primarily along Z (cranio-caudal) with small XY components
  const ampZ = config.respirationAmplitude / 2;
  const ampX = ampZ * 0.15; // 15% lateral
  const ampY = ampX;

  return {
    offset: new Float64Array([
      ampX * Math.sin(phase * 1.1), // Slightly different freq for realism
      ampY * Math.sin(phase * 0.9),
      ampZ * Math.sin(phase),
    ]),
    velocity: new Float64Array([
      ampX * Math.cos(phase * 1.1) * 2 * Math.PI * config.respirationHz * 1.1,
      ampY * Math.cos(phase * 0.9) * 2 * Math.PI * config.respirationHz * 0.9,
      ampZ * Math.cos(phase) * 2 * Math.PI * config.respirationHz,
    ]),
  };
}

/**
 * Step the needle-tissue interaction physics.
 *
 * @param commandVelocity Velocity commanded by the controller [vx, vy, vz] mm/s
 * @param dt Time step in seconds
 */
export function stepTissue(
  state: NeedleState,
  commandVelocity: Float64Array,
  dt: number,
  config: TissueConfig,
): void {
  state.time += dt;

  // Update velocity (first-order response, tau = 2ms for surgical robot)
  const tau = 0.002;
  for (let i = 0; i < 3; i++) {
    state.velocity[i]! += (commandVelocity[i]! - state.velocity[i]!) * (dt / tau);
  }

  // Update position
  for (let i = 0; i < 3; i++) {
    state.tipPos[i]! += state.velocity[i]! * dt;
  }

  // Depth is Z component (clamped to >= 0)
  state.depth = Math.max(0, state.tipPos[2]!);

  // ── Force model ──────────────────────────────────────
  state.force.fill(0);

  if (state.depth <= 0) return; // Not in tissue yet

  // 1. SKIN PUNCTURE
  if (!state.skinPunctured && state.depth < config.skinThickness) {
    // Force ramps up then drops at puncture
    const progress = state.depth / config.skinThickness;
    const punctureForce = config.skinPunctureForce * Math.sin(progress * Math.PI);
    state.force[2] = -punctureForce; // Resists insertion
  } else if (!state.skinPunctured && state.depth >= config.skinThickness) {
    state.skinPunctured = true;
  }

  // 2. TISSUE FRICTION (depth × velocity dependent)
  if (state.skinPunctured) {
    const speed = Math.abs(state.velocity[2]!);
    const friction = config.frictionCoeff * state.depth * speed;
    state.force[2]! -= friction * Math.sign(state.velocity[2]!);

    // Lateral friction
    for (let i = 0; i < 2; i++) {
      const latFriction = config.frictionCoeff * 0.5 * Math.abs(state.velocity[i]!);
      state.force[i]! -= latFriction * Math.sign(state.velocity[i]!);
    }
  }

  // 3. TISSUE DEFORMATION (viscoelastic — deforms under load, relaxes when released)
  if (state.skinPunctured) {
    // Deformation increases with insertion velocity, relaxes exponentially
    const deformRate = Math.max(0, state.velocity[2]!) * 0.1; // mm of deformation per mm/s
    const relaxRate = 0.5; // Relaxation time constant (1/s) — tissue springs back
    state.tissueDeformation += (deformRate - state.tissueDeformation * relaxRate) * dt;
    state.tissueDeformation = Math.max(0, state.tissueDeformation);

    if (state.tissueDeformation > config.tearThreshold) {
      state.tissueTorn = true;
    }
  }
}

/**
 * Compute the ACTUAL target position (accounting for respiration + deformation).
 */
export function getActualTargetPos(
  config: TissueConfig,
  respiratoryOffset: Float64Array,
  tissueDeformation: number,
): Float64Array {
  const pos = new Float64Array(3);
  pos[0] = config.targetPos[0]! + respiratoryOffset[0]!;
  pos[1] = config.targetPos[1]! + respiratoryOffset[1]!;
  pos[2] = config.targetPos[2]! + respiratoryOffset[2]! - tissueDeformation;
  return pos;
}
