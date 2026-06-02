/**
 * OASIS Sovereign — Watchdog Timer
 *
 * If the kernel tick doesn't complete within the deadline,
 * SOMETHING IS WRONG. The watchdog triggers an automatic
 * kill switch activation.
 *
 * This catches:
 * - Infinite loops in agent code
 * - Memory leaks causing GC pauses
 * - Deadlocks between subsystems
 * - CPU exhaustion from rogue agents
 *
 * The watchdog is the LAST line of defense.
 * It runs OUTSIDE the main tick loop.
 * If the tick hangs, the watchdog fires.
 *
 * METHOD: The main loop must call `feed()` every tick.
 * If `feed()` isn't called within `deadlineMs`, the watchdog
 * calls the kill switch callback.
 */

export interface WatchdogConfig {
  /** Maximum time between feeds in milliseconds */
  deadlineMs: number;
  /** Number of consecutive misses before triggering */
  missThreshold: number;
}

export const DEFAULT_WATCHDOG: WatchdogConfig = {
  deadlineMs: 100,   // 100ms — if tick takes > 100ms, something is wrong
  missThreshold: 3,  // 3 consecutive misses → kill
};

export class Watchdog {
  private readonly config: WatchdogConfig;
  private readonly onTrigger: (reason: string) => void;
  private lastFeedMs: number;
  private consecutiveMisses: number;
  private triggered: boolean;
  private feedCount: number;

  constructor(config: WatchdogConfig, onTrigger: (reason: string) => void) {
    this.config = config;
    this.onTrigger = onTrigger;
    this.lastFeedMs = Date.now();
    this.consecutiveMisses = 0;
    this.triggered = false;
    this.feedCount = 0;
  }

  /** Call this every tick to reset the watchdog timer. */
  feed(): void {
    const now = Date.now();
    const elapsed = now - this.lastFeedMs;

    if (elapsed > this.config.deadlineMs) {
      this.consecutiveMisses++;

      if (this.consecutiveMisses >= this.config.missThreshold && !this.triggered) {
        this.triggered = true;
        this.onTrigger(
          `Watchdog: ${this.consecutiveMisses} consecutive misses ` +
          `(last feed ${elapsed}ms ago, deadline ${this.config.deadlineMs}ms)`,
        );
      }
    } else {
      this.consecutiveMisses = 0;
    }

    this.lastFeedMs = now;
    this.feedCount++;
  }

  /** Check if the watchdog has been triggered. */
  isTriggered(): boolean {
    return this.triggered;
  }

  /** Get the number of consecutive misses. */
  getConsecutiveMisses(): number {
    return this.consecutiveMisses;
  }

  /** Total feeds received. */
  getFeedCount(): number {
    return this.feedCount;
  }

  /** Force check without feeding (called by external monitor). */
  check(): { healthy: boolean; elapsedMs: number; misses: number } {
    const elapsed = Date.now() - this.lastFeedMs;
    return {
      healthy: elapsed <= this.config.deadlineMs,
      elapsedMs: elapsed,
      misses: this.consecutiveMisses,
    };
  }
}
