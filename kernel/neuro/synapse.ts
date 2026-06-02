/**
 * OASIS Kernel — Neuroplasticity (Synaptic Network)
 *
 * "Neurons that fire together wire together" — Donald Hebb, 1949
 *
 * THE MISSING PIECE: Tensions are ephemeral. They appear, influence
 * agents, and evaporate. The system has NO LONG-TERM MEMORY of
 * which agents work well together.
 *
 * NEUROPLASTICITY fixes this. When two agents' tensions consistently
 * correlate, a SYNAPSE forms between them — a persistent, weighted
 * connection that doesn't require continuous tension emission.
 *
 * Synapses are NOT messages. They are STRUCTURAL MODIFICATIONS
 * to the tension field itself. Like biological synapses:
 *
 * - POTENTIATION: Used synapses get stronger (Hebbian learning)
 * - DEPRESSION: Unused synapses get weaker (synaptic pruning)
 * - SATURATION: Synapses have a max weight (prevents runaway)
 * - PLASTICITY WINDOWS: Synapses are more plastic when entropy
 *   is moderate (not too stable, not too chaotic)
 *
 * The result: over time, the tension field develops a TOPOLOGY
 * that reflects the system's learned relationships. The field
 * itself becomes the memory — not data stored somewhere,
 * but the SHAPE of the space agents navigate through.
 */

import type { AgentId } from '../types.js';
import { monotonicNow } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  scale,
  norm,
  normalize,
  cosineSimilarity,
} from '../physics/vector-math.js';
import type { HyperState } from '../physics/hyper-state.js';

// ─── Synapse ────────────────────────────────────────────────────

export interface Synapse {
  /** Pre-synaptic agent (the influencer) */
  readonly preId: AgentId;
  /** Post-synaptic agent (the influenced) */
  readonly postId: AgentId;
  /** Connection weight [-1, 1]: positive = excitatory, negative = inhibitory */
  weight: number;
  /** How many times this synapse has been activated */
  activationCount: number;
  /** Tick of last activation */
  lastActivatedAt: number;
  /** Tick when the synapse was formed */
  readonly formedAt: number;
  /** The axis in latent space along which influence flows */
  conductionAxis: Vec;
  /** STDP: tick when pre-synaptic agent last fired */
  preFireTick: number;
  /** STDP: tick when post-synaptic agent last fired */
  postFireTick: number;
  /** Eligibility trace — decaying memory of recent activity for credit assignment */
  eligibility: number;
}

// ─── Plasticity Event ───────────────────────────────────────────

export interface PlasticityEvent {
  readonly type: 'POTENTIATED' | 'DEPRESSED' | 'FORMED' | 'PRUNED';
  readonly preId: AgentId;
  readonly postId: AgentId;
  readonly weightBefore: number;
  readonly weightAfter: number;
  readonly tick: number;
}

// ─── Synaptic Network ───────────────────────────────────────────

export class SynapticNetwork {
  /** Synapses indexed by "preId:postId" */
  private readonly synapses = new Map<string, Synapse>();
  private readonly events: PlasticityEvent[] = [];
  private readonly dim: number;
  private tickCount = 0;

  /** How fast synapses strengthen with use */
  private readonly potentiationRate: number;
  /** How fast unused synapses weaken per tick */
  private readonly depressionRate: number;
  /** Minimum weight before a synapse is pruned */
  private readonly pruneThreshold: number;
  /** Maximum synapse weight */
  private readonly maxWeight: number;
  /** Minimum co-activation to form a new synapse */
  private readonly formationThreshold: number;

  constructor(
    dim: number,
    potentiationRate = 0.05,
    depressionRate = 0.002,
    pruneThreshold = 0.05,
    maxWeight = 1.0,
    formationThreshold = 0.5,
  ) {
    this.dim = dim;
    this.potentiationRate = potentiationRate;
    this.depressionRate = depressionRate;
    this.pruneThreshold = pruneThreshold;
    this.maxWeight = maxWeight;
    this.formationThreshold = formationThreshold;
  }

  /**
   * HEBBIAN + STDP UPDATE (Claim 7 — Patent-optimized)
   *
   * Two learning rules combined:
   *
   * 1. HEBBIAN: "Fire together, wire together"
   *    Co-active agents strengthen their synapse.
   *
   * 2. STDP (Spike-Timing-Dependent Plasticity):
   *    The ORDER of firing matters.
   *    Pre fires BEFORE post → LTP (Long-Term Potentiation)
   *    Post fires BEFORE pre → LTD (Long-Term Depression)
   *    This creates DIRECTIONAL synapses — A→B is different from B→A.
   *    The temporal window follows an exponential decay curve.
   *
   * 3. ELIGIBILITY TRACE: A decaying memory of recent co-activation.
   *    When a reward/pain signal arrives later, the trace determines
   *    WHICH synapse gets credit. This bridges the temporal gap
   *    between action and consequence (credit assignment problem).
   */
  update(states: Map<AgentId, HyperState>): PlasticityEvent[] {
    this.tickCount++;
    const newEvents: PlasticityEvent[] = [];
    const agents = [...states.entries()];

    // Phase 1: Check co-activation for all pairs
    for (let i = 0; i < agents.length; i++) {
      for (let j = i + 1; j < agents.length; j++) {
        const [idA, stateA] = agents[i]!;
        const [idB, stateB] = agents[j]!;

        const momA = norm(stateA.momentum);
        const momB = norm(stateB.momentum);

        // Both must be "firing" (have momentum above threshold)
        if (momA < 0.01 && momB < 0.01) continue;

        const key = this.synapseKey(idA, idB);
        const synapse = this.synapses.get(key);

        // Co-activation strength: cosine similarity of momenta
        const coActivation = momA > 0.01 && momB > 0.01
          ? cosineSimilarity(stateA.momentum, stateB.momentum)
          : 0;

        // Plasticity window: synapses are more plastic at moderate entropy
        const avgEntropy = (stateA.entropy + stateB.entropy) / 2;
        const plasticity = 1 - Math.abs(avgEntropy - 0.4) * 2; // Peak at 0.4
        const effectivePlasticity = Math.max(0.1, plasticity);

        if (synapse) {
          const weightBefore = synapse.weight;

          // STDP: compute dt from PREVIOUS fire ticks first,
          // then update fire ticks for next iteration.
          // This ensures dt reflects actual temporal ordering across ticks.
          const dt = synapse.postFireTick - synapse.preFireTick;

          if (momA > 0.1) synapse.preFireTick = this.tickCount;
          if (momB > 0.1) synapse.postFireTick = this.tickCount;
          // STDP temporal window (exponential decay)
          // dt > 0: pre fires before post → LTP (causal → strengthen)
          // dt < 0: post fires before pre → LTD (anti-causal → weaken)
          // dt = 0: simultaneous → standard Hebbian
          const stdpTau = 5; // Time constant in ticks
          const stdpFactor = dt === 0
            ? 1.0
            : dt > 0
              ? Math.exp(-dt / stdpTau)      // LTP: decays with delay
              : -0.5 * Math.exp(dt / stdpTau); // LTD: weaker, decays

          if (Math.abs(coActivation) > 0.1) {
            // Combined: Hebbian * STDP * plasticity
            const delta = coActivation * this.potentiationRate * effectivePlasticity * (1 + stdpFactor);
            synapse.weight = Math.max(-this.maxWeight,
              Math.min(this.maxWeight, synapse.weight + delta));
            synapse.activationCount++;
            synapse.lastActivatedAt = this.tickCount;

            // Eligibility trace: mark this synapse as recently active
            // Decays exponentially — used for delayed reward assignment
            synapse.eligibility = Math.min(1, synapse.eligibility + 0.3);

            const direction = norm(stateA.momentum) > norm(stateB.momentum)
              ? stateA.momentum : stateB.momentum;
            if (norm(direction) > 0.01) {
              synapse.conductionAxis = normalize(direction);
            }

            if (Math.abs(synapse.weight - weightBefore) > 0.01) {
              newEvents.push({
                type: 'POTENTIATED', preId: idA, postId: idB,
                weightBefore, weightAfter: synapse.weight, tick: this.tickCount,
              });
            }
          }

          // Decay eligibility trace
          synapse.eligibility *= 0.9;
        } else if (Math.abs(coActivation) > this.formationThreshold) {
          const conductionAxis = norm(stateA.momentum) > norm(stateB.momentum)
            ? normalize(stateA.momentum) : normalize(stateB.momentum);

          this.synapses.set(key, {
            preId: idA,
            postId: idB,
            weight: coActivation * 0.3,
            activationCount: 1,
            lastActivatedAt: this.tickCount,
            formedAt: this.tickCount,
            conductionAxis,
            preFireTick: momA > 0.1 ? this.tickCount : 0,
            postFireTick: momB > 0.1 ? this.tickCount : 0,
            eligibility: 0.3,
          });

          newEvents.push({
            type: 'FORMED', preId: idA, postId: idB,
            weightBefore: 0, weightAfter: coActivation * 0.3, tick: this.tickCount,
          });
        }
      }
    }

    // Phase 2: Depress unused synapses (synaptic pruning)
    for (const [key, synapse] of this.synapses) {
      const timeSinceActivation = this.tickCount - synapse.lastActivatedAt;

      if (timeSinceActivation > 0) {
        const weightBefore = synapse.weight;
        // Decay toward zero
        synapse.weight *= (1 - this.depressionRate * timeSinceActivation * 0.1);

        // Prune if too weak
        if (Math.abs(synapse.weight) < this.pruneThreshold) {
          this.synapses.delete(key);
          newEvents.push({
            type: 'PRUNED', preId: synapse.preId, postId: synapse.postId,
            weightBefore, weightAfter: 0, tick: this.tickCount,
          });
        } else if (Math.abs(synapse.weight - weightBefore) > 0.01) {
          newEvents.push({
            type: 'DEPRESSED', preId: synapse.preId, postId: synapse.postId,
            weightBefore, weightAfter: synapse.weight, tick: this.tickCount,
          });
        }
      }
    }

    this.events.push(...newEvents);
    return newEvents;
  }

  /**
   * Compute SYNAPTIC FORCE on an agent.
   *
   * Each synapse connecting to this agent contributes a force
   * along its conduction axis, weighted by the synapse strength.
   *
   * Excitatory synapses (weight > 0) PULL the agent along the axis.
   * Inhibitory synapses (weight < 0) PUSH the agent against it.
   */
  computeSynapticForce(agentId: AgentId, states: Map<AgentId, HyperState>): Vec {
    let force = zeros(this.dim);

    for (const synapse of this.synapses.values()) {
      if (synapse.preId !== agentId && synapse.postId !== agentId) continue;

      const otherId = synapse.preId === agentId ? synapse.postId : synapse.preId;
      const otherState = states.get(otherId);
      if (!otherState || norm(otherState.momentum) < 0.01) continue;

      // Force = weight * conduction axis * other's momentum magnitude
      const influence = synapse.weight * norm(otherState.momentum) * 0.3;
      force = add(force, scale(synapse.conductionAxis, influence));
    }

    return force;
  }

  /**
   * CREDIT ASSIGNMENT via eligibility traces.
   *
   * When a delayed reward/pain signal arrives (e.g., goal reached
   * 10 ticks after the decision that led to it), the eligibility
   * trace determines WHICH synapses were responsible.
   *
   * Recently active synapses (high eligibility) get more credit.
   * Old synapses (decayed eligibility) get less.
   *
   * @param agentId The agent that received the reward/pain
   * @param reward Positive = reward, negative = pain [-1, 1]
   */
  reinforceByReward(agentId: AgentId, reward: number): number {
    let modified = 0;
    for (const synapse of this.synapses.values()) {
      if (synapse.preId !== agentId && synapse.postId !== agentId) continue;
      if (synapse.eligibility < 0.01) continue;

      const delta = reward * synapse.eligibility * this.potentiationRate;
      synapse.weight = Math.max(-this.maxWeight, Math.min(this.maxWeight, synapse.weight + delta));
      modified++;
    }
    return modified;
  }

  /** Get all synapses for an agent */
  getSynapses(agentId: AgentId): Synapse[] {
    const result: Synapse[] = [];
    for (const synapse of this.synapses.values()) {
      if (synapse.preId === agentId || synapse.postId === agentId) {
        result.push(synapse);
      }
    }
    return result;
  }

  /** Get total synapse count */
  getSynapseCount(): number {
    return this.synapses.size;
  }

  /** Get all plasticity events */
  getEvents(): readonly PlasticityEvent[] {
    return this.events;
  }

  /** Get the strongest synapse */
  getStrongestSynapse(): Synapse | null {
    let strongest: Synapse | null = null;
    let maxWeight = 0;
    for (const synapse of this.synapses.values()) {
      if (Math.abs(synapse.weight) > maxWeight) {
        maxWeight = Math.abs(synapse.weight);
        strongest = synapse;
      }
    }
    return strongest;
  }

  private synapseKey(a: AgentId, b: AgentId): string {
    return a < b ? `${a}:${b}` : `${b}:${a}`;
  }
}
