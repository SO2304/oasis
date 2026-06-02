/**
 * OASIS Kernel — Reflection Engine (PR2)
 *
 * Implements PREDICTIVE PROCESSING (Karl Friston's Free Energy Principle).
 *
 * CORE INNOVATION: The Efference Copy
 *
 * When you move your arm, your brain doesn't wait for the arm to arrive
 * and then check. It creates an EFFERENCE COPY — a prediction of what
 * the proprioceptive feedback SHOULD be, given the motor command sent.
 * Then it compares prediction vs reality. The delta is the
 * PREDICTION ERROR — and it's this error that drives everything:
 *
 * - Small error → normal operation, micro-corrections
 * - Growing error → something is resisting, increase effort
 * - Sustained error → obstacle or jam, change strategy
 * - Catastrophic error → R16 dysmorphia, entropy spike, freeze
 *
 * This is fundamentally different from "compare goal vs reality":
 * - Goal comparison: "I wanted to be at X, I'm at Y" (slow, coarse)
 * - Prediction error: "I pushed with F, I expected to move by dX,
 *   but I moved by 0" (fast, precise, catches problems within 1 tick)
 *
 * The Reflection Engine maintains:
 * 1. Efference Copy: predicted proprioception per driver
 * 2. Deviation Vector: prediction error in latent space
 * 3. Pain Signal: accumulated prediction error over time
 * 4. Adaptation: model correction when predictions are wrong
 */

import type { DriverId, AgentId, TenantId } from '../types.js';
import { monotonicNow, nsToMs } from '../types.js';
import { type KillSwitch, getKillSwitch } from '../kill-switch.js';
import {
  type Vec,
  zeros,
  add,
  sub,
  scale,
  norm,
  normalize,
  dot,
  cosineSimilarity,
  distance,
} from './vector-math.js';
import type { TensionField } from './tension-field.js';
import type { ProprioceptiveState } from '../perception/proprioception.js';
import {
  type EfferenceCopy,
  type DeviationVector,
  type PainSignal,
  type ReflectionTickResult,
  DeviationSeverity,
  classifySeverity,
  diagnoseCause,
} from './deviation.js';

// Re-export for consumers
export { DeviationSeverity } from './deviation.js';
export type { EfferenceCopy, DeviationVector, PainSignal, ReflectionTickResult } from './deviation.js';

// (types moved to deviation.ts — no inline definitions needed)

// ─── Reflection Engine ──────────────────────────────────────────

export class ReflectionEngine {
  private readonly efferenceCopies = new Map<DriverId, EfferenceCopy>();
  private readonly deviationHistory = new Map<DriverId, number[]>(); // magnitude history
  private readonly persistenceCounters = new Map<DriverId, number>();
  private readonly dim: number;

  /** Learned model: how much position changes per unit force */
  private readonly responsivityModel = new Map<DriverId, number>();

  /** Threshold for each severity level */
  private readonly thresholds: {
    resistance: number;
    anomaly: number;
    dysmorphia: number;
  };

  /** Pain decay rate per tick (pain fades if deviation stops) */
  private readonly painDecay: number;
  private readonly painAccumulator = new Map<DriverId, number>();

  constructor(
    dim: number,
    private readonly field: TensionField,
    private readonly tenantId: TenantId,
    private readonly agentId: AgentId,
    private readonly killSwitch: KillSwitch = getKillSwitch(),
    thresholds?: Partial<ReflectionEngine['thresholds']>,
    painDecay = 0.1,
  ) {
    this.dim = dim;
    this.thresholds = {
      resistance: thresholds?.resistance ?? 0.15,
      anomaly: thresholds?.anomaly ?? 0.4,
      dysmorphia: thresholds?.dysmorphia ?? 0.75,
    };
    this.painDecay = painDecay;
  }

  /**
   * Create an Efference Copy — predict what proprioception SHOULD
   * report after a motor command.
   *
   * Call this WHEN a command is sent, BEFORE the actuator responds.
   *
   * @param driverId Which driver the command was sent to
   * @param currentPosition Current proprioceptive position imprint
   * @param currentEffort Current proprioceptive effort imprint
   * @param commandForce Force that was commanded
   * @param mass Known mass of the actuator
   */
  predict(
    driverId: DriverId,
    currentPosition: Vec,
    currentEffort: Vec,
    commandForce: number,
    mass: number,
  ): EfferenceCopy {
    // Learn responsivity: how much does this driver actually move per unit force?
    const responsivity = this.responsivityModel.get(driverId) ?? (1 / mass);

    // Predicted position: current + expected displacement
    // F = ma → a = F/m → dx = a * dt ≈ F * responsivity * dt
    const dt = 0.016; // Match actuator simulation step
    const expectedDisplacement = commandForce * responsivity * dt;

    const predictedPosition = Float64Array.from(currentPosition);
    // Apply displacement to first position dimension that's non-zero, or dim 0
    let positionDim = 0;
    for (let i = 0; i < currentPosition.length; i++) {
      if (Math.abs(currentPosition[i]!) > 1e-10) {
        positionDim = i;
        break;
      }
    }
    predictedPosition[positionDim]! += expectedDisplacement;

    // Predicted effort: proportional to commanded force
    // Normal operation: force produces proportional effort response
    const predictedEffort = Float64Array.from(currentEffort);
    for (let i = 0; i < currentEffort.length; i++) {
      if (Math.abs(currentEffort[i]!) > 1e-10 || i === 0) {
        predictedEffort[i] = commandForce * 0.02; // Normal current draw
        break;
      }
    }

    const efference: EfferenceCopy = {
      driverId,
      predictedPosition,
      predictedEffort,
      commandForce,
      timestamp: monotonicNow(),
      expectedLatencyTicks: 1,
    };

    this.efferenceCopies.set(driverId, efference);
    return efference;
  }

  /**
   * REFLECT: Compare the efference copy (prediction) with actual
   * proprioceptive feedback.
   *
   * Call this AFTER receiving proprioceptive state from a driver.
   *
   * This is the core of predictive processing:
   * prediction_error = |predicted - actual|
   */
  reflect(proprioState: ProprioceptiveState): DeviationVector | null {
    const efference = this.efferenceCopies.get(proprioState.driverId);
    if (!efference) return null; // No prediction to compare against

    // Position prediction error
    const posError = distance(efference.predictedPosition, proprioState.positionImprint);

    // Effort prediction error — key signal for jam detection
    // If we commanded force but effort is unexpectedly HIGH (high current, high stress)
    // that means something is blocking the motor
    const effortError = distance(efference.predictedEffort, proprioState.effortImprint);

    // Combined deviation magnitude
    const magnitude = Math.sqrt(posError * posError + effortError * effortError);

    // Deviation direction in latent space
    const posDeviation = sub(proprioState.positionImprint, efference.predictedPosition);
    const effortDeviation = sub(proprioState.effortImprint, efference.predictedEffort);
    const combinedDeviation = add(posDeviation, effortDeviation);
    const direction = norm(combinedDeviation) > 0 ? normalize(combinedDeviation) : zeros(this.dim);

    // Update persistence counter
    const prevCount = this.persistenceCounters.get(proprioState.driverId) ?? 0;
    const persistence = magnitude > this.thresholds.resistance ? prevCount + 1 : 0;
    this.persistenceCounters.set(proprioState.driverId, persistence);

    // Track deviation history for adaptation
    const history = this.deviationHistory.get(proprioState.driverId) ?? [];
    history.push(magnitude);
    if (history.length > 50) history.shift();
    this.deviationHistory.set(proprioState.driverId, history);

    // Classify severity (delegates to deviation.ts)
    const severity = classifySeverity(magnitude, persistence, this.thresholds);

    return {
      driverId: proprioState.driverId,
      direction,
      magnitude,
      positionError: posError,
      effortError: effortError,
      persistenceTicks: persistence,
      severity,
    };
  }

  /**
   * Execute one reflection tick.
   *
   * Processes all deviations, generates pain signals, injects
   * tension into the field, and adapts the internal model.
   */
  tick(deviations: DeviationVector[]): ReflectionTickResult {
    const painSignals: PainSignal[] = [];
    let r16Triggered = false;
    let tensionsInjected = 0;
    let adaptations = 0;

    for (const dev of deviations) {
      // Update pain accumulator
      const currentPain = this.painAccumulator.get(dev.driverId) ?? 0;
      const newPain = Math.min(1, currentPain + dev.magnitude * 0.2 - this.painDecay);
      this.painAccumulator.set(dev.driverId, Math.max(0, newPain));

      // Generate pain signal if above threshold
      if (newPain > 0.1 || dev.severity !== DeviationSeverity.NOMINAL) {
        const cause = diagnoseCause(dev);
        const r16 = dev.severity === DeviationSeverity.DYSMORPHIA;

        painSignals.push({
          driverId: dev.driverId,
          intensity: newPain,
          direction: dev.direction,
          cause,
          r16Trigger: r16,
        });

        if (r16) {
          r16Triggered = true;
        }
      }

      // Inject pain as tension into the field
      // Pain creates REPULSIVE force away from the failing action
      if (dev.magnitude > this.thresholds.resistance) {
        this.field.emit({
          source: this.agentId,
          tenantId: this.tenantId,
          // Pain pushes AWAY from current trajectory
          force: scale(dev.direction, -dev.magnitude * 3),
          signature: dev.direction,
          intensity: dev.magnitude * 5,
          decayRate: 2,
          emittedAt: monotonicNow(),
          ticksRemaining: 8,
        });
        tensionsInjected++;
      }

      // R16: Dysmorphia → massive entropy injection
      if (dev.severity === DeviationSeverity.DYSMORPHIA) {
        // Entropy bomb — forces R14 to freeze actuation
        const entropySignature = zeros(this.dim);
        for (let i = 0; i < Math.min(9, this.dim); i++) {
          entropySignature[i] = 1 / 3;
        }
        this.field.emit({
          source: this.agentId,
          tenantId: this.tenantId,
          force: zeros(this.dim),
          signature: normalize(entropySignature),
          intensity: 10, // Maximum entropy injection
          decayRate: 1,
          emittedAt: monotonicNow(),
          ticksRemaining: 20,
        });
        tensionsInjected++;
      }

      // Adapt internal model based on prediction errors
      if (dev.severity === DeviationSeverity.NOMINAL && dev.magnitude > 0) {
        // Small error → adjust responsivity model
        const currentResp = this.responsivityModel.get(dev.driverId) ?? 0.5;
        // If position error is positive (moved more than expected), increase responsivity
        // If negative (moved less), decrease
        const adjustment = dev.positionError > dev.effortError ? 0.01 : -0.01;
        this.responsivityModel.set(dev.driverId, Math.max(0.01, currentResp + adjustment));
        adaptations++;
      }
    }

    // Decay pain for drivers with no deviation this tick
    for (const [driverId, pain] of this.painAccumulator) {
      if (!deviations.some(d => d.driverId === driverId)) {
        this.painAccumulator.set(driverId, Math.max(0, pain - this.painDecay));
      }
    }

    return {
      deviations,
      painSignals,
      r16Triggered,
      tensionsInjected,
      adaptations,
    };
  }

  /** Get current pain level for a driver */
  getPainLevel(driverId: DriverId): number {
    return this.painAccumulator.get(driverId) ?? 0;
  }

  /** Get deviation history for a driver */
  getDeviationHistory(driverId: DriverId): readonly number[] {
    return this.deviationHistory.get(driverId) ?? [];
  }

  /** Get learned responsivity for a driver */
  getResponsivity(driverId: DriverId): number {
    return this.responsivityModel.get(driverId) ?? 0.5;
  }

}
