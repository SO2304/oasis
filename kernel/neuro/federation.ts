/**
 * OASIS Kernel — Federated Neural Mesh
 *
 * "What one neuron learns, the network remembers" — Claim 11
 *
 * THE PROBLEM: In a multi-agent system, each synapse pair learns
 * in isolation. Agent A discovers that "moving left avoids pain"
 * but Agent B has to learn this from scratch. Experience dies
 * with the pair that earned it.
 *
 * THE SOLUTION: Synaptic Resonance Protocol.
 *
 * When a synapse undergoes significant plasticity (weight changes
 * beyond a threshold), the learning event is compressed into an
 * ExperienceDigest — a compact vector encoding:
 *   - WHAT was learned (the conduction axis)
 *   - HOW MUCH (magnitude of weight change)
 *   - GOOD OR BAD (valence: potentiation vs depression)
 *   - IN WHAT CONTEXT (entropy level during learning)
 *
 * This digest is NOT a message. It's emitted as a RESONANCE FORCE
 * into the tension field. Other agents' synapses that share a
 * similar conduction axis (cosine similarity > threshold) receive
 * a SYMPATHETIC UPDATE — their weights shift in the same direction,
 * scaled by axis alignment and inter-agent trust.
 *
 * KEY PROPERTIES:
 *   - Privacy: only direction + magnitude flow, not raw weights
 *   - Non-intrusive: SynapticNetwork is NOT modified
 *   - Trust-weighted: unreliable agents' resonance is discounted
 *   - Convergent: collective learning > individual learning
 *   - Compatible: uses existing tension field (R13 compliant)
 *
 * PATENT CLAIM 11 — Federated Synaptic Resonance
 * Method of collective learning in which autonomous agents share
 * learning experiences not as raw synaptic weights but as resonance
 * vectors emitted into a shared tension field, where receiving
 * agents' synapses that share conduction axis alignment absorb
 * the resonance proportionally to axis similarity and trust level,
 * enabling collective knowledge without centralized aggregation
 * or privacy violation.
 */

import type { AgentId } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  scale,
  norm,
  normalize,
  cosineSimilarity,
  dot,
} from '../physics/vector-math.js';
import type { Synapse, PlasticityEvent, SynapticNetwork } from './synapse.js';

// ─── Experience Digest ─────────────────────────────────────

export interface ExperienceDigest {
  /** Agent that generated this experience */
  readonly sourceAgent: AgentId;
  /** The axis in latent space along which learning occurred */
  readonly axis: Vec;
  /** Magnitude of the weight change (always positive) */
  readonly magnitude: number;
  /** Valence: +1 = potentiation, -1 = depression */
  readonly valence: number;
  /** Entropy context when learning occurred */
  readonly entropy: number;
  /** Tick when this experience was generated */
  readonly tick: number;
  /** How many times this pattern was reinforced */
  reinforcements: number;
}

// ─── Resonance Event ───────────────────────────────────────

export interface ResonanceEvent {
  readonly type: 'SYMPATHETIC' | 'ABSORBED' | 'REJECTED';
  readonly sourceAgent: AgentId;
  readonly targetSynapse: { pre: AgentId; post: AgentId };
  readonly alignment: number;
  readonly weightDelta: number;
  readonly tick: number;
}

// ─── Federated Neural Mesh ─────────────────────────────────

export class FederatedMesh {
  private readonly dim: number;

  /** Pool of unprocessed experience digests */
  private readonly digestPool: ExperienceDigest[] = [];

  /** Trust matrix: "agentA:agentB" → trust level [0, 1] */
  private readonly trust = new Map<string, number>();

  /** Resonance events log */
  private readonly events: ResonanceEvent[] = [];

  /** Minimum weight change to emit an experience digest */
  private readonly significanceThreshold: number;

  /** Minimum axis alignment for sympathetic resonance */
  private readonly resonanceThreshold: number;

  /** How strongly resonance affects target synapses */
  private readonly resonanceGain: number;

  /** How fast trust adapts */
  private readonly trustLearningRate: number;

  /** Default trust for new agent pairs */
  private readonly defaultTrust: number;

  /** Max digests in pool (prevents memory bloat) */
  private readonly maxPoolSize: number;

  private tickCount = 0;

  constructor(
    dim: number,
    significanceThreshold = 0.05,
    resonanceThreshold = 0.3,
    resonanceGain = 0.02,
    trustLearningRate = 0.1,
    defaultTrust = 0.5,
    maxPoolSize = 64,
  ) {
    this.dim = dim;
    this.significanceThreshold = significanceThreshold;
    this.resonanceThreshold = resonanceThreshold;
    this.resonanceGain = resonanceGain;
    this.trustLearningRate = trustLearningRate;
    this.defaultTrust = defaultTrust;
    this.maxPoolSize = maxPoolSize;
  }

  /**
   * PHASE 1: HARVEST
   *
   * After SynapticNetwork.update(), scan plasticity events for
   * significant learning. Convert to ExperienceDigests.
   *
   * Only POTENTIATED and DEPRESSED events above the significance
   * threshold produce digests. FORMED and PRUNED are structural
   * events, not transferable experience.
   */
  harvest(
    events: readonly PlasticityEvent[],
    network: SynapticNetwork,
    agentEntropies: Map<AgentId, number>,
  ): number {
    this.tickCount++;
    let harvested = 0;

    for (const event of events) {
      if (event.type !== 'POTENTIATED' && event.type !== 'DEPRESSED') continue;

      const delta = Math.abs(event.weightAfter - event.weightBefore);
      if (delta < this.significanceThreshold) continue;

      const synapses = network.getSynapses(event.preId);
      const synapse = synapses.find(
        s => (s.preId === event.preId && s.postId === event.postId)
          || (s.preId === event.postId && s.postId === event.preId),
      );
      if (!synapse || norm(synapse.conductionAxis) < 0.01) continue;

      const avgEntropy = (
        (agentEntropies.get(event.preId) ?? 0.5) +
        (agentEntropies.get(event.postId) ?? 0.5)
      ) / 2;

      // Check if similar digest already exists (dedup)
      const existing = this.digestPool.find(d =>
        d.sourceAgent === event.preId &&
        cosineSimilarity(d.axis, synapse.conductionAxis) > 0.9,
      );

      if (existing) {
        existing.reinforcements++;
      } else {
        if (this.digestPool.length >= this.maxPoolSize) {
          // Evict oldest digest
          this.digestPool.shift();
        }
        this.digestPool.push({
          sourceAgent: event.preId,
          axis: normalize(synapse.conductionAxis),
          magnitude: delta,
          valence: event.type === 'POTENTIATED' ? 1 : -1,
          entropy: avgEntropy,
          tick: event.tick,
          reinforcements: 1,
        });
      }
      harvested++;
    }

    return harvested;
  }

  /**
   * PHASE 2: PROPAGATE RESONANCE
   *
   * For each digest in the pool, scan ALL synapses in the network.
   * If a synapse's conduction axis aligns with the digest's axis
   * (cosine > threshold), apply a sympathetic weight update.
   *
   * The update is scaled by:
   *   - Axis alignment (cosine similarity)
   *   - Trust level between source agent and synapse agents
   *   - Digest magnitude and reinforcement count
   *   - Context match (entropy similarity)
   *
   * Returns resonance forces that can be injected into the
   * tension field (one force per affected agent).
   */
  propagate(
    network: SynapticNetwork,
    agentIds: AgentId[],
    agentEntropies: Map<AgentId, number>,
  ): { forces: Map<AgentId, Vec>; events: ResonanceEvent[] } {
    const forces = new Map<AgentId, Vec>();
    const newEvents: ResonanceEvent[] = [];

    for (const agentId of agentIds) {
      forces.set(agentId, zeros(this.dim));
    }

    const processed: number[] = [];

    for (let di = 0; di < this.digestPool.length; di++) {
      const digest = this.digestPool[di]!;

      // Don't propagate stale digests (> 50 ticks old)
      if (this.tickCount - digest.tick > 50) {
        processed.push(di);
        continue;
      }

      for (const agentId of agentIds) {
        // Don't resonate with yourself
        if (agentId === digest.sourceAgent) continue;

        const trust = this.getTrust(digest.sourceAgent, agentId);
        if (trust < 0.1) continue; // Too little trust, skip

        const synapses = network.getSynapses(agentId);

        for (const synapse of synapses) {
          if (norm(synapse.conductionAxis) < 0.01) continue;

          const alignment = cosineSimilarity(
            digest.axis,
            synapse.conductionAxis,
          );

          if (Math.abs(alignment) < this.resonanceThreshold) {
            newEvents.push({
              type: 'REJECTED',
              sourceAgent: digest.sourceAgent,
              targetSynapse: { pre: synapse.preId, post: synapse.postId },
              alignment,
              weightDelta: 0,
              tick: this.tickCount,
            });
            continue;
          }

          // Context match: learning transfers better in similar entropy
          const currentEntropy = agentEntropies.get(agentId) ?? 0.5;
          const entropyMatch = 1 - Math.abs(currentEntropy - digest.entropy);

          // Sympathetic weight delta
          const reinforcementBoost = Math.min(3, 1 + Math.log2(digest.reinforcements));
          const weightDelta = alignment
            * digest.valence
            * digest.magnitude
            * trust
            * entropyMatch
            * reinforcementBoost
            * this.resonanceGain;

          // Apply to synapse weight (bounded)
          synapse.weight = Math.max(-1, Math.min(1,
            synapse.weight + weightDelta,
          ));

          // Generate resonance force for this agent
          const force = forces.get(agentId)!;
          const resonanceForce = scale(
            digest.axis,
            weightDelta * 0.5,
          );
          forces.set(agentId, add(force, resonanceForce));

          newEvents.push({
            type: 'SYMPATHETIC',
            sourceAgent: digest.sourceAgent,
            targetSynapse: { pre: synapse.preId, post: synapse.postId },
            alignment,
            weightDelta,
            tick: this.tickCount,
          });
        }
      }

      // Mark as absorbed after full propagation
      if (digest.reinforcements <= 1) {
        processed.push(di);
      } else {
        digest.reinforcements--;
      }
    }

    // Remove processed digests (reverse order to preserve indices)
    for (const idx of processed.sort((a, b) => b - a)) {
      this.digestPool.splice(idx, 1);
    }

    this.events.push(...newEvents);
    return { forces, events: newEvents };
  }

  /**
   * UPDATE TRUST
   *
   * Trust increases when resonance from an agent leads to
   * positive outcomes (reward), decreases on negative outcomes.
   *
   * @param sourceAgent The agent whose resonance we're evaluating
   * @param targetAgent The agent that received the resonance
   * @param outcome +1 = beneficial, -1 = harmful
   */
  updateTrust(sourceAgent: AgentId, targetAgent: AgentId, outcome: number): void {
    const key = this.trustKey(sourceAgent, targetAgent);
    const current = this.trust.get(key) ?? this.defaultTrust;
    const updated = Math.max(0, Math.min(1,
      current + outcome * this.trustLearningRate,
    ));
    this.trust.set(key, updated);
  }

  getTrust(a: AgentId, b: AgentId): number {
    return this.trust.get(this.trustKey(a, b)) ?? this.defaultTrust;
  }

  getDigestCount(): number {
    return this.digestPool.length;
  }

  getEvents(): readonly ResonanceEvent[] {
    return this.events;
  }

  getResonanceStats(): {
    total: number;
    sympathetic: number;
    rejected: number;
    avgAlignment: number;
  } {
    const sympathetic = this.events.filter(e => e.type === 'SYMPATHETIC');
    const rejected = this.events.filter(e => e.type === 'REJECTED');
    const avgAlignment = sympathetic.length > 0
      ? sympathetic.reduce((s, e) => s + Math.abs(e.alignment), 0) / sympathetic.length
      : 0;
    return {
      total: this.events.length,
      sympathetic: sympathetic.length,
      rejected: rejected.length,
      avgAlignment,
    };
  }

  private trustKey(a: AgentId, b: AgentId): string {
    return `${a}→${b}`;
  }
}
