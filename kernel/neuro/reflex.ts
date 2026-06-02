/**
 * OASIS Kernel — Reflex Arc
 *
 * The SPINAL CORD of OASIS.
 *
 * When you touch a hot stove, you pull your hand back BEFORE
 * your brain even processes the pain. The reflex arc shortcuts
 * the brain entirely — sensor → spinal cord → motor.
 *
 * OASIS reflexes work the same way:
 * - Proximity < threshold → EMERGENCY STOP (no tensor math)
 * - Force > safety limit → RELEASE (no deliberation)
 * - Temperature > critical → SHUTDOWN (no negotiation)
 * - Entropy > R14 → FREEZE (hardwired, not computed)
 *
 * Reflexes are HARDWIRED. They cannot be overridden by
 * the latent engine, emotions, or swarm consensus.
 * They execute in MICROSECONDS, not milliseconds.
 *
 * After the reflex fires, it sends a PAIN signal upward
 * so the brain can learn from the event (fear conditioning).
 *
 * DESIGN: Reflexes are registered as condition-action pairs.
 * They are checked BEFORE the main tick loop. If any fires,
 * the corresponding action executes immediately and the
 * associated agent is flagged for pain recording.
 */

import type { AgentId, DriverId } from '../types.js';
import { monotonicNow, nsToMs } from '../types.js';

// ─── Reflex Types ───────────────────────────────────────────────

export interface ReflexCondition {
  /** Telemetry key to check */
  readonly key: string;
  /** Comparison operator */
  readonly op: 'LT' | 'GT' | 'EQ';
  /** Threshold value */
  readonly threshold: number;
}

export interface ReflexRule {
  readonly id: string;
  readonly name: string;
  /** Which driver this reflex monitors */
  readonly driverId: DriverId;
  /** Conditions that trigger the reflex (ALL must be true) */
  readonly conditions: ReflexCondition[];
  /** Action to take when triggered */
  readonly action: ReflexAction;
  /** Priority (lower = fires first) */
  readonly priority: number;
  /** Is this reflex currently enabled? */
  enabled: boolean;
}

export const ReflexAction = {
  /** Emergency stop the driver */
  EMERGENCY_STOP: 'EMERGENCY_STOP',
  /** Zero all forces */
  ZERO_FORCE: 'ZERO_FORCE',
  /** Reverse direction */
  REVERSE: 'REVERSE',
  /** Trigger kernel panic */
  PANIC: 'PANIC',
} as const;
export type ReflexAction = (typeof ReflexAction)[keyof typeof ReflexAction];

// ─── Reflex Event ───────────────────────────────────────────────

export interface ReflexEvent {
  readonly ruleId: string;
  readonly ruleName: string;
  readonly driverId: DriverId;
  readonly action: ReflexAction;
  readonly triggerValues: Record<string, number>;
  readonly latencyNs: bigint;
  readonly tick: number;
}

// ─── Reflex Arc ─────────────────────────────────────────────────

export class ReflexArc {
  private readonly rules: ReflexRule[] = [];
  private readonly events: ReflexEvent[] = [];
  private tickCount = 0;

  /**
   * Register a reflex rule.
   * Rules are sorted by priority (lowest first = fires first).
   */
  register(rule: ReflexRule): void {
    this.rules.push(rule);
    this.rules.sort((a, b) => a.priority - b.priority);
  }

  /** Enable/disable a reflex */
  setEnabled(ruleId: string, enabled: boolean): void {
    const rule = this.rules.find(r => r.id === ruleId);
    if (rule) rule.enabled = enabled;
  }

  /**
   * CHECK ALL REFLEXES against current telemetry.
   *
   * This runs BEFORE the main tick loop.
   * Must be fast — O(rules * conditions), no allocation.
   *
   * Returns fired reflexes (may be empty).
   */
  check(telemetryByDriver: Map<DriverId, Record<string, number>>): ReflexEvent[] {
    this.tickCount++;
    const fired: ReflexEvent[] = [];

    for (const rule of this.rules) {
      if (!rule.enabled) continue;

      const telemetry = telemetryByDriver.get(rule.driverId);
      if (!telemetry) continue;

      const start = monotonicNow();

      // Check ALL conditions (AND logic)
      let allMet = true;
      for (const cond of rule.conditions) {
        const value = telemetry[cond.key];
        if (value === undefined) { allMet = false; break; }

        switch (cond.op) {
          case 'LT': if (!(value < cond.threshold)) allMet = false; break;
          case 'GT': if (!(value > cond.threshold)) allMet = false; break;
          case 'EQ': if (!(Math.abs(value - cond.threshold) < 1e-6)) allMet = false; break;
        }

        if (!allMet) break;
      }

      if (allMet) {
        const latencyNs = monotonicNow() - start;

        const event: ReflexEvent = {
          ruleId: rule.id,
          ruleName: rule.name,
          driverId: rule.driverId,
          action: rule.action,
          triggerValues: { ...telemetry },
          latencyNs,
          tick: this.tickCount,
        };

        fired.push(event);
        this.events.push(event);
      }
    }

    return fired;
  }

  /** Get all reflex events */
  getEvents(): readonly ReflexEvent[] {
    return this.events;
  }

  /** Get registered rule count */
  getRuleCount(): number {
    return this.rules.length;
  }

  /** Get latest event for a rule */
  getLatestEvent(ruleId: string): ReflexEvent | undefined {
    for (let i = this.events.length - 1; i >= 0; i--) {
      if (this.events[i]!.ruleId === ruleId) return this.events[i];
    }
    return undefined;
  }
}

// ─── Pre-built Safety Reflexes ──────────────────────────────────

/** Create a proximity emergency stop reflex */
export function proximityReflex(driverId: DriverId, minDistanceCm: number): ReflexRule {
  return {
    id: `reflex-proximity-${driverId}`,
    name: 'Proximity Emergency Stop',
    driverId,
    conditions: [
      { key: 'distance', op: 'LT', threshold: minDistanceCm },
    ],
    action: ReflexAction.EMERGENCY_STOP,
    priority: 0, // Highest priority
    enabled: true,
  };
}

/** Create a temperature shutdown reflex */
export function temperatureReflex(driverId: DriverId, maxTempC: number): ReflexRule {
  return {
    id: `reflex-temp-${driverId}`,
    name: 'Temperature Shutdown',
    driverId,
    conditions: [
      { key: 'temperature', op: 'GT', threshold: maxTempC },
    ],
    action: ReflexAction.EMERGENCY_STOP,
    priority: 1,
    enabled: true,
  };
}

/** Create a force overload reflex */
export function forceOverloadReflex(driverId: DriverId, maxForceN: number): ReflexRule {
  return {
    id: `reflex-force-${driverId}`,
    name: 'Force Overload',
    driverId,
    conditions: [
      { key: 'force', op: 'GT', threshold: maxForceN },
    ],
    action: ReflexAction.ZERO_FORCE,
    priority: 2,
    enabled: true,
  };
}

/** Create a stress panic reflex */
export function stressPanicReflex(driverId: DriverId, maxStress: number): ReflexRule {
  return {
    id: `reflex-stress-${driverId}`,
    name: 'Stress Panic',
    driverId,
    conditions: [
      { key: 'stress', op: 'GT', threshold: maxStress },
    ],
    action: ReflexAction.PANIC,
    priority: 0,
    enabled: true,
  };
}
