/**
 * OASIS Kernel — Latent Engine (H3)
 *
 * The "brain" of the Tensorial Kernel.
 *
 * INNOVATION: The Latent Engine doesn't compute explicit rules.
 * It discovers NON-INTUITIVE CORRELATIONS between agents by
 * observing their trajectories in latent space.
 *
 * Example: A vibrating motor agent and an optical stabilization agent
 * have no logical connection — but in latent space, their state vectors
 * show anti-correlated oscillation. The engine detects this and creates
 * a coupling tension that stabilizes the camera when the motor vibrates.
 *
 * This is the key insight: in high-dimensional space, patterns that
 * are invisible in 3D become obvious geometric relationships.
 *
 * The engine operates in 3 phases per tick:
 * 1. SENSE: Sample the tension field for each agent
 * 2. CORRELATE: Discover latent relationships between agent trajectories
 * 3. ACTUATE: Apply forces to agents (with R14 entropy gating)
 */

import type { AgentId, TenantId, DriverId } from '../types.js';
import { monotonicNow, nsToMs, agentId, tenantId } from '../types.js';
import { type KillSwitch, getKillSwitch } from '../kill-switch.js';
import {
  type Vec,
  zeros,
  vec,
  add,
  sub,
  scale,
  norm,
  normalize,
  dot,
  cosineSimilarity,
  randomUnit,
  lerp,
  hadamard,
  reject,
} from './vector-math.js';
import {
  type HyperState,
  createHyperState,
  evolveState,
  isActionSafe,
  calculateEntropy,
  R14_ENTROPY_CRITICAL,
} from './hyper-state.js';
import { type TensionVector, type InterferenceResult, TensionField } from './tension-field.js';
import { discoverCorrelations as discoverCorrelationsFn, type LatentCorrelation } from './correlation.js';
import { AgentState } from '../types.js';

// Re-export for consumers
export type { LatentCorrelation } from './correlation.js';

export interface EngineTickResult {
  /** Forces applied to each agent */
  readonly forcesApplied: Map<AgentId, Vec>;
  /** Agents blocked by R14 (entropy too high) */
  readonly r14Blocked: AgentId[];
  /** New correlations discovered */
  readonly newCorrelations: LatentCorrelation[];
  /** Tension field coherence (global signal-to-noise) */
  readonly fieldCoherence: number;
  /** Tick duration in ms */
  readonly durationMs: number;
}

// ─── Trajectory Buffer ──────────────────────────────────────────

interface TrajectoryPoint {
  readonly position: Vec;
  readonly tick: number;
}

// ─── Latent Engine ──────────────────────────────────────────────

export class LatentEngine {
  private readonly agents = new Map<AgentId, HyperState>();
  private readonly trajectories = new Map<AgentId, TrajectoryPoint[]>();
  private readonly field: TensionField;
  private readonly correlations: LatentCorrelation[] = [];
  private readonly dim: number;
  private readonly tenant: TenantId;
  private tickCount = 0;

  /** How many trajectory points to keep per agent */
  private readonly trajectoryWindow: number;
  /** Minimum correlation strength to consider significant */
  private readonly correlationThreshold: number;

  constructor(
    tenant: TenantId,
    dim = 128,
    private readonly killSwitch: KillSwitch = getKillSwitch(),
    trajectoryWindow = 50,
    correlationThreshold = 0.6,
  ) {
    this.dim = dim;
    this.tenant = tenant;
    this.field = new TensionField(10_000, dim);
    this.trajectoryWindow = trajectoryWindow;
    this.correlationThreshold = correlationThreshold;
  }

  // ─── Agent Management ───────────────────────────────────────

  /** Register an agent in the latent space */
  registerAgent(id: AgentId, initialState: AgentState = AgentState.CREATED): HyperState {
    const state = createHyperState(id, initialState, this.dim);
    this.agents.set(id, state);
    this.trajectories.set(id, []);
    return state;
  }

  /** Get an agent's current HyperState */
  getState(id: AgentId): HyperState | undefined {
    return this.agents.get(id);
  }

  /** Get all agent IDs */
  getAgentIds(): AgentId[] {
    return [...this.agents.keys()];
  }

  // ─── Tension Emission ───────────────────────────────────────

  /**
   * An agent emits a tension into the field.
   *
   * This is how agents "communicate" in the Tensorial Kernel.
   * Instead of sending a message, the agent creates a force
   * in the shared latent space.
   */
  emitTension(
    sourceId: AgentId,
    force: Vec,
    signature: Vec,
    intensity = 1.0,
    decayRate = 5,
    lifetime = 20,
  ): void {
    this.killSwitch.assertAlive();

    const tension: TensionVector = {
      source: sourceId,
      tenantId: this.tenant,
      force,
      signature,
      intensity,
      decayRate,
      emittedAt: monotonicNow(),
      ticksRemaining: lifetime,
    };

    this.field.emit(tension);
  }

  // ─── Engine Tick (SENSE → CORRELATE → ACTUATE) ─────────────

  /**
   * Execute one engine tick.
   * This is the heartbeat of the Tensorial Brain.
   */
  tick(): EngineTickResult {
    this.killSwitch.assertAlive();
    const start = monotonicNow();
    this.tickCount++;

    const forcesApplied = new Map<AgentId, Vec>();
    const r14Blocked: AgentId[] = [];

    // ── PHASE 1: SENSE ──────────────────────────────────────
    // Sample the tension field for each agent
    const fieldSamples = new Map<AgentId, InterferenceResult>();

    for (const [id, state] of this.agents) {
      // Each agent's "antenna" is its current position + momentum direction
      const signature = state.momentum.length > 0 && norm(state.momentum) > 1e-10
        ? normalize(state.momentum)
        : normalize(state.position);

      const sample = this.field.sample(state.position, signature, this.tenant);
      fieldSamples.set(id, sample);
    }

    // ── PHASE 2: CORRELATE ──────────────────────────────────
    // Record trajectories and discover correlations
    for (const [id, state] of this.agents) {
      const traj = this.trajectories.get(id)!;
      traj.push({ position: Float64Array.from(state.position), tick: this.tickCount });
      if (traj.length > this.trajectoryWindow) traj.shift();
    }

    // Correlation discovery is O(n^2 × trajectory_length) — gate it
    const newCorrelations = this.tickCount % 10 === 0
      ? this.discoverCorrelations()
      : [];

    // ── PHASE 3: ACTUATE ────────────────────────────────────
    // Apply forces from the tension field + correlations
    for (const [id, state] of this.agents) {
      const sample = fieldSamples.get(id)!;

      // Base force from tension field
      let totalForce = sample.netForce;

      // Add correlation-induced forces
      for (const corr of this.correlations) {
        if (corr.agentA !== id && corr.agentB !== id) continue;

        const otherId = corr.agentA === id ? corr.agentB : corr.agentA;
        const otherState = this.agents.get(otherId);
        if (!otherState) continue;

        // If correlated: gentle pull toward alignment
        // If anti-correlated: gentle push toward anti-alignment
        const direction = corr.strength > 0
          ? sub(otherState.position, state.position) // Pull toward
          : sub(state.position, otherState.position); // Push away

        const corrForce = scale(normalize(direction), Math.abs(corr.strength) * 0.1);
        totalForce = add(totalForce, corrForce);
      }

      // R14: Entropy gate — block physical actuation if uncertain
      const projectedState = evolveState(state, totalForce);
      if (!isActionSafe(projectedState)) {
        r14Blocked.push(id);
        // Still evolve but with dampened force (don't freeze completely)
        const dampenedForce = scale(totalForce, 0.1);
        this.agents.set(id, evolveState(state, dampenedForce, 0.1, 0.3));
        forcesApplied.set(id, dampenedForce);
        continue;
      }

      // Apply the full force
      this.agents.set(id, evolveState(state, totalForce));
      forcesApplied.set(id, totalForce);
    }

    // Advance the tension field
    this.field.tick();

    const durationMs = nsToMs(monotonicNow() - start);

    // Global field coherence
    let totalCoherence = 0;
    let coherenceCount = 0;
    for (const sample of fieldSamples.values()) {
      if (sample.contributorCount > 0) {
        totalCoherence += sample.coherence;
        coherenceCount++;
      }
    }

    return {
      forcesApplied,
      r14Blocked,
      newCorrelations,
      fieldCoherence: coherenceCount > 0 ? totalCoherence / coherenceCount : 1,
      durationMs,
    };
  }

  // ─── Correlation Discovery ────────────────────────────────

  /** Discover correlations (delegates to correlation.ts) */
  private discoverCorrelations(): LatentCorrelation[] {
    return discoverCorrelationsFn(this.trajectories, this.correlations, this.field, this.correlationThreshold);
  }

  // ─── Accessors ────────────────────────────────────────────

  getField(): TensionField {
    return this.field;
  }

  getCorrelations(): readonly LatentCorrelation[] {
    return this.correlations;
  }

  getTickCount(): number {
    return this.tickCount;
  }

  getDimension(): number {
    return this.dim;
  }
}
