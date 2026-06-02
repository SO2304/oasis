/**
 * OASIS Kernel — Budget-Bounded Tick (R18)
 *
 * PROBLEM: At 1kHz, each tick has exactly 1ms budget.
 * If correlation discovery or field sampling takes too long,
 * the control loop misses its deadline.
 *
 * SOLUTION: Anytime Algorithm wrapper.
 * Every computation runs inside a budget. When time runs out,
 * the computation returns whatever partial result it has.
 * Better to have 80% of the answer on time than 100% late.
 *
 * INNOVATION: Priority-ordered work items.
 * The budget executor processes work items in priority order.
 * HARD_RT items always complete. SOFT_RT items complete if budget
 * remains. BEST_EFFORT items get whatever's left.
 *
 * R18: Jitter must be < 5%. We measure and enforce this.
 */

import { monotonicNow, nsToMs, msToNs } from '../types.js';

// ─── Types ──────────────────────────────────────────────────────

export interface WorkItem<T> {
  readonly id: string;
  readonly priority: 0 | 1 | 2; // 0 = HARD_RT, 1 = SOFT_RT, 2 = BEST_EFFORT
  readonly fn: () => T;
}

export interface BudgetResult<T> {
  /** Completed work items and their results */
  readonly completed: Array<{ id: string; result: T }>;
  /** Work items that were skipped due to budget exhaustion */
  readonly skipped: string[];
  /** Actual tick duration in microseconds */
  readonly durationUs: number;
  /** Budget utilization [0, 1] */
  readonly utilization: number;
  /** Whether the tick completed within budget */
  readonly withinBudget: boolean;
}

export interface JitterStats {
  /** Average tick duration in microseconds */
  readonly avgDurationUs: number;
  /** Jitter as percentage of average */
  readonly jitterPercent: number;
  /** Min tick duration */
  readonly minUs: number;
  /** Max tick duration */
  readonly maxUs: number;
  /** Number of ticks measured */
  readonly sampleCount: number;
  /** Number of budget overruns */
  readonly overruns: number;
  /** R18 compliant? (jitter < 5%) */
  readonly r18Compliant: boolean;
}

// ─── Budget Tick Executor ───────────────────────────────────────

export class BudgetTick {
  /** Budget per tick in nanoseconds */
  private readonly budgetNs: bigint;
  /** History of tick durations for jitter calculation */
  private readonly durations: number[] = [];
  private readonly maxHistory: number;
  private overrunCount = 0;

  /**
   * @param budgetMs Time budget per tick in milliseconds (e.g., 1.0 for 1kHz)
   * @param maxHistory How many tick durations to keep for jitter stats
   */
  constructor(budgetMs: number, maxHistory = 1000) {
    this.budgetNs = msToNs(budgetMs);
    this.maxHistory = maxHistory;
  }

  /**
   * Execute work items within the time budget.
   *
   * Priority 0 (HARD_RT) items ALWAYS execute regardless of budget.
   * Priority 1 (SOFT_RT) items execute if budget remains.
   * Priority 2 (BEST_EFFORT) items get whatever time is left.
   *
   * R18: If total duration exceeds budget, it's recorded as an overrun.
   */
  execute<T>(items: WorkItem<T>[]): BudgetResult<T> {
    const tickStart = monotonicNow();
    const deadline = tickStart + this.budgetNs;

    // Sort by priority (stable sort preserves insertion order within same priority)
    const sorted = [...items].sort((a, b) => a.priority - b.priority);

    const completed: Array<{ id: string; result: T }> = [];
    const skipped: string[] = [];

    for (const item of sorted) {
      const now = monotonicNow();

      // HARD_RT always executes, others check budget
      if (item.priority > 0 && now >= deadline) {
        skipped.push(item.id);
        continue;
      }

      try {
        const result = item.fn();
        completed.push({ id: item.id, result });
      } catch {
        // Failed items are still "completed" (attempted)
        skipped.push(item.id);
      }
    }

    const tickEnd = monotonicNow();
    const durationNs = Number(tickEnd - tickStart);
    const durationUs = durationNs / 1000;

    // Record duration for jitter stats
    this.durations.push(durationUs);
    if (this.durations.length > this.maxHistory) this.durations.shift();

    const withinBudget = tickEnd <= deadline;
    if (!withinBudget) this.overrunCount++;

    return {
      completed,
      skipped,
      durationUs,
      utilization: durationNs / Number(this.budgetNs),
      withinBudget,
    };
  }

  /**
   * Get jitter statistics.
   * R18: Jitter must be < 5% of average tick duration.
   */
  getJitterStats(): JitterStats {
    if (this.durations.length < 2) {
      return {
        avgDurationUs: this.durations[0] ?? 0,
        jitterPercent: 0,
        minUs: this.durations[0] ?? 0,
        maxUs: this.durations[0] ?? 0,
        sampleCount: this.durations.length,
        overruns: this.overrunCount,
        r18Compliant: true,
      };
    }

    let sum = 0;
    let min = Infinity;
    let max = -Infinity;

    for (const d of this.durations) {
      sum += d;
      if (d < min) min = d;
      if (d > max) max = d;
    }

    const avg = sum / this.durations.length;

    // Jitter = (max - min) / avg as percentage
    // More precisely: standard deviation / mean (coefficient of variation)
    let variance = 0;
    for (const d of this.durations) {
      variance += (d - avg) ** 2;
    }
    variance /= this.durations.length;
    const stddev = Math.sqrt(variance);
    const jitterPercent = avg > 0 ? (stddev / avg) * 100 : 0;

    return {
      avgDurationUs: avg,
      jitterPercent,
      minUs: min,
      maxUs: max,
      sampleCount: this.durations.length,
      overruns: this.overrunCount,
      r18Compliant: jitterPercent < 5,
    };
  }

  /** Reset statistics */
  resetStats(): void {
    this.durations.length = 0;
    this.overrunCount = 0;
  }
}
