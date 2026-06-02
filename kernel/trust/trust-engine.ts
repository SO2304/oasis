/**
 * OASIS Kernel — Trust Engine
 *
 * Heritage: LAP agent-sandbox.ts (trust multiplier, PII detection, citation validation)
 *
 * INNOVATION: Dynamic trust scoring with continuous Bayesian updates.
 * Instead of static trust levels, each agent has a continuous trust score
 * that evolves based on observed behavior:
 * - Successful operations increase trust
 * - Constraint violations decrease trust
 * - Trust decay over inactivity (use-it-or-lose-it)
 * - Floor and ceiling per trust class (EXPERIMENTAL can never reach NATIVE trust)
 *
 * Physical trust is separate from cyber trust — an agent can be
 * cyber-NATIVE but physical-SUPERVISED.
 */

import {
  type AgentId,
  type CyberTrustLevel,
  type PhysicalTrustLevel,
  CyberTrustLevel as CTL,
  PhysicalTrustLevel as PTL,
  TRUST_MULTIPLIERS,
  monotonicNow,
  nsToMs,
} from '../types.js';
import { type KillSwitch, getKillSwitch } from '../kill-switch.js';

interface TrustProfile {
  readonly agentId: AgentId;
  cyberClass: CyberTrustLevel;
  physicalClass: PhysicalTrustLevel | null;
  /** Continuous score 0.0 - 1.0, bounded by class ceiling */
  cyberScore: number;
  physicalScore: number;
  /** Bayesian counters */
  successCount: number;
  failureCount: number;
  lastActivityAt: bigint;
  violations: TrustViolation[];
}

interface TrustViolation {
  readonly timestamp: bigint;
  readonly type: 'FORCE_EXCEEDED' | 'GEOFENCE_BREACH' | 'TRUST_VIOLATION' | 'DEADLINE_MISS' | 'PII_LEAK';
  readonly detail: string;
  readonly penaltyApplied: number;
}

/** Trust class boundaries — score can never exceed ceiling for its class */
const TRUST_CEILINGS: Record<CyberTrustLevel, number> = {
  NATIVE: 1.0,
  VERIFIED: 0.95,
  ENTERPRISE: 0.85,
  EXPERIMENTAL: 0.7,
};

const TRUST_FLOORS: Record<CyberTrustLevel, number> = {
  NATIVE: 0.8,
  VERIFIED: 0.5,
  ENTERPRISE: 0.3,
  EXPERIMENTAL: 0.1,
};

const PHYSICAL_CEILINGS: Record<PhysicalTrustLevel, number> = {
  SIM_ONLY: 0.5,
  SUPERVISED: 0.7,
  AUTONOMOUS_INDOOR: 0.85,
  AUTONOMOUS_OUTDOOR: 0.95,
  SAFETY_CRITICAL: 1.0,
};

/** Trust decay: lose 0.01 per hour of inactivity */
const DECAY_RATE_PER_HOUR = 0.01;

export class TrustEngine {
  private readonly profiles = new Map<AgentId, TrustProfile>();

  constructor(private readonly killSwitch: KillSwitch = getKillSwitch()) {}

  /** Register an agent with initial trust class */
  register(
    agentId: AgentId,
    cyberClass: CyberTrustLevel,
    physicalClass?: PhysicalTrustLevel,
  ): void {
    const cyberScore = TRUST_MULTIPLIERS[cyberClass];
    const physicalScore = physicalClass ? TRUST_MULTIPLIERS[physicalClass] : 0;

    this.profiles.set(agentId, {
      agentId,
      cyberClass,
      physicalClass: physicalClass ?? null,
      cyberScore,
      physicalScore,
      successCount: 0,
      failureCount: 0,
      lastActivityAt: monotonicNow(),
      violations: [],
    });
  }

  /** Record a successful operation — increases trust score */
  recordSuccess(agentId: AgentId): void {
    const profile = this.getProfile(agentId);
    profile.successCount++;
    profile.lastActivityAt = monotonicNow();

    // Bayesian update: trust = successes / (successes + failures)
    // Bounded by class ceiling
    const raw = profile.successCount / (profile.successCount + profile.failureCount + 1);
    const ceiling = TRUST_CEILINGS[profile.cyberClass];
    const floor = TRUST_FLOORS[profile.cyberClass];
    profile.cyberScore = Math.max(floor, Math.min(ceiling, raw + floor));
  }

  /** Record a violation — decreases trust, may trigger panic */
  recordViolation(
    agentId: AgentId,
    type: TrustViolation['type'],
    detail: string,
  ): void {
    const profile = this.getProfile(agentId);
    profile.failureCount++;
    profile.lastActivityAt = monotonicNow();

    // Penalty scales with severity
    const penalty = type === 'GEOFENCE_BREACH' || type === 'FORCE_EXCEEDED' ? 0.3 : 0.1;

    const violation: TrustViolation = {
      timestamp: monotonicNow(),
      type,
      detail,
      penaltyApplied: penalty,
    };
    profile.violations.push(violation);

    const floor = TRUST_FLOORS[profile.cyberClass];
    profile.cyberScore = Math.max(floor, profile.cyberScore - penalty);

    if (profile.physicalClass) {
      profile.physicalScore = Math.max(0.1, profile.physicalScore - penalty);
    }

    // 3 physical violations = kill switch
    const physicalViolations = profile.violations.filter(
      v => v.type === 'GEOFENCE_BREACH' || v.type === 'FORCE_EXCEEDED',
    );
    if (physicalViolations.length >= 3) {
      this.killSwitch.panic(
        'TRUST_VIOLATION',
        agentId,
        `Agent ${agentId} has ${physicalViolations.length} physical violations — trust revoked`,
      );
    }
  }

  /** Get effective trust multiplier (with time decay) */
  getEffectiveTrust(agentId: AgentId): { cyber: number; physical: number } {
    const profile = this.getProfile(agentId);
    const now = monotonicNow();
    const inactiveHours = nsToMs(now - profile.lastActivityAt) / 3_600_000;

    const decay = Math.min(inactiveHours * DECAY_RATE_PER_HOUR, 0.2);
    const floor = TRUST_FLOORS[profile.cyberClass];

    return {
      cyber: Math.max(floor, profile.cyberScore - decay),
      physical: profile.physicalClass
        ? Math.max(0.1, profile.physicalScore - decay)
        : 0,
    };
  }

  /** Check if an agent meets minimum trust for an operation */
  meetsMinimumTrust(agentId: AgentId, minCyber: number, minPhysical?: number): boolean {
    const trust = this.getEffectiveTrust(agentId);
    if (trust.cyber < minCyber) return false;
    if (minPhysical !== undefined && trust.physical < minPhysical) return false;
    return true;
  }

  /** Get full profile for inspection */
  getProfile(agentId: AgentId): TrustProfile {
    const profile = this.profiles.get(agentId);
    if (!profile) throw new Error(`No trust profile for agent ${agentId}`);
    return profile;
  }

  /** Get all registered agent IDs */
  getRegisteredAgents(): AgentId[] {
    return [...this.profiles.keys()];
  }
}
