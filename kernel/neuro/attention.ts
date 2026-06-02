/**
 * OASIS Kernel — Attention Mechanism
 *
 * A REAL brain doesn't process everything. It SPOTLIGHTS
 * what matters and suppresses the rest. This is not optimization —
 * it's a fundamental cognitive architecture.
 *
 * OASIS Attention works like biological attention:
 *
 * - EXOGENOUS (bottom-up): Sudden changes grab attention
 *   (a new obstacle appearing, a sensor spike, an agent dying)
 *
 * - ENDOGENOUS (top-down): Goals direct attention
 *   (looking for the exit, tracking a specific agent)
 *
 * - INHIBITION OF RETURN: Already-attended things are suppressed
 *   (don't keep looking at the same obstacle forever)
 *
 * The output is an ATTENTION MAP: a weight [0,1] for each agent
 * that determines how much computational resources it gets.
 * This directly feeds the LazyScheduler — high attention agents
 * ALWAYS get recomputed, zero attention agents are skipped.
 *
 * KEY INSIGHT: Attention is multiplicative with emotion.
 * A fearful agent attends MORE to threats.
 * A curious agent attends MORE to novel stimuli.
 * Attention × Emotion = behavioral priority.
 */

import type { AgentId } from '../types.js';
import {
  type Vec,
  norm,
  sub,
  distance,
  cosineSimilarity,
} from '../physics/vector-math.js';
import type { HyperState } from '../physics/hyper-state.js';

// ─── Attention Weight ───────────────────────────────────────────

export interface AttentionWeight {
  readonly agentId: AgentId;
  /** Total attention weight [0, 1] */
  readonly weight: number;
  /** Breakdown of attention sources */
  readonly sources: {
    novelty: number;      // Exogenous: how much has this agent changed?
    salience: number;     // Exogenous: how dangerous/important is it?
    relevance: number;    // Endogenous: how goal-related is it?
    inhibition: number;   // Inhibition of return: attention fatigue
  };
}

// ─── Attention Mechanism ────────────────────────────────────────

export class AttentionMechanism {
  /** Previous states for change detection */
  private readonly previousStates = new Map<AgentId, Vec>();
  /** How many consecutive ticks each agent has been attended */
  private readonly attendedDuration = new Map<AgentId, number>();
  /** Goal positions for relevance computation */
  private readonly goals: Vec[] = [];
  private readonly dim: number;
  private tickCount = 0;

  /** Inhibition decay per tick (how fast attention fatigue fades) */
  private readonly inhibitionDecay: number;
  /** Max consecutive ticks before inhibition kicks in */
  private readonly inhibitionOnset: number;
  /** Minimum attention weight (no agent is ever fully ignored) */
  private readonly floorWeight: number;

  constructor(
    dim: number,
    inhibitionDecay = 0.1,
    inhibitionOnset = 10,
    floorWeight = 0.05,
  ) {
    this.dim = dim;
    this.inhibitionDecay = inhibitionDecay;
    this.inhibitionOnset = inhibitionOnset;
    this.floorWeight = floorWeight;
  }

  /** Set goal positions for endogenous attention */
  setGoals(goals: Vec[]): void {
    this.goals.length = 0;
    this.goals.push(...goals);
  }

  /**
   * Compute attention weights for all agents.
   *
   * Returns a map of agent → attention weight.
   * High weight = should be processed this tick.
   * Low weight = can be skipped.
   */
  compute(states: Map<AgentId, HyperState>): AttentionWeight[] {
    this.tickCount++;
    const weights: AttentionWeight[] = [];

    for (const [id, state] of states) {
      // ── NOVELTY (exogenous) ─────────────────────────
      // How much has this agent's state changed since last tick?
      const prevPos = this.previousStates.get(id);
      let novelty = 0;

      if (prevPos) {
        const posChange = distance(state.position, prevPos);
        // Also consider entropy change as novel
        novelty = Math.min(1, posChange * 5);
      } else {
        novelty = 1; // New agent = maximally novel
      }

      // ── SALIENCE (exogenous) ────────────────────────
      // High entropy = salient (uncertain = pay attention)
      // High momentum = salient (fast-moving = pay attention)
      // High pain (momentum + high entropy) = very salient
      const entropySalience = state.entropy;
      const speedSalience = Math.min(1, norm(state.momentum) * 2);
      const salience = Math.max(entropySalience, speedSalience);

      // ── RELEVANCE (endogenous) ──────────────────────
      // How close is this agent to any goal?
      let relevance = 0.2; // Base relevance
      for (const goal of this.goals) {
        const dist = distance(state.position, goal);
        const goalRelevance = 1 / (1 + dist);
        relevance = Math.max(relevance, goalRelevance);
      }

      // Also: if momentum points toward a goal, extra relevance
      if (norm(state.momentum) > 0.01) {
        for (const goal of this.goals) {
          const toGoal = sub(goal, state.position);
          if (norm(toGoal) > 0.01) {
            const alignment = cosineSimilarity(state.momentum, toGoal);
            if (alignment > 0.5) {
              relevance = Math.min(1, relevance + alignment * 0.3);
            }
          }
        }
      }

      // ── INHIBITION OF RETURN ────────────────────────
      // If we've been attending to this agent for too long,
      // suppress it (unless it's truly important)
      const attended = this.attendedDuration.get(id) ?? 0;
      let inhibition = 0;
      if (attended > this.inhibitionOnset) {
        inhibition = Math.min(0.5,
          (attended - this.inhibitionOnset) * this.inhibitionDecay * 0.05);
      }

      // ── COMBINED WEIGHT ─────────────────────────────
      const rawWeight =
        novelty * 0.30 +
        salience * 0.30 +
        relevance * 0.25 +
        (1 - inhibition) * 0.15;

      const weight = Math.max(this.floorWeight, Math.min(1, rawWeight));

      weights.push({
        agentId: id,
        weight,
        sources: { novelty, salience, relevance, inhibition },
      });

      // Update tracking
      this.previousStates.set(id, Float64Array.from(state.position));

      if (weight > 0.5) {
        this.attendedDuration.set(id, attended + 1);
      } else {
        // Not attended → decay inhibition
        this.attendedDuration.set(id, Math.max(0, attended - 1));
      }
    }

    // Sort by weight descending
    weights.sort((a, b) => b.weight - a.weight);

    return weights;
  }

  /** Get the top-K most attended agents */
  getTopK(weights: AttentionWeight[], k: number): AgentId[] {
    return weights.slice(0, k).map(w => w.agentId);
  }

  /** Get total attention "budget" being spent */
  getTotalAttention(weights: AttentionWeight[]): number {
    return weights.reduce((sum, w) => sum + w.weight, 0);
  }
}
