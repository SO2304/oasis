/**
 * OASIS — Surgical Needle Controller
 *
 * Three-layer control inspired by the OASIS nervous system:
 *
 * LAYER 0 — REFLEX: Force > 5N → immediate stop (< 1ms)
 * LAYER 1 — TREMOR FILTER: Notch filter at 8-12Hz removes hand tremor
 * LAYER 2 — RESPIRATORY TRACKING: Predict breathing phase, compensate
 * LAYER 3 — INSERTION CONTROL: Constant velocity with force feedback
 *
 * INNOVATION: Respiratory phase prediction via efference copy.
 * Instead of reacting to respiratory motion (always late),
 * the controller PREDICTS the breathing cycle by fitting
 * a sinusoidal model to past observations. Then it leads
 * the motion by the control latency, achieving zero-lag tracking.
 */

import type { TissueConfig, NeedleState, RespiratoryState } from './tissue-model.js';

// ─── Tremor Filter (Notch at 8-12Hz) ───────────────────────────

export class TremorFilter {
  /** Second-order IIR notch filter state */
  private readonly stateX: Float64Array; // [z1, z2] per axis
  private readonly stateY: Float64Array;

  /** Notch filter coefficients (computed in constructor) */
  private b0: number;
  private b1: number;
  private b2: number;
  private a1: number;
  private a2: number;

  constructor(centerHz = 10, bandwidthHz = 3, sampleRateHz = 1000) {
    this.stateX = new Float64Array(6); // 2 states × 3 axes
    this.stateY = new Float64Array(6);

    // Notch filter design (biquad)
    const w0 = 2 * Math.PI * centerHz / sampleRateHz;
    const bw = 2 * Math.PI * bandwidthHz / sampleRateHz;
    const alpha = Math.sin(w0) * Math.sinh(Math.log(2) / 2 * bw * w0 / Math.sin(w0));

    this.b0 = 1;
    this.b1 = -2 * Math.cos(w0);
    this.b2 = 1;
    const a0 = 1 + alpha;
    this.a1 = -2 * Math.cos(w0) / a0;
    this.a2 = (1 - alpha) / a0;
    // Normalize b coefficients
    this.b0 /= a0;
    this.b1 /= a0;
    this.b2 /= a0;
  }

  /** Filter a 3D signal sample (in-place modification) */
  filter(input: Float64Array): Float64Array {
    const output = new Float64Array(3);

    for (let axis = 0; axis < 3; axis++) {
      const x = input[axis]!;
      const x1 = this.stateX[axis * 2]!;
      const x2 = this.stateX[axis * 2 + 1]!;
      const y1 = this.stateY[axis * 2]!;
      const y2 = this.stateY[axis * 2 + 1]!;

      // Direct Form I
      const y = this.b0 * x + this.b1 * x1 + this.b2 * x2
              - this.a1 * y1 - this.a2 * y2;

      // Shift state
      this.stateX[axis * 2 + 1] = x1;
      this.stateX[axis * 2] = x;
      this.stateY[axis * 2 + 1] = y1;
      this.stateY[axis * 2] = y;

      output[axis] = y;
    }

    return output;
  }

  reset(): void {
    this.stateX.fill(0);
    this.stateY.fill(0);
  }
}

// ─── Respiratory Phase Predictor ────────────────────────────────

export class RespiratoryPredictor {
  /** Circular buffer of recent Z positions for cycle detection */
  private readonly buffer: number[] = [];
  private readonly bufferSize: number;
  /** Estimated respiratory frequency in Hz */
  estimatedHz: number;
  /** Estimated amplitude in mm */
  estimatedAmplitude: number;
  /** Estimated phase in radians */
  estimatedPhase: number;
  private sampleCount = 0;

  constructor(bufferSize = 500) { // 0.5s at 1kHz
    this.bufferSize = bufferSize;
    this.estimatedHz = 0.25; // Prior: typical breathing
    this.estimatedAmplitude = 10;
    this.estimatedPhase = 0;
  }

  /** Low-pass filtered value for clean zero-crossing */
  private lpfValue = 0;

  /** Add a Z-axis observation and update the respiratory model */
  observe(zPosition: number, dt: number): void {
    // Low-pass filter the input first (reject tremor and noise)
    // Cutoff ~1Hz at 1kHz: alpha = 1 - exp(-2π×1/1000) ≈ 0.006
    this.lpfValue += (zPosition - this.lpfValue) * 0.006;

    this.buffer.push(this.lpfValue);
    if (this.buffer.length > this.bufferSize) this.buffer.shift();
    this.sampleCount++;

    if (this.buffer.length < 200) return; // Need ~0.2s of data

    // Estimate frequency by zero-crossing detection on FILTERED signal
    let crossings = 0;
    const mean = this.buffer.reduce((a, b) => a + b, 0) / this.buffer.length;
    for (let i = 1; i < this.buffer.length; i++) {
      if ((this.buffer[i]! - mean) * (this.buffer[i - 1]! - mean) < 0) {
        crossings++;
      }
    }
    if (crossings < 2) return; // Not enough cycles
    const periodSamples = this.buffer.length / (crossings / 2);
    this.estimatedHz = 1 / (periodSamples * dt);

    // Estimate amplitude
    let maxVal = -Infinity, minVal = Infinity;
    for (const v of this.buffer) {
      if (v > maxVal) maxVal = v;
      if (v < minVal) minVal = v;
    }
    this.estimatedAmplitude = (maxVal - minVal) / 2;

    // Estimate current phase from recent trend
    const recent = this.buffer.slice(-10);
    const trend = recent[recent.length - 1]! - recent[0]!;
    const pos = (this.buffer[this.buffer.length - 1]! - mean) / Math.max(0.1, this.estimatedAmplitude);
    this.estimatedPhase = Math.asin(Math.max(-1, Math.min(1, pos)));
    if (trend < 0) this.estimatedPhase = Math.PI - this.estimatedPhase;
  }

  /** Predict the respiratory offset at time t + lookahead */
  predict(lookaheadS: number): number {
    const futurePhase = this.estimatedPhase + 2 * Math.PI * this.estimatedHz * lookaheadS;
    return this.estimatedAmplitude * Math.sin(futurePhase);
  }

  reset(): void {
    this.buffer.length = 0;
    this.sampleCount = 0;
    this.estimatedHz = 0.25;
    this.estimatedAmplitude = 10;
    this.estimatedPhase = 0;
  }
}

// ─── Surgical Controller ────────────────────────────────────────

export interface SurgicalCommand {
  /** Commanded velocity [vx, vy, vz] in mm/s */
  velocity: Float64Array;
  /** Insertion phase */
  phase: 'APPROACH' | 'PUNCTURE' | 'INSERTING' | 'AT_TARGET' | 'STOPPED';
  /** Force safety check passed? */
  forceSafe: boolean;
}

export class SurgicalController {
  readonly tremorFilter: TremorFilter;
  readonly respiratoryPredictor: RespiratoryPredictor;

  /** Target insertion velocity in mm/s */
  private readonly insertionSpeed: number;
  /** Maximum force before emergency stop in N */
  private readonly maxForce: number;
  /** Control loop lookahead for respiratory compensation in seconds */
  private readonly lookahead: number;

  constructor(insertionSpeed = 2.0, maxForce = 4.0, lookahead = 0.05) {
    this.tremorFilter = new TremorFilter(10, 6, 1000);
    this.respiratoryPredictor = new RespiratoryPredictor();
    this.insertionSpeed = insertionSpeed;
    this.maxForce = maxForce;
    this.lookahead = lookahead;
  }

  /**
   * Compute the needle velocity command.
   *
   * @param needleState Current needle state
   * @param surgeonInput Raw surgeon hand position [x, y, z] in mm (includes tremor)
   * @param respiratoryOffset Current respiratory offset [x, y, z] in mm
   * @param targetInTissue Target position in tissue frame [x, y, z] in mm
   * @param dt Time step
   */
  compute(
    needleState: NeedleState,
    surgeonInput: Float64Array,
    respiratoryOffset: Float64Array,
    targetInTissue: Float64Array,
    dt: number,
  ): SurgicalCommand {
    // ── LAYER 0: FORCE SAFETY REFLEX ─────────────────
    const forceMag = Math.sqrt(
      needleState.force[0]! ** 2 + needleState.force[1]! ** 2 + needleState.force[2]! ** 2,
    );
    if (forceMag > this.maxForce || needleState.tissueTorn) {
      return {
        velocity: new Float64Array(3),
        phase: 'STOPPED',
        forceSafe: false,
      };
    }

    // ── LAYER 1: TREMOR FILTERING ────────────────────
    const filtered = this.tremorFilter.filter(surgeonInput);

    // ── LAYER 2: RESPIRATORY COMPENSATION ────────────
    // Observe current respiratory motion
    this.respiratoryPredictor.observe(respiratoryOffset[2]!, dt);

    // Predict where the target will be at t + lookahead
    const predictedRespZ = this.respiratoryPredictor.predict(this.lookahead);
    const currentRespZ = respiratoryOffset[2]!;
    const respCompensation = new Float64Array(3);
    respCompensation[0] = respiratoryOffset[0]!; // Track current XY
    respCompensation[1] = respiratoryOffset[1]!;
    respCompensation[2] = predictedRespZ; // PREDICT Z

    // ── LAYER 3: INSERTION CONTROL ───────────────────
    // Target in world frame = tissue target + respiratory offset
    const worldTarget = new Float64Array(3);
    worldTarget[0] = targetInTissue[0]! + respCompensation[0]!;
    worldTarget[1] = targetInTissue[1]! + respCompensation[1]!;
    worldTarget[2] = targetInTissue[2]! + respCompensation[2]!;

    // Error to target
    const errorX = worldTarget[0]! - needleState.tipPos[0]!;
    const errorY = worldTarget[1]! - needleState.tipPos[1]!;
    const errorZ = worldTarget[2]! - needleState.tipPos[2]!;
    const distToTarget = Math.sqrt(errorX ** 2 + errorY ** 2 + errorZ ** 2);

    // Determine phase
    let phase: SurgicalCommand['phase'];
    if (distToTarget < 0.5) {
      phase = 'AT_TARGET';
    } else if (needleState.depth < 0.1) {
      phase = 'APPROACH';
    } else if (!needleState.skinPunctured) {
      phase = 'PUNCTURE';
    } else {
      phase = 'INSERTING';
    }

    // Compute velocity command
    const velocity = new Float64Array(3);

    if (phase === 'AT_TARGET') {
      // Hold position — track respiratory motion only
      velocity[0] = respCompensation[0]! * 0.1; // Gentle tracking
      velocity[1] = respCompensation[1]! * 0.1;
      velocity[2] = 0; // Don't push deeper
    } else {
      // XY: track filtered surgeon input + respiratory compensation
      velocity[0] = filtered[0]! * 0.3 + errorX * 2.0;
      velocity[1] = filtered[1]! * 0.3 + errorY * 2.0;

      // Z: constant insertion speed, modulated by force feedback
      const forceRatio = forceMag / this.maxForce;
      const speedScale = 1 - forceRatio * 0.8; // Slow down near force limit
      velocity[2] = this.insertionSpeed * Math.max(0.1, speedScale);

      // During puncture, slow down for controlled entry
      if (phase === 'PUNCTURE') {
        velocity[2]! *= 0.5;
      }
    }

    return { velocity, phase, forceSafe: true };
  }

  reset(): void {
    this.tremorFilter.reset();
    this.respiratoryPredictor.reset();
  }
}
