/**
 * OASIS Kernel — Swarm Mind
 *
 * Emergent collective intelligence via crystallization, flocking,
 * healing, and fusion. No agent knows the swarm exists —
 * it EMERGES from local rules + the TensionField + stigmergy.
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
  weightedCentroid,
  lerp,
} from '../physics/vector-math.js';
import { computeFlockingForces, type FlockingForces } from './flocking.js';
import type { HyperState } from '../physics/hyper-state.js';
import { StigmergyEngine, PheromoneType } from './stigmergy.js';
import type { TensionField } from '../physics/tension-field.js';

// ─── Swarm Identity ─────────────────────────────────────────────

export interface SwarmId {
  readonly id: string;
  /** Tick when the swarm crystallized */
  readonly formedAt: number;
}

export interface Swarm {
  readonly id: SwarmId;
  /** Current members */
  readonly members: Set<AgentId>;
  /** Collective state — weighted centroid of member states */
  collectiveState: Vec;
  /** Collective momentum — average direction of movement */
  collectiveMomentum: Vec;
  /** Collective entropy — measure of swarm coherence */
  coherence: number;
  /** Swarm goal — emergent from member intentions */
  emergentGoal: Vec | null;
  /** Age in ticks */
  age: number;
}

// ─── Flocking Forces ────────────────────────────────────────────

export interface FlockingForces {
  /** Alignment: match neighbors' direction */
  readonly alignment: Vec;
  /** Cohesion: move toward swarm center */
  readonly cohesion: Vec;
  /** Separation: don't crowd too close */
  readonly separation: Vec;
  /** Combined flocking force */
  readonly combined: Vec;
}

// ─── Swarm Event ────────────────────────────────────────────────

export const SwarmEvent = {
  FORMED: 'FORMED',
  MEMBER_JOINED: 'MEMBER_JOINED',
  MEMBER_LEFT: 'MEMBER_LEFT',
  MEMBER_FAILED: 'MEMBER_FAILED',
  HEALING: 'HEALING',
  DISSOLVED: 'DISSOLVED',
  GOAL_CRYSTALLIZED: 'GOAL_CRYSTALLIZED',
} as const;
export type SwarmEventType = (typeof SwarmEvent)[keyof typeof SwarmEvent];

export interface SwarmEventRecord {
  readonly type: SwarmEventType;
  readonly swarmId: string;
  readonly agentId?: AgentId;
  readonly tick: number;
  readonly detail: string;
}

// ─── Swarm Mind ─────────────────────────────────────────────────

export class SwarmMind {
  private readonly swarms = new Map<string, Swarm>();
  private readonly agentSwarm = new Map<AgentId, string>(); // agent → swarm ID
  private readonly events: SwarmEventRecord[] = [];
  private readonly dim: number;
  private readonly tenantId: TenantId;
  private readonly stigmergy: StigmergyEngine;
  private tickCount = 0;
  private swarmCounter = 0;

  /** Minimum resonance strength to form a swarm */
  private readonly formationThreshold: number;
  /** Maximum distance for flocking neighbor detection */
  private readonly neighborRadius: number;
  /** Separation distance — agents repel if closer than this */
  private readonly separationDistance: number;

  constructor(
    dim: number,
    tenantId: TenantId,
    stigmergy: StigmergyEngine,
    formationThreshold = 0.5,
    neighborRadius = 3.0,
    separationDistance = 0.5,
  ) {
    this.dim = dim;
    this.tenantId = tenantId;
    this.stigmergy = stigmergy;
    this.formationThreshold = formationThreshold;
    this.neighborRadius = neighborRadius;
    this.separationDistance = separationDistance;
  }

  /** CRYSTALLIZE: Form swarms from resonating agents. */
  crystallize(states: Map<AgentId, HyperState>): SwarmEventRecord[] {
    const newEvents: SwarmEventRecord[] = [];
    const agents = [...states.entries()];

    // Find resonating pairs
    for (let i = 0; i < agents.length; i++) {
      for (let j = i + 1; j < agents.length; j++) {
        const [idA, stateA] = agents[i]!;
        const [idB, stateB] = agents[j]!;

        // Skip if either has no momentum
        if (norm(stateA.momentum) < 1e-6 || norm(stateB.momentum) < 1e-6) continue;

        // Check resonance: aligned momentum + spatial proximity
        const momentumAlignment = cosineSimilarity(stateA.momentum, stateB.momentum);
        const spatialProximity = 1 / (1 + distance(stateA.position, stateB.position));
        const resonance = momentumAlignment * spatialProximity;

        if (resonance < this.formationThreshold) continue;

        // These agents are resonating — join or form a swarm
        const swarmA = this.agentSwarm.get(idA);
        const swarmB = this.agentSwarm.get(idB);

        if (swarmA && swarmB && swarmA === swarmB) {
          // Already in the same swarm
          continue;
        } else if (swarmA && !swarmB) {
          // B joins A's swarm
          this.joinSwarm(idB, swarmA, stateB);
          newEvents.push(this.createEvent('MEMBER_JOINED', swarmA, idB, `Resonance: ${resonance.toFixed(3)}`));
        } else if (!swarmA && swarmB) {
          // A joins B's swarm
          this.joinSwarm(idA, swarmB, stateA);
          newEvents.push(this.createEvent('MEMBER_JOINED', swarmB, idA, `Resonance: ${resonance.toFixed(3)}`));
        } else if (!swarmA && !swarmB) {
          // Form new swarm
          const swarm = this.formSwarm(idA, idB, stateA, stateB);
          newEvents.push(this.createEvent('FORMED', swarm.id.id, undefined, `Members: ${idA}, ${idB}`));
        }
        else if (swarmA && swarmB && swarmA !== swarmB) {
          // SWARM FUSION: two different swarms with resonating members → merge
          newEvents.push(...this.mergeSwarms(swarmA, swarmB, states));
        }
      }
    }

    return newEvents;
  }

  /** Compute flocking forces (alignment, cohesion, separation) for a swarm member */
  computeFlocking(agentId: AgentId, agentState: HyperState, allStates: Map<AgentId, HyperState>): FlockingForces {
    const swarmId = this.agentSwarm.get(agentId);
    if (!swarmId) {
      const z = zeros(this.dim);
      return { alignment: z, cohesion: z, separation: z, combined: z };
    }
    const swarm = this.swarms.get(swarmId)!;
    return computeFlockingForces(agentId, agentState, allStates, swarm, this.dim, this.neighborRadius, this.separationDistance);
  }

  /** HEAL: Detect failed members, recruit, or dissolve. */
  heal(currentAgentIds: Set<AgentId>): SwarmEventRecord[] {
    const newEvents: SwarmEventRecord[] = [];

    for (const [swarmId, swarm] of this.swarms) {
      const failed: AgentId[] = [];

      for (const memberId of swarm.members) {
        if (!currentAgentIds.has(memberId)) {
          failed.push(memberId);
        }
      }

      for (const failedId of failed) {
        swarm.members.delete(failedId);
        this.agentSwarm.delete(failedId);

        // Emit recruitment pheromone at the gap
        this.stigmergy.deposit(
          failedId,
          PheromoneType.RECRUIT,
          swarm.collectiveState,
          swarm.collectiveMomentum,
          3.0, // Strong recruitment signal
          0.02, // Slow decay
        );

        newEvents.push(this.createEvent('MEMBER_FAILED', swarmId, failedId, 'Agent lost — recruitment emitted'));
        newEvents.push(this.createEvent('HEALING', swarmId, undefined, `Recruiting to fill gap left by ${failedId}`));
      }

      // Dissolve if too few members
      if (swarm.members.size < 2) {
        for (const remainingId of swarm.members) {
          this.agentSwarm.delete(remainingId);
        }
        this.swarms.delete(swarmId);
        newEvents.push(this.createEvent('DISSOLVED', swarmId, undefined, `Only ${swarm.members.size} member(s) remaining`));
      }
    }

    return newEvents;
  }

  /**
   * UPDATE collective state from member HyperStates.
   * Also detect emergent goals.
   */
  updateCollective(allStates: Map<AgentId, HyperState>): void {
    for (const swarm of this.swarms.values()) {
      const memberStates: Vec[] = [];
      const memberMomenta: Vec[] = [];
      const weights: number[] = [];

      for (const memberId of swarm.members) {
        const state = allStates.get(memberId);
        if (!state) continue;
        memberStates.push(state.position);
        memberMomenta.push(state.momentum);
        weights.push(1 - state.entropy); // Higher certainty = more weight
      }

      if (memberStates.length === 0) continue;

      // Collective state = weighted centroid
      swarm.collectiveState = weightedCentroid(memberStates, weights);

      // Collective momentum = weighted average direction
      swarm.collectiveMomentum = weightedCentroid(memberMomenta, weights);

      // Coherence = average pairwise alignment of momenta
      let totalAlignment = 0;
      let pairs = 0;
      for (let i = 0; i < memberMomenta.length; i++) {
        for (let j = i + 1; j < memberMomenta.length; j++) {
          if (norm(memberMomenta[i]!) > 1e-6 && norm(memberMomenta[j]!) > 1e-6) {
            totalAlignment += cosineSimilarity(memberMomenta[i]!, memberMomenta[j]!);
            pairs++;
          }
        }
      }
      swarm.coherence = pairs > 0 ? (totalAlignment / pairs + 1) / 2 : 0.5;

      // Emergent goal: if coherence is high AND collective momentum is strong,
      // the swarm "knows" where it wants to go
      if (swarm.coherence > 0.7 && norm(swarm.collectiveMomentum) > 0.1) {
        swarm.emergentGoal = normalize(swarm.collectiveMomentum);
      }

      swarm.age++;
    }
  }

  /**
   * Tick: evaporate, heal, update.
   */
  tick(allStates: Map<AgentId, HyperState>): SwarmEventRecord[] {
    this.tickCount++;
    const events: SwarmEventRecord[] = [];

    // Crystallize new swarms
    events.push(...this.crystallize(allStates));

    // Heal broken swarms
    events.push(...this.heal(new Set(allStates.keys())));

    // Update collective states
    this.updateCollective(allStates);

    this.events.push(...events);
    return events;
  }

  // ─── Accessors ────────────────────────────────────────────────

  getSwarm(swarmId: string): Swarm | undefined {
    return this.swarms.get(swarmId);
  }

  getAgentSwarm(agentId: AgentId): Swarm | undefined {
    const swarmId = this.agentSwarm.get(agentId);
    return swarmId ? this.swarms.get(swarmId) : undefined;
  }

  getSwarmCount(): number {
    return this.swarms.size;
  }

  getAllEvents(): readonly SwarmEventRecord[] {
    return this.events;
  }

  // ─── Private ──────────────────────────────────────────────────

  /**
   * SWARM FUSION: Merge two swarms into one.
   * The larger swarm absorbs the smaller one.
   * Collective state is recomputed from all members.
   */
  private mergeSwarms(
    swarmIdA: string,
    swarmIdB: string,
    states: Map<AgentId, HyperState>,
  ): SwarmEventRecord[] {
    const events: SwarmEventRecord[] = [];
    const swarmA = this.swarms.get(swarmIdA);
    const swarmB = this.swarms.get(swarmIdB);
    if (!swarmA || !swarmB) return events;

    // Larger swarm absorbs smaller
    const [keeper, absorbed] = swarmA.members.size >= swarmB.members.size
      ? [swarmA, swarmB]
      : [swarmB, swarmA];
    const keeperId = keeper === swarmA ? swarmIdA : swarmIdB;
    const absorbedId = keeper === swarmA ? swarmIdB : swarmIdA;

    // Move all members from absorbed into keeper
    for (const memberId of absorbed.members) {
      keeper.members.add(memberId);
      this.agentSwarm.set(memberId, keeperId);
    }

    // Remove absorbed swarm
    this.swarms.delete(absorbedId);

    events.push(this.createEvent('DISSOLVED', absorbedId, undefined,
      `Merged into ${keeperId} (${keeper.members.size} total members)`));
    events.push(this.createEvent('FORMED', keeperId, undefined,
      `Absorbed ${absorbedId} — now ${keeper.members.size} members`));

    return events;
  }

  private formSwarm(idA: AgentId, idB: AgentId, stateA: HyperState, stateB: HyperState): Swarm {
    const swarmId: SwarmId = {
      id: `swarm-${++this.swarmCounter}`,
      formedAt: this.tickCount,
    };

    const members = new Set([idA, idB]);
    const collectiveState = lerp(stateA.position, stateB.position, 0.5);
    const collectiveMomentum = lerp(stateA.momentum, stateB.momentum, 0.5);

    const swarm: Swarm = {
      id: swarmId,
      members,
      collectiveState,
      collectiveMomentum,
      coherence: cosineSimilarity(stateA.momentum, stateB.momentum),
      emergentGoal: null,
      age: 0,
    };

    this.swarms.set(swarmId.id, swarm);
    this.agentSwarm.set(idA, swarmId.id);
    this.agentSwarm.set(idB, swarmId.id);

    return swarm;
  }

  private joinSwarm(agentId: AgentId, swarmId: string, state: HyperState): void {
    const swarm = this.swarms.get(swarmId);
    if (!swarm) return;
    swarm.members.add(agentId);
    this.agentSwarm.set(agentId, swarmId);
  }

  private createEvent(type: SwarmEventType, swarmId: string, agentId?: AgentId, detail = ''): SwarmEventRecord {
    return { type, swarmId, agentId, tick: this.tickCount, detail };
  }
}
