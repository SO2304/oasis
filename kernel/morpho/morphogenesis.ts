/**
 * OASIS Kernel — Morphogenesis Engine
 *
 * NO OTHER SYSTEM HAS THIS.
 *
 * In biology, a single fertilized egg becomes a brain, heart,
 * liver, muscles — not because each cell is pre-programmed,
 * but because chemical gradients tell cells WHAT TO BECOME
 * based on WHERE THEY ARE.
 *
 * OASIS agents work the same way:
 *
 * 1. All agents start UNDIFFERENTIATED (stem cells)
 * 2. The tension field creates "morphogenetic gradients"
 * 3. Each agent reads the local gradient and SPECIALIZES
 *    into the role the field needs most
 * 4. Specialization is REVERSIBLE — if a navigator dies,
 *    a nearby generalist can re-differentiate to replace it
 *
 * ROLE TYPES (analogous to cell types):
 * - NAVIGATOR: follows attractive gradients, plans paths
 * - SENTINEL: monitors for threats, feeds immune system
 * - WORKER: follows pheromone trails, executes tasks
 * - SCOUT: seeks high-entropy regions, explores unknown
 * - HEALER: responds to recruitment pheromones, fills gaps
 *
 * KEY INSIGHT: The number and ratio of each role is not
 * configured — it EMERGES from the field topology.
 * More threats → more sentinels. More unknown area → more scouts.
 * This is self-organizing resource allocation.
 */

import type { AgentId } from '../types.js';
import { monotonicNow } from '../types.js';
import {
  type Vec,
  zeros,
  norm,
  normalize,
  distance,
  cosineSimilarity,
} from '../physics/vector-math.js';
import type { HyperState } from '../physics/hyper-state.js';

// ─── Agent Roles ────────────────────────────────────────────────

export const AgentRole = {
  /** Undifferentiated — can become anything */
  STEM: 'STEM',
  /** Path planning and goal navigation */
  NAVIGATOR: 'NAVIGATOR',
  /** Threat detection and immune support */
  SENTINEL: 'SENTINEL',
  /** Task execution along pheromone trails */
  WORKER: 'WORKER',
  /** Exploration of unknown regions */
  SCOUT: 'SCOUT',
  /** Gap-filling when agents fail */
  HEALER: 'HEALER',
} as const;
export type AgentRole = (typeof AgentRole)[keyof typeof AgentRole];

// ─── Role Profile ───────────────────────────────────────────────

export interface RoleProfile {
  readonly agentId: AgentId;
  /** Current role */
  role: AgentRole;
  /** How committed to this role [0, 1] — high = hard to re-differentiate */
  commitment: number;
  /** Tick when last differentiation occurred */
  differentiatedAt: number;
  /** Performance in current role [0, 1] */
  performance: number;
}

// ─── Field Needs (what the system lacks) ────────────────────────

export interface FieldNeeds {
  /** How much the field needs each role [0, 1] */
  readonly needs: Record<AgentRole, number>;
  /** Which role is most needed */
  readonly mostNeeded: AgentRole;
  /** Which role has surplus */
  readonly leastNeeded: AgentRole;
}

// ─── Morphogenetic Event ────────────────────────────────────────

export interface MorphoEvent {
  readonly agentId: AgentId;
  readonly fromRole: AgentRole;
  readonly toRole: AgentRole;
  readonly reason: string;
  readonly tick: number;
}

// ─── Morphogenesis Engine ───────────────────────────────────────

export class MorphogenesisEngine {
  private readonly profiles = new Map<AgentId, RoleProfile>();
  private readonly events: MorphoEvent[] = [];
  private readonly dim: number;
  private tickCount = 0;

  /** How fast commitment grows per tick in current role */
  private readonly commitmentGrowth: number;
  /** Commitment threshold below which re-differentiation is easy */
  private readonly flexibilityThreshold: number;
  /** Minimum ticks before an agent can re-differentiate */
  private readonly cooldownTicks: number;

  constructor(
    dim: number,
    commitmentGrowth = 0.02,
    flexibilityThreshold = 0.5,
    cooldownTicks = 10,
  ) {
    this.dim = dim;
    this.commitmentGrowth = commitmentGrowth;
    this.flexibilityThreshold = flexibilityThreshold;
    this.cooldownTicks = cooldownTicks;
  }

  /** Register an agent as undifferentiated stem cell */
  register(agentId: AgentId): void {
    this.profiles.set(agentId, {
      agentId,
      role: AgentRole.STEM,
      commitment: 0,
      differentiatedAt: this.tickCount,
      performance: 0,
    });
  }

  /**
   * DIFFERENTIATE: Assign roles based on field needs.
   *
   * @param agentStates Current HyperStates of all agents
   * @param threatLevel Current threat level from immune system [0,1]
   * @param unknownRatio Fraction of world that is unexplored [0,1]
   * @param healingNeeded Number of recruitment pheromones active
   * @param goalExists Whether there's an active goal
   */
  differentiate(
    agentStates: Map<AgentId, HyperState>,
    threatLevel: number,
    unknownRatio: number,
    healingNeeded: number,
    goalExists: boolean,
  ): MorphoEvent[] {
    this.tickCount++;
    const newEvents: MorphoEvent[] = [];

    // Assess what the field needs
    const needs = this.assessNeeds(threatLevel, unknownRatio, healingNeeded, goalExists);

    // Count current role distribution
    const roleCounts = this.countRoles();

    // For each stem or flexible agent, assign the most needed role
    for (const [agentId, profile] of this.profiles) {
      const state = agentStates.get(agentId);
      if (!state) continue;

      // Grow commitment over time
      if (profile.role !== AgentRole.STEM) {
        profile.commitment = Math.min(1, profile.commitment + this.commitmentGrowth);
      }

      // Check if re-differentiation is possible
      const canRedifferentiate =
        profile.role === AgentRole.STEM ||
        (profile.commitment < this.flexibilityThreshold &&
         this.tickCount - profile.differentiatedAt > this.cooldownTicks);

      if (!canRedifferentiate) continue;

      // Find the best role for this agent based on field needs
      const bestRole = this.selectRole(agentId, state, needs, roleCounts);

      if (bestRole !== profile.role) {
        const fromRole = profile.role;
        profile.role = bestRole;
        profile.commitment = 0.1; // Fresh commitment
        profile.differentiatedAt = this.tickCount;

        const event: MorphoEvent = {
          agentId,
          fromRole,
          toRole: bestRole,
          reason: `Field needs ${bestRole} (need=${needs.needs[bestRole].toFixed(2)})`,
          tick: this.tickCount,
        };
        newEvents.push(event);
        this.events.push(event);

        // Update counts
        roleCounts[fromRole] = (roleCounts[fromRole] ?? 0) - 1;
        roleCounts[bestRole] = (roleCounts[bestRole] ?? 0) + 1;
      }
    }

    return newEvents;
  }

  /**
   * Report performance in current role — affects re-differentiation.
   * High performers stay committed. Low performers become flexible again.
   */
  reportPerformance(agentId: AgentId, performance: number): void {
    const profile = this.profiles.get(agentId);
    if (!profile) return;

    profile.performance = performance;

    // Poor performance reduces commitment (makes re-differentiation easier)
    if (performance < 0.3) {
      profile.commitment = Math.max(0, profile.commitment - 0.05);
    }
  }

  /** Get role of an agent */
  getRole(agentId: AgentId): AgentRole {
    return this.profiles.get(agentId)?.role ?? AgentRole.STEM;
  }

  /** Get all agents with a specific role */
  getAgentsByRole(role: AgentRole): AgentId[] {
    const result: AgentId[] = [];
    for (const [id, profile] of this.profiles) {
      if (profile.role === role) result.push(id);
    }
    return result;
  }

  /** Get role distribution */
  getRoleDistribution(): Record<AgentRole, number> {
    return this.countRoles();
  }

  /** Get all morphogenetic events */
  getEvents(): readonly MorphoEvent[] {
    return this.events;
  }

  /** Get profile */
  getProfile(agentId: AgentId): RoleProfile | undefined {
    return this.profiles.get(agentId);
  }

  // ─── Private ──────────────────────────────────────────────────

  private assessNeeds(
    threatLevel: number,
    unknownRatio: number,
    healingNeeded: number,
    goalExists: boolean,
  ): FieldNeeds {
    // MORPHOGEN GRADIENT (Claim 6 — Patent-optimized)
    //
    // In biological morphogenesis, cells differentiate based on
    // CONCENTRATION GRADIENTS of signaling molecules (morphogens).
    // High BMP4 → bone. High Sonic Hedgehog → neural tube.
    //
    // OASIS morphogens are the environmental signals themselves:
    // - threatLevel IS the "immune morphogen"
    // - unknownRatio IS the "exploration morphogen"
    // - healingNeeded IS the "repair morphogen"
    // - goalExists IS the "navigation morphogen"
    //
    // The key innovation: morphogen concentrations INTERACT.
    // High threat + high unknown = prioritize SENTINEL over SCOUT
    // (survive first, explore later — like cortisol suppressing growth)
    const threatMorphogen = Math.min(1, threatLevel * 2);
    const explorationMorphogen = Math.min(1, unknownRatio * 1.5) * (1 - threatMorphogen * 0.5); // Threat suppresses exploration
    const repairMorphogen = Math.min(1, healingNeeded * 0.5);
    const navigationMorphogen = goalExists ? 0.6 * (1 - repairMorphogen * 0.3) : 0.2; // Repair needs reduce nav priority

    const needs: Record<AgentRole, number> = {
      STEM: 0.1 * (1 - threatMorphogen), // Under threat, fewer reserves
      NAVIGATOR: navigationMorphogen,
      SENTINEL: threatMorphogen,
      WORKER: 0.4 * (1 - explorationMorphogen * 0.3), // Exploration reduces worker need
      SCOUT: explorationMorphogen,
      HEALER: repairMorphogen,
    };

    // Find most and least needed
    let mostNeeded: AgentRole = AgentRole.WORKER;
    let leastNeeded: AgentRole = AgentRole.STEM;
    let maxNeed = 0;
    let minNeed = Infinity;

    for (const [role, need] of Object.entries(needs)) {
      if (role === 'STEM') continue; // Skip STEM for comparison
      if (need > maxNeed) { maxNeed = need; mostNeeded = role as AgentRole; }
      if (need < minNeed) { minNeed = need; leastNeeded = role as AgentRole; }
    }

    return { needs, mostNeeded, leastNeeded };
  }

  private selectRole(
    _agentId: AgentId,
    state: HyperState,
    needs: FieldNeeds,
    currentCounts: Record<string, number>,
  ): AgentRole {
    // Score each role: need * scarcity^2 * aptitude
    // Scarcity is squared to strongly favor under-represented roles
    // This prevents WORKER domination when needs are similar
    let bestRole = AgentRole.WORKER;
    let bestScore = -Infinity;

    const roles: AgentRole[] = ['NAVIGATOR', 'SENTINEL', 'WORKER', 'SCOUT', 'HEALER'];
    const totalAgents = Object.values(currentCounts).reduce((s, c) => s + c, 0) || 1;

    for (const role of roles) {
      const need = needs.needs[role];
      const currentCount = currentCounts[role] ?? 0;
      const targetRatio = need / (Object.values(needs.needs).reduce((s, n) => s + n, 0) || 1);
      const actualRatio = currentCount / totalAgents;
      // Deficit: how far below target this role is
      const deficit = Math.max(0, targetRatio - actualRatio);
      const scarcity = 1 / (currentCount + 0.5); // Smoother scarcity curve
      const aptitude = this.assessAptitude(state, role);

      // Combined score: deficit drives allocation, aptitude refines it
      const score = (need + deficit * 2) * scarcity * aptitude;

      if (score > bestScore) {
        bestScore = score;
        bestRole = role;
      }
    }

    return bestRole;
  }

  /**
   * Assess how well-suited an agent is for a role based on
   * its current HyperState.
   *
   * This is the "epigenetic" factor — the agent's position
   * in latent space predisposes it toward certain roles.
   */
  private assessAptitude(state: HyperState, role: AgentRole): number {
    const mom = norm(state.momentum);
    const e = state.entropy;

    switch (role) {
      case AgentRole.NAVIGATOR:
        // Strong momentum = good navigator. Even low-entropy agents qualify
        // if they have any directional tendency
        return Math.min(1, mom * 3 + 0.2) * (1 - e * 0.3);

      case AgentRole.SENTINEL:
        // Sentinels need awareness — moderate entropy is optimal
        // but any agent can sentinel if the field demands it
        return 0.5 + (1 - Math.abs(e - 0.4)) * 0.5;

      case AgentRole.WORKER:
        // Workers are the default, but aptitude shouldn't dominate
        return 0.5 + (1 - e) * 0.2;

      case AgentRole.SCOUT:
        // Scouts thrive in uncertainty OR when low-momentum (nowhere to go → explore)
        return 0.4 + Math.min(1, e * 1.2) * 0.3 + (mom < 0.1 ? 0.3 : 0);

      case AgentRole.HEALER:
        // Healers need flexibility — moderate state
        return 0.5 + (1 - Math.abs(e - 0.5)) * 0.3;

      default:
        return 0.5;
    }
  }

  private countRoles(): Record<AgentRole, number> {
    const counts: Record<string, number> = {};
    for (const role of Object.values(AgentRole)) counts[role] = 0;
    for (const profile of this.profiles.values()) {
      counts[profile.role] = (counts[profile.role] ?? 0) + 1;
    }
    return counts as Record<AgentRole, number>;
  }
}
