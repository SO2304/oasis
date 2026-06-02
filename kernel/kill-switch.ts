/**
 * OASIS Kernel — Kill Switch (R12)
 *
 * INNOVATION: Lock-free atomic kill switch using a shared flag.
 * Target: < 10ms propagation to ALL subsystems.
 *
 * Design:
 * - Single atomic boolean (no mutex, no lock)
 * - Subscribers are notified via synchronous callbacks (no event loop delay)
 * - The kill switch is ONE-WAY: once triggered, it cannot be reset
 *   without a full kernel restart. This is intentional (R12).
 * - Cascade: PANIC message is broadcast to all agents via IPC
 */

import {
  type PanicEvent,
  type PanicReason,
  type AgentId,
  type DriverId,
  monotonicNow,
} from './types.js';

export type PanicHandler = (event: PanicEvent) => void;

export class KillSwitch {
  private triggered = false;
  private panicEvent: PanicEvent | null = null;
  private readonly handlers: PanicHandler[] = [];
  private triggerTimestamp: bigint | null = null;
  private propagationCompleteTimestamp: bigint | null = null;

  /** Register a synchronous panic handler. Called in registration order. */
  onPanic(handler: PanicHandler): void {
    if (this.triggered && this.panicEvent) {
      // Late subscriber — fire immediately
      handler(this.panicEvent);
      return;
    }
    this.handlers.push(handler);
  }

  /** Is the kill switch currently engaged? */
  isTriggered(): boolean {
    return this.triggered;
  }

  /** Get the panic event if triggered */
  getPanicEvent(): PanicEvent | null {
    return this.panicEvent;
  }

  /** Propagation time in nanoseconds (null if not triggered) */
  getPropagationTimeNs(): bigint | null {
    if (!this.triggerTimestamp || !this.propagationCompleteTimestamp) return null;
    return this.propagationCompleteTimestamp - this.triggerTimestamp;
  }

  /**
   * TRIGGER THE KILL SWITCH
   *
   * This is irreversible. All handlers fire synchronously.
   * The entire call completes before returning — no async gaps.
   */
  panic(
    reason: PanicReason,
    source: AgentId | DriverId | 'KERNEL',
    detail: string,
  ): PanicEvent {
    // Idempotent — second panic is a no-op (first reason wins)
    if (this.triggered && this.panicEvent) {
      return this.panicEvent;
    }

    this.triggerTimestamp = monotonicNow();

    const event: PanicEvent = {
      reason,
      source,
      timestamp: this.triggerTimestamp,
      detail,
    };

    // Set flag BEFORE handlers — any code checking isTriggered()
    // during handler execution will see true
    this.triggered = true;
    this.panicEvent = event;

    // Fire all handlers synchronously — no event loop delay
    for (const handler of this.handlers) {
      try {
        handler(event);
      } catch {
        // Handler crash must NEVER prevent other handlers from firing
        // This is safety-critical code — swallow and continue
      }
    }

    this.propagationCompleteTimestamp = monotonicNow();

    return event;
  }

  /**
   * Assert the kill switch is NOT triggered.
   * Call this at the start of any safety-critical operation.
   * Throws if the system is in panic state.
   */
  assertAlive(): void {
    if (this.triggered) {
      throw new KillSwitchEngagedException(this.panicEvent!);
    }
  }

  /**
   * Create a guard that checks the kill switch before and after an operation.
   * Usage: const result = killSwitch.guard(() => dangerousOperation());
   */
  guard<T>(fn: () => T): T {
    this.assertAlive();
    const result = fn();
    this.assertAlive();
    return result;
  }
}

export class KillSwitchEngagedException extends Error {
  constructor(public readonly event: PanicEvent) {
    super(`[OASIS PANIC] Kill switch engaged: ${event.reason} — ${event.detail}`);
    this.name = 'KillSwitchEngagedException';
  }
}

// ─── Singleton ──────────────────────────────────────────────────

let globalKillSwitch: KillSwitch | null = null;

export function getKillSwitch(): KillSwitch {
  if (!globalKillSwitch) {
    globalKillSwitch = new KillSwitch();
  }
  return globalKillSwitch;
}

/** Reset for testing only — never call in production */
export function __resetKillSwitchForTesting(): KillSwitch {
  globalKillSwitch = new KillSwitch();
  return globalKillSwitch;
}
