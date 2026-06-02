/**
 * OASIS — GHOST-to-CORE Protocol
 *
 * THE PROBLEM: An AI planner says "move the arm to position (x,y,z)".
 * But a robot arm has force limits, joint constraints, and a human
 * standing nearby. The AI doesn't know any of this.
 *
 * THE SOLUTION: The AI doesn't COMMAND. It INTENDS.
 *
 * The GHOST emits an INTENTION — a high-level goal in semantic space.
 * OASIS translates the intention into a TENSION in the latent field.
 * The CORE (robot) perceives the tension and DECIDES how to act,
 * subject to its own safety constraints.
 *
 * If the intention is safe → the robot follows it.
 * If the intention violates safety → the robot refuses.
 * If the intention is ambiguous → entropy rises (R14) → robot holds.
 *
 * The GHOST gets FEEDBACK: not raw sensor data, but SEMANTIC STATE.
 * "Robot reached 80% of goal. Obstacle at 30cm. Stress rising."
 * The GHOST can then adjust its intention based on this feedback.
 *
 * This is a DIALOGUE, not a command chain.
 *
 * LAYERS:
 * 1. INTENTION: GHOST → semantic goal (text/structured → latent vector)
 * 2. MEDIATION: OASIS validates, translates, injects into field
 * 3. PERCEPTION: CORE feels the tension, acts within constraints
 * 4. FEEDBACK: CORE state → semantic summary → GHOST
 */

import { createHash } from 'crypto';
import { monotonicNow, nsToMs } from '../types.js';

// ─── Intention (GHOST → OASIS) ──────────────────────────────────

export interface GhostIntention {
  /** Unique intention ID */
  readonly id: string;
  /** Source GHOST identifier */
  readonly ghostId: string;
  /** What the GHOST wants (semantic) */
  readonly goal: string;
  /** Structured parameters */
  readonly params: Record<string, number | string | boolean>;
  /** Priority [0-10] — how important is this intention? */
  priority: number;
  /** Deadline in seconds (0 = no deadline) */
  readonly deadlineS: number;
  /** Timestamp */
  readonly timestamp: bigint;
}

// ─── Mediation Result ───────────────────────────────────────────

export type MediationVerdict = 'ACCEPTED' | 'MODIFIED' | 'REFUSED' | 'QUEUED';

export interface MediationResult {
  readonly intentionId: string;
  readonly verdict: MediationVerdict;
  readonly reason: string;
  /** If modified: what changed */
  readonly modifications: string[];
  /** Safety rules that were checked */
  readonly rulesChecked: string[];
  /** Latent vector that was injected (null if refused) */
  readonly injected: boolean;
}

// ─── Feedback (CORE → GHOST) ────────────────────────────────────

export interface CoreFeedback {
  readonly intentionId: string;
  readonly progress: number; // [0, 1]
  readonly status: 'EXECUTING' | 'COMPLETED' | 'BLOCKED' | 'REFUSED' | 'FAILED';
  readonly obstacles: string[];
  readonly stress: number; // [0, 1]
  readonly entropy: number; // [0, 1]
  readonly safetyViolations: string[];
  readonly timestamp: bigint;
}

// ─── Safety Rules ───────────────────────────────────────────────

export interface SafetyRule {
  readonly id: string;
  readonly description: string;
  readonly check: (intention: GhostIntention) => { pass: boolean; reason: string };
}

// ─── GHOST-to-CORE Mediator ─────────────────────────────────────

export class GhostMediator {
  private readonly registeredGhosts = new Map<string, { name: string; trustLevel: number }>();
  private readonly safetyRules: SafetyRule[] = [];
  private readonly intentionLog: Array<{ intention: GhostIntention; result: MediationResult }> = [];
  private readonly feedbackQueue = new Map<string, CoreFeedback>();
  private rejectedCount = 0;
  private modifiedCount = 0;
  private acceptedCount = 0;

  /** Register a GHOST (non-physical AI) */
  registerGhost(ghostId: string, name: string, trustLevel: number): void {
    this.registeredGhosts.set(ghostId, { name, trustLevel });
  }

  /** Add a safety rule that ALL intentions must pass */
  addSafetyRule(rule: SafetyRule): void {
    this.safetyRules.push(rule);
  }

  /**
   * MEDIATE: Validate and translate an intention.
   *
   * 1. Verify GHOST is registered
   * 2. Check ALL safety rules
   * 3. If safe → inject into field (ACCEPTED)
   * 4. If partially unsafe → modify and inject (MODIFIED)
   * 5. If dangerous → refuse (REFUSED)
   */
  mediate(intention: GhostIntention): MediationResult {
    const rulesChecked: string[] = [];
    const modifications: string[] = [];
    let refused = false;
    let refuseReason = '';

    // 1. Identity check
    if (!this.registeredGhosts.has(intention.ghostId)) {
      this.rejectedCount++;
      const result: MediationResult = {
        intentionId: intention.id, verdict: 'REFUSED',
        reason: `GHOST ${intention.ghostId} not registered`,
        modifications: [], rulesChecked: ['IDENTITY'], injected: false,
      };
      this.intentionLog.push({ intention, result });
      return result;
    }

    // 2. Trust level check
    const ghost = this.registeredGhosts.get(intention.ghostId)!;
    if (ghost.trustLevel < 0.3 && intention.priority > 5) {
      modifications.push(`Priority reduced: ${intention.priority} → 5 (low trust)`);
      intention.priority = 5;
    }

    // 3. Safety rules
    for (const rule of this.safetyRules) {
      rulesChecked.push(rule.id);
      const { pass, reason } = rule.check(intention);

      if (!pass) {
        // Check if rule is hard (refuse) or soft (modify)
        if (rule.id.startsWith('HARD_')) {
          refused = true;
          refuseReason = `${rule.id}: ${reason}`;
          break;
        } else {
          modifications.push(`${rule.id}: ${reason}`);
        }
      }
    }

    if (refused) {
      this.rejectedCount++;
      const result: MediationResult = {
        intentionId: intention.id, verdict: 'REFUSED',
        reason: refuseReason, modifications, rulesChecked, injected: false,
      };
      this.intentionLog.push({ intention, result });
      return result;
    }

    const verdict: MediationVerdict = modifications.length > 0 ? 'MODIFIED' : 'ACCEPTED';
    if (verdict === 'MODIFIED') this.modifiedCount++;
    else this.acceptedCount++;

    const result: MediationResult = {
      intentionId: intention.id, verdict,
      reason: verdict === 'ACCEPTED' ? 'All safety checks passed' : 'Modified for safety',
      modifications, rulesChecked, injected: true,
    };
    this.intentionLog.push({ intention, result });
    return result;
  }

  /**
   * Record feedback from the CORE for a given intention.
   */
  recordFeedback(feedback: CoreFeedback): void {
    this.feedbackQueue.set(feedback.intentionId, feedback);
  }

  /**
   * Get feedback for a GHOST about its intention.
   * The GHOST doesn't see raw sensor data — it sees a SEMANTIC summary.
   */
  getFeedback(intentionId: string): CoreFeedback | null {
    return this.feedbackQueue.get(intentionId) ?? null;
  }

  /** Get mediation statistics */
  getStats(): { accepted: number; modified: number; refused: number; total: number } {
    return {
      accepted: this.acceptedCount,
      modified: this.modifiedCount,
      refused: this.rejectedCount,
      total: this.intentionLog.length,
    };
  }

  /** Get full intention log */
  getLog(): readonly Array<{ intention: GhostIntention; result: MediationResult }> {
    return this.intentionLog;
  }
}

// ─── Pre-built Safety Rules ─────────────────────────────────────

/** HARD: No intention can target a RED zone */
export const RULE_NO_RED_ZONE: SafetyRule = {
  id: 'HARD_NO_RED_ZONE',
  description: 'Intentions targeting RED zones are absolutely refused',
  check: (i) => {
    const zone = i.params['zone'] as string | undefined;
    if (zone && (zone.includes('RED') || zone.includes('forbidden'))) {
      return { pass: false, reason: `Target zone "${zone}" is RED` };
    }
    return { pass: true, reason: '' };
  },
};

/** HARD: No intention with force > 5N near humans */
export const RULE_HUMAN_FORCE_LIMIT: SafetyRule = {
  id: 'HARD_HUMAN_FORCE',
  description: 'Force limited to 5N when humans are nearby',
  check: (i) => {
    const force = i.params['force'] as number | undefined;
    const humansNearby = i.params['humansNearby'] as boolean | undefined;
    if (force && force > 5 && humansNearby) {
      return { pass: false, reason: `Force ${force}N with humans nearby` };
    }
    return { pass: true, reason: '' };
  },
};

/** SOFT: Speed limited in YELLOW zones */
export const RULE_YELLOW_SPEED: SafetyRule = {
  id: 'SOFT_YELLOW_SPEED',
  description: 'Speed reduced in YELLOW thermal zones',
  check: (i) => {
    const zone = i.params['zone'] as string | undefined;
    const speed = i.params['speed'] as number | undefined;
    if (zone?.includes('YELLOW') && speed && speed > 0.5) {
      return { pass: false, reason: `Speed ${speed}m/s too high for YELLOW zone, reduced to 0.5` };
    }
    return { pass: true, reason: '' };
  },
};

/** HARD: No intention during EVACUATE status */
export const RULE_NO_WORK_DURING_EVAC: SafetyRule = {
  id: 'HARD_EVACUATION',
  description: 'All work intentions refused during evacuation',
  check: (i) => {
    const opStatus = i.params['opStatus'] as string | undefined;
    if (opStatus === 'EVACUATE' && i.goal !== 'EVACUATE') {
      return { pass: false, reason: 'Platform is EVACUATING — only evacuation allowed' };
    }
    return { pass: true, reason: '' };
  },
};
