/**
 * OASIS Kernel — Distributed Immune System
 *
 * HOW YOUR BODY FIGHTS INFECTION WITHOUT A GENERAL:
 * No central authority commands your immune cells.
 * Each cell patrols, detects anomalies, and recruits neighbors.
 * The response EMERGES from local interactions.
 *
 * OASIS agents do the same:
 * - Each agent monitors its neighbors' behavior
 * - Anomalies are detected by PREDICTION ERROR (like the
 *   Reflection Engine, but for OTHER agents' behavior)
 * - Detection triggers "antibody tensions" that contain the threat
 * - Nearby agents converge to quarantine the anomaly
 *
 * INNOVATION: Behavioral Immune Memory
 * When a threat pattern is detected and resolved, the immune
 * system remembers it. Next time a similar pattern appears,
 * the response is faster (like biological acquired immunity).
 */

import type { AgentId, TenantId } from '../types.js';
import { monotonicNow } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  sub,
  scale,
  normalize,
  norm,
  cosineSimilarity,
  distance,
} from '../physics/vector-math.js';
import type { HyperState } from '../physics/hyper-state.js';
import { StigmergyEngine, PheromoneType } from './stigmergy.js';
import type { TensionField } from '../physics/tension-field.js';

// ─── Threat Types ───────────────────────────────────────────────

export const ThreatType = {
  /** Agent behaving erratically — sudden trajectory changes */
  ERRATIC: 'ERRATIC',
  /** Agent moving against swarm consensus */
  CONTRARIAN: 'CONTRARIAN',
  /** Agent not responding — zombie process */
  UNRESPONSIVE: 'UNRESPONSIVE',
  /** Agent's entropy is dangerously high while still acting */
  ENTROPY_LEAK: 'ENTROPY_LEAK',
} as const;
export type ThreatType = (typeof ThreatType)[keyof typeof ThreatType];

export interface ThreatDetection {
  readonly suspectId: AgentId;
  readonly detectorId: AgentId;
  readonly type: ThreatType;
  readonly confidence: number;
  readonly evidence: string;
  readonly tick: number;
}

export interface ImmuneResponse {
  /** Threats detected this tick */
  readonly threats: ThreatDetection[];
  /** Agents currently quarantined */
  readonly quarantined: AgentId[];
  /** Antibody tensions injected */
  readonly antibodiesInjected: number;
  /** Known threat patterns (immune memory) */
  readonly memorizedPatterns: number;
}

// ─── Agent Behavior Profile ─────────────────────────────────────

interface BehaviorProfile {
  /** Recent trajectory directions */
  readonly recentDirections: Vec[];
  /** Average momentum magnitude */
  avgMomentum: number;
  /** Average entropy */
  avgEntropy: number;
  /** How often this agent changes direction sharply */
  directionChanges: number;
  /** Last known state */
  lastState: HyperState | null;
  /** Ticks since last state update */
  ticksSinceUpdate: number;
}

// ─── Immune Memory Pattern ──────────────────────────────────────

interface MemorizedPattern {
  readonly type: ThreatType;
  readonly signature: Vec;
  readonly occurrences: number;
  readonly firstSeen: number;
}

// ─── Immune System ──────────────────────────────────────────────

export class ImmuneSystem {
  private readonly profiles = new Map<AgentId, BehaviorProfile>();
  private readonly quarantine = new Set<AgentId>();
  private readonly memory: MemorizedPattern[] = [];
  private readonly dim: number;
  private readonly stigmergy: StigmergyEngine;
  private tickCount = 0;

  /** Threshold for erratic behavior detection */
  private readonly erraticThreshold: number;
  /** Threshold for entropy leak detection */
  private readonly entropyLeakThreshold: number;
  /** Ticks before an agent is considered unresponsive */
  private readonly unresponsiveTimeout: number;

  constructor(
    dim: number,
    stigmergy: StigmergyEngine,
    erraticThreshold = 0.7,
    entropyLeakThreshold = 0.8,
    unresponsiveTimeout = 20,
  ) {
    this.dim = dim;
    this.stigmergy = stigmergy;
    this.erraticThreshold = erraticThreshold;
    this.entropyLeakThreshold = entropyLeakThreshold;
    this.unresponsiveTimeout = unresponsiveTimeout;
  }

  /**
   * Monitor: update behavior profiles and detect threats.
   */
  monitor(
    allStates: Map<AgentId, HyperState>,
    field: TensionField,
    detectorId: AgentId,
  ): ImmuneResponse {
    this.tickCount++;
    const threats: ThreatDetection[] = [];
    let antibodiesInjected = 0;

    for (const [agentId, state] of allStates) {
      if (agentId === detectorId) continue; // Don't monitor yourself

      // Update behavior profile
      const profile = this.getOrCreateProfile(agentId);

      if (profile.lastState) {
        // Track direction changes
        const prevDir = profile.lastState.momentum;
        const currDir = state.momentum;

        if (norm(prevDir) > 1e-6 && norm(currDir) > 1e-6) {
          const dirChange = 1 - cosineSimilarity(prevDir, currDir);
          if (dirChange > this.erraticThreshold) {
            profile.directionChanges++;
          }

          profile.recentDirections.push(Float64Array.from(currDir));
          if (profile.recentDirections.length > 10) profile.recentDirections.shift();
        }

        // Update running averages
        profile.avgMomentum = profile.avgMomentum * 0.9 + norm(state.momentum) * 0.1;
        profile.avgEntropy = profile.avgEntropy * 0.9 + state.entropy * 0.1;
      }

      profile.lastState = state;
      profile.ticksSinceUpdate = 0;

      // ─── Threat Detection ───────────────────────────────

      // 1. ERRATIC: too many sharp direction changes
      if (profile.directionChanges > 5) {
        const threat = this.createThreat(agentId, detectorId, 'ERRATIC',
          Math.min(1, profile.directionChanges / 10),
          `${profile.directionChanges} sharp direction changes`,
        );
        threats.push(threat);
        this.emitAntibody(agentId, state, field);
        antibodiesInjected++;
      }

      // 2. ENTROPY_LEAK: high entropy while still active
      if (state.entropy > this.entropyLeakThreshold && norm(state.momentum) > 0.1) {
        const threat = this.createThreat(agentId, detectorId, 'ENTROPY_LEAK',
          state.entropy,
          `Entropy ${state.entropy.toFixed(3)} while momentum ${norm(state.momentum).toFixed(3)}`,
        );
        threats.push(threat);
        this.emitAntibody(agentId, state, field);
        antibodiesInjected++;
      }
    }

    // 3. UNRESPONSIVE: agents not in allStates but in profiles
    for (const [agentId, profile] of this.profiles) {
      if (!allStates.has(agentId)) {
        profile.ticksSinceUpdate++;

        if (profile.ticksSinceUpdate > this.unresponsiveTimeout) {
          threats.push(this.createThreat(agentId, detectorId, 'UNRESPONSIVE',
            Math.min(1, profile.ticksSinceUpdate / (this.unresponsiveTimeout * 2)),
            `No update for ${profile.ticksSinceUpdate} ticks`,
          ));
        }
      }
    }

    // Quarantine high-confidence threats
    for (const threat of threats) {
      if (threat.confidence > 0.7) {
        this.quarantine.add(threat.suspectId);

        // Memorize the pattern
        this.memorizePattern(threat, allStates.get(threat.suspectId));
      }
    }

    return {
      threats,
      quarantined: [...this.quarantine],
      antibodiesInjected,
      memorizedPatterns: this.memory.length,
    };
  }

  /** Release an agent from quarantine */
  release(agentId: AgentId): void {
    this.quarantine.delete(agentId);
    // Reset profile
    const profile = this.profiles.get(agentId);
    if (profile) {
      profile.directionChanges = 0;
      profile.recentDirections.length = 0;
    }
  }

  /** Is an agent quarantined? */
  isQuarantined(agentId: AgentId): boolean {
    return this.quarantine.has(agentId);
  }

  /** Get quarantined agents */
  getQuarantined(): AgentId[] {
    return [...this.quarantine];
  }

  /** Get immune memory */
  getMemory(): readonly MemorizedPattern[] {
    return this.memory;
  }

  // ─── Private ──────────────────────────────────────────────────

  private getOrCreateProfile(agentId: AgentId): BehaviorProfile {
    let profile = this.profiles.get(agentId);
    if (!profile) {
      profile = {
        recentDirections: [],
        avgMomentum: 0,
        avgEntropy: 0,
        directionChanges: 0,
        lastState: null,
        ticksSinceUpdate: 0,
      };
      this.profiles.set(agentId, profile);
    }
    return profile;
  }

  private createThreat(
    suspectId: AgentId,
    detectorId: AgentId,
    type: ThreatType,
    confidence: number,
    evidence: string,
  ): ThreatDetection {
    return { suspectId, detectorId, type, confidence, evidence, tick: this.tickCount };
  }

  /**
   * Emit an "antibody tension" — a strong REPEL pheromone
   * around the suspect that warns other agents away.
   */
  private emitAntibody(suspectId: AgentId, state: HyperState, field: TensionField): void {
    // Deposit REPEL pheromone at the suspect's position
    this.stigmergy.deposit(
      suspectId,
      PheromoneType.REPEL,
      state.position,
      state.momentum,
      5.0, // Strong repulsion
      0.03, // Slow decay — warning persists
    );
  }

  /**
   * Store a threat pattern in immune memory for faster
   * future detection.
   */
  private memorizePattern(threat: ThreatDetection, state: HyperState | undefined): void {
    if (!state) return;

    // Check if we already know this pattern
    const existing = this.memory.find(
      m => m.type === threat.type && cosineSimilarity(m.signature, state.momentum) > 0.8,
    );

    if (existing) {
      existing.occurrences++;
    } else {
      this.memory.push({
        type: threat.type,
        signature: Float64Array.from(state.momentum),
        occurrences: 1,
        firstSeen: this.tickCount,
      });
    }
  }
}
