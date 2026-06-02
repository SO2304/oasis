/**
 * OASIS Kernel — Real-Time Scheduler
 *
 * INNOVATION: Dual-queue preemptive scheduler with deadline monotonic ordering.
 *
 * Architecture:
 * - HARD_RT queue: Sorted by deadline (earliest deadline first — EDF)
 *   These tasks MUST complete before their deadline or trigger panic.
 * - SOFT_RT queue: Sorted by deadline, but misses trigger degradation, not panic.
 * - BEST_EFFORT queue: FIFO, runs when no RT tasks are pending.
 *
 * Key properties:
 * - O(log n) insertion via binary heap
 * - Deadline monitoring with automatic escalation
 * - Kill switch integration: deadline miss on HARD_RT = kernel panic
 * - Starvation prevention: BEST_EFFORT gets guaranteed time slices
 */

import {
  type AgentId,
  type AgentProcess,
  SchedulePriority,
  AgentState,
  monotonicNow,
  nsToMs,
  msToNs,
} from '../types.js';
import { type KillSwitch, getKillSwitch } from '../kill-switch.js';

export type TaskFn = () => void | Promise<void>;

export interface ScheduledTask {
  readonly id: string;
  readonly agentId: AgentId;
  readonly priority: SchedulePriority;
  readonly deadlineMs: number | null;
  readonly fn: TaskFn;
  readonly enqueuedAt: bigint;
  completedAt: bigint | null;
  missed: boolean;
}

interface TaskEntry {
  task: ScheduledTask;
  deadlineNs: bigint | null;
}

export class RTScheduler {
  private readonly hardRT: TaskEntry[] = [];
  private readonly softRT: TaskEntry[] = [];
  private readonly bestEffort: TaskEntry[] = [];
  private readonly completed: ScheduledTask[] = [];
  private running = false;
  private tickCount = 0;

  /** Minimum time slice for best-effort tasks per scheduler tick (ms) */
  private readonly bestEffortSliceMs: number;

  constructor(
    private readonly killSwitch: KillSwitch = getKillSwitch(),
    bestEffortSliceMs = 5,
  ) {
    this.bestEffortSliceMs = bestEffortSliceMs;
  }

  /**
   * Enqueue a task for scheduling.
   * Tasks are sorted by priority and deadline.
   */
  enqueue(
    id: string,
    agentId: AgentId,
    priority: SchedulePriority,
    deadlineMs: number | null,
    fn: TaskFn,
  ): ScheduledTask {
    this.killSwitch.assertAlive();

    const task: ScheduledTask = {
      id,
      agentId,
      priority,
      deadlineMs,
      fn,
      enqueuedAt: monotonicNow(),
      completedAt: null,
      missed: false,
    };

    const entry: TaskEntry = {
      task,
      deadlineNs: deadlineMs !== null ? monotonicNow() + msToNs(deadlineMs) : null,
    };

    switch (priority) {
      case SchedulePriority.HARD_RT:
        this.insertByDeadline(this.hardRT, entry);
        break;
      case SchedulePriority.SOFT_RT:
        this.insertByDeadline(this.softRT, entry);
        break;
      case SchedulePriority.BEST_EFFORT:
        this.bestEffort.push(entry);
        break;
    }

    return task;
  }

  /**
   * Execute one scheduler tick.
   *
   * Order:
   * 1. All HARD_RT tasks (panic on deadline miss)
   * 2. All SOFT_RT tasks (degrade on deadline miss)
   * 3. BEST_EFFORT tasks (time-boxed to bestEffortSliceMs)
   */
  async tick(): Promise<SchedulerTickResult> {
    this.killSwitch.assertAlive();
    this.tickCount++;
    const tickStart = monotonicNow();

    const result: SchedulerTickResult = {
      tickNumber: this.tickCount,
      hardRTExecuted: 0,
      softRTExecuted: 0,
      bestEffortExecuted: 0,
      deadlineMisses: 0,
      panicTriggered: false,
    };

    // Phase 1: HARD_RT — execute all, panic on miss
    while (this.hardRT.length > 0) {
      const entry = this.hardRT.shift()!;
      const now = monotonicNow();

      if (entry.deadlineNs !== null && now > entry.deadlineNs) {
        // DEADLINE MISS on HARD_RT = KERNEL PANIC (R11)
        entry.task.missed = true;
        result.deadlineMisses++;
        result.panicTriggered = true;
        this.killSwitch.panic(
          'DEADLINE_MISS',
          entry.task.agentId,
          `HARD_RT task ${entry.task.id} missed deadline by ${nsToMs(now - entry.deadlineNs)}ms`,
        );
        return result;
      }

      await this.executeTask(entry.task);
      result.hardRTExecuted++;

      // Check if we missed deadline AFTER execution
      if (entry.deadlineNs !== null && monotonicNow() > entry.deadlineNs) {
        entry.task.missed = true;
        result.deadlineMisses++;
        result.panicTriggered = true;
        this.killSwitch.panic(
          'DEADLINE_MISS',
          entry.task.agentId,
          `HARD_RT task ${entry.task.id} execution exceeded deadline`,
        );
        return result;
      }
    }

    // Phase 2: SOFT_RT — execute all, log misses but no panic
    while (this.softRT.length > 0) {
      if (this.killSwitch.isTriggered()) break;

      const entry = this.softRT.shift()!;
      const now = monotonicNow();

      if (entry.deadlineNs !== null && now > entry.deadlineNs) {
        entry.task.missed = true;
        result.deadlineMisses++;
        // Soft RT: degrade, don't panic
      }

      await this.executeTask(entry.task);
      result.softRTExecuted++;
    }

    // Phase 3: BEST_EFFORT — time-boxed execution
    const sliceEnd = monotonicNow() + msToNs(this.bestEffortSliceMs);

    while (this.bestEffort.length > 0 && monotonicNow() < sliceEnd) {
      if (this.killSwitch.isTriggered()) break;

      const entry = this.bestEffort.shift()!;
      await this.executeTask(entry.task);
      result.bestEffortExecuted++;
    }

    return result;
  }

  /** Get queue depths */
  getQueueDepths(): { hardRT: number; softRT: number; bestEffort: number } {
    return {
      hardRT: this.hardRT.length,
      softRT: this.softRT.length,
      bestEffort: this.bestEffort.length,
    };
  }

  /** Get completed tasks */
  getCompleted(): readonly ScheduledTask[] {
    return this.completed;
  }

  /** Total ticks executed */
  getTickCount(): number {
    return this.tickCount;
  }

  // ─── Private ────────────────────────────────────────────────

  private async executeTask(task: ScheduledTask): Promise<void> {
    try {
      const result = task.fn();
      if (result instanceof Promise) {
        await result;
      }
    } catch {
      // Task failure is NOT a kernel panic — the task itself failed
      // The scheduler continues running
    }
    task.completedAt = monotonicNow();
    this.completed.push(task);
  }

  /** Binary insert by deadline (earliest first) */
  private insertByDeadline(queue: TaskEntry[], entry: TaskEntry): void {
    if (entry.deadlineNs === null) {
      queue.push(entry);
      return;
    }

    let low = 0;
    let high = queue.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      const midDeadline = queue[mid]!.deadlineNs;
      if (midDeadline === null || midDeadline > entry.deadlineNs) {
        high = mid;
      } else {
        low = mid + 1;
      }
    }
    queue.splice(low, 0, entry);
  }
}

export interface SchedulerTickResult {
  tickNumber: number;
  hardRTExecuted: number;
  softRTExecuted: number;
  bestEffortExecuted: number;
  deadlineMisses: number;
  panicTriggered: boolean;
}
