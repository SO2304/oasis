/**
 * OASIS Kernel — Lazy Inference Engine (OPT3)
 *
 * PROBLEM: The LatentEngine recomputes ALL agents every tick.
 * With 100 agents at 1kHz, that's 100,000 field samples/sec +
 * cross-correlation computations. Most of this is wasted — agents
 * in stable states don't need recomputation every tick.
 *
 * SOLUTION: Entropy-gated lazy evaluation.
 *
 * Each agent has an "urgency" score based on:
 * 1. Its entropy (high entropy = needs attention NOW)
 * 2. How many ticks since last recomputation
 * 3. Whether it received new tension since last tick
 * 4. Whether it's physical (physical agents always recompute — R11)
 *
 * Only agents above the urgency threshold are recomputed each tick.
 * The rest coast on their last computed state.
 *
 * INNOVATION: Adaptive tick rate per agent.
 * A HARD_RT physical agent recomputes every tick (1kHz).
 * A stable virtual agent recomputes every 10th tick (100Hz).
 * A dormant agent recomputes every 100th tick (10Hz).
 * The system automatically allocates compute where it matters.
 */

import type { AgentId } from '../types.js';
import { SchedulePriority } from '../types.js';

// ─── Agent Urgency ──────────────────────────────────────────────

export interface AgentUrgency {
  readonly agentId: AgentId;
  /** Current entropy of the agent's HyperState */
  readonly entropy: number;
  /** Ticks since last full recomputation */
  readonly ticksSinceUpdate: number;
  /** Whether new tension was received since last tick */
  readonly hasNewInput: boolean;
  /** Scheduling priority */
  readonly priority: SchedulePriority;
  /** Computed urgency score [0, 1] — higher = needs recompute */
  readonly urgencyScore: number;
  /** Should this agent be recomputed this tick? */
  readonly shouldRecompute: boolean;
}

// ─── Lazy Scheduler ─────────────────────────────────────────────

export class LazyScheduler {
  private readonly lastUpdate = new Map<AgentId, number>();
  private readonly inputFlags = new Map<AgentId, boolean>();
  private readonly priorities = new Map<AgentId, SchedulePriority>();
  private tickCount = 0;

  /** Entropy threshold: below this, agent is "stable" */
  private readonly stabilityThreshold: number;
  /** Maximum ticks before forced recomputation */
  private readonly maxStaleTicks: number;

  constructor(stabilityThreshold = 0.3, maxStaleTicks = 100) {
    this.stabilityThreshold = stabilityThreshold;
    this.maxStaleTicks = maxStaleTicks;
  }

  /** Register an agent */
  register(agentId: AgentId, priority: SchedulePriority): void {
    this.lastUpdate.set(agentId, 0);
    this.inputFlags.set(agentId, true); // First tick always computes
    this.priorities.set(agentId, priority);
  }

  /** Mark that an agent received new input (tension, perception) */
  markInput(agentId: AgentId): void {
    this.inputFlags.set(agentId, true);
  }

  /** Mark that an agent was recomputed */
  markUpdated(agentId: AgentId): void {
    this.lastUpdate.set(agentId, this.tickCount);
    this.inputFlags.set(agentId, false);
  }

  /**
   * Compute urgency for all agents and decide who gets recomputed.
   *
   * Returns agents sorted by urgency (highest first).
   */
  computeUrgencies(
    entropies: Map<AgentId, number>,
  ): AgentUrgency[] {
    this.tickCount++;
    const urgencies: AgentUrgency[] = [];

    for (const [agentId, priority] of this.priorities) {
      const entropy = entropies.get(agentId) ?? 0;
      const lastTick = this.lastUpdate.get(agentId) ?? 0;
      const ticksSinceUpdate = this.tickCount - lastTick;
      const hasNewInput = this.inputFlags.get(agentId) ?? false;

      // Compute urgency score
      let urgencyScore = 0;

      // Factor 1: HARD_RT always gets max urgency
      if (priority === SchedulePriority.HARD_RT) {
        urgencyScore = 1.0;
      } else {
        // Factor 2: Entropy — uncertain agents need attention
        urgencyScore += entropy * 0.4;

        // Factor 3: Staleness — even stable agents need periodic updates
        urgencyScore += Math.min(1, ticksSinceUpdate / this.maxStaleTicks) * 0.3;

        // Factor 4: New input — always process if something changed
        if (hasNewInput) urgencyScore += 0.3;

        // Factor 5: Priority boost for SOFT_RT
        if (priority === SchedulePriority.SOFT_RT) urgencyScore *= 1.2;
      }

      urgencyScore = Math.min(1, urgencyScore);

      // Decision: should we recompute?
      const shouldRecompute =
        priority === SchedulePriority.HARD_RT || // Always
        hasNewInput ||                            // New data
        entropy > this.stabilityThreshold ||      // Unstable
        ticksSinceUpdate >= this.maxStaleTicks;    // Too stale

      urgencies.push({
        agentId,
        entropy,
        ticksSinceUpdate,
        hasNewInput,
        priority,
        urgencyScore,
        shouldRecompute,
      });
    }

    // Sort by urgency (highest first)
    urgencies.sort((a, b) => b.urgencyScore - a.urgencyScore);

    return urgencies;
  }

  /** How many agents are registered */
  getAgentCount(): number {
    return this.priorities.size;
  }

  /** Current tick */
  getTickCount(): number {
    return this.tickCount;
  }
}
