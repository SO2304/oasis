/**
 * OASIS Kernel — Kill Switch Tests
 *
 * VALIDATION: Proves R12 — kill switch propagation < 10ms
 * with 1000 subscribers across all subsystems.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { KillSwitch, KillSwitchEngagedException, __resetKillSwitchForTesting } from '../kill-switch.js';
import { nsToMs } from '../types.js';

describe('KillSwitch (R12)', () => {
  let ks: KillSwitch;

  beforeEach(() => {
    ks = __resetKillSwitchForTesting();
  });

  it('should not be triggered initially', () => {
    expect(ks.isTriggered()).toBe(false);
    expect(ks.getPanicEvent()).toBeNull();
  });

  it('should trigger and propagate to all handlers synchronously', () => {
    const calls: number[] = [];
    ks.onPanic(() => calls.push(1));
    ks.onPanic(() => calls.push(2));
    ks.onPanic(() => calls.push(3));

    ks.panic('MANUAL', 'KERNEL', 'Test panic');

    expect(ks.isTriggered()).toBe(true);
    expect(calls).toEqual([1, 2, 3]);
  });

  it('should be idempotent — second panic returns first event', () => {
    const event1 = ks.panic('MANUAL', 'KERNEL', 'First');
    const event2 = ks.panic('FORCE_EXCEEDED', 'KERNEL', 'Second');

    expect(event1.reason).toBe('MANUAL');
    expect(event2.reason).toBe('MANUAL'); // Same event
    expect(event1).toBe(event2);
  });

  it('should survive handler crashes without blocking other handlers', () => {
    const calls: number[] = [];
    ks.onPanic(() => calls.push(1));
    ks.onPanic(() => { throw new Error('handler crash'); });
    ks.onPanic(() => calls.push(3));

    ks.panic('MANUAL', 'KERNEL', 'Crash test');

    expect(calls).toEqual([1, 3]); // Handler 2 crashed, 3 still ran
  });

  it('assertAlive() should throw when triggered', () => {
    ks.panic('MANUAL', 'KERNEL', 'Test');

    expect(() => ks.assertAlive()).toThrow(KillSwitchEngagedException);
  });

  it('guard() should check before and after execution', () => {
    const result = ks.guard(() => 42);
    expect(result).toBe(42);

    ks.panic('MANUAL', 'KERNEL', 'Test');
    expect(() => ks.guard(() => 42)).toThrow(KillSwitchEngagedException);
  });

  it('late subscribers should receive the panic event immediately', () => {
    ks.panic('MANUAL', 'KERNEL', 'Already panicked');

    let received = false;
    ks.onPanic(() => { received = true; });

    expect(received).toBe(true);
  });

  // ─── THE CRITICAL TEST: < 10ms propagation ───────────────────

  it('should propagate to 1000 handlers in under 10ms', () => {
    const HANDLER_COUNT = 1000;
    let handlersFired = 0;

    // Register 1000 handlers (simulating a large system)
    for (let i = 0; i < HANDLER_COUNT; i++) {
      ks.onPanic(() => {
        handlersFired++;
        // Simulate minimal work per handler (state flag flip)
        const _ = Math.sqrt(i);
      });
    }

    // Trigger and measure
    ks.panic('MANUAL', 'KERNEL', 'Propagation benchmark');

    const propagationNs = ks.getPropagationTimeNs();
    expect(propagationNs).not.toBeNull();

    const propagationMs = nsToMs(propagationNs!);

    // ALL handlers must have fired
    expect(handlersFired).toBe(HANDLER_COUNT);

    // Propagation MUST be under 10ms (R12)
    expect(propagationMs).toBeLessThan(10);

    console.log(
      `[R12 VALIDATED] Kill switch propagation to ${HANDLER_COUNT} handlers: ${propagationMs}ms` +
      ` (${Number(propagationNs)} ns)`,
    );
  });

  it('should propagate to 1000 handlers with real subsystem work in under 10ms', () => {
    const HANDLER_COUNT = 1000;
    const stoppedDrivers: string[] = [];
    const killedAgents: string[] = [];

    // Simulate real subsystems registering handlers
    for (let i = 0; i < HANDLER_COUNT; i++) {
      if (i % 3 === 0) {
        // HAL driver emergency stop
        ks.onPanic(() => {
          stoppedDrivers.push(`driver-${i}`);
        });
      } else if (i % 3 === 1) {
        // Agent kill
        ks.onPanic(() => {
          killedAgents.push(`agent-${i}`);
        });
      } else {
        // Logger/telemetry
        ks.onPanic(() => {
          // Write audit log entry (simulated)
          const _log = `PANIC at ${Date.now()}`;
        });
      }
    }

    ks.panic('FORCE_EXCEEDED', 'KERNEL', 'Motor force exceeded 500N limit');

    const propagationMs = nsToMs(ks.getPropagationTimeNs()!);

    expect(propagationMs).toBeLessThan(10);
    expect(stoppedDrivers.length).toBeGreaterThan(0);
    expect(killedAgents.length).toBeGreaterThan(0);

    console.log(
      `[R12 VALIDATED — REALISTIC] Propagation: ${propagationMs}ms | ` +
      `Drivers stopped: ${stoppedDrivers.length} | Agents killed: ${killedAgents.length}`,
    );
  });
});
