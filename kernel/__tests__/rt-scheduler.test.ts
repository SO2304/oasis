/**
 * OASIS Kernel — RT Scheduler Tests
 *
 * Tests HARD_RT/SOFT_RT/BEST_EFFORT priority ordering,
 * deadline enforcement, and kill switch integration (R11).
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { RTScheduler } from '../scheduler/rt-scheduler.js';
import { __resetKillSwitchForTesting } from '../kill-switch.js';
import { agentId, SchedulePriority } from '../types.js';

describe('RTScheduler', () => {
  let ks: ReturnType<typeof __resetKillSwitchForTesting>;
  let scheduler: RTScheduler;

  beforeEach(() => {
    ks = __resetKillSwitchForTesting();
    scheduler = new RTScheduler(ks);
  });

  it('should execute HARD_RT tasks before SOFT_RT and BEST_EFFORT', async () => {
    const order: string[] = [];

    scheduler.enqueue('be-1', agentId('a1'), SchedulePriority.BEST_EFFORT, null, () => {
      order.push('BEST_EFFORT');
    });
    scheduler.enqueue('srt-1', agentId('a2'), SchedulePriority.SOFT_RT, 1000, () => {
      order.push('SOFT_RT');
    });
    scheduler.enqueue('hrt-1', agentId('a3'), SchedulePriority.HARD_RT, 1000, () => {
      order.push('HARD_RT');
    });

    await scheduler.tick();

    expect(order).toEqual(['HARD_RT', 'SOFT_RT', 'BEST_EFFORT']);
  });

  it('should sort HARD_RT tasks by earliest deadline first (EDF)', async () => {
    const order: string[] = [];

    scheduler.enqueue('hrt-late', agentId('a1'), SchedulePriority.HARD_RT, 500, () => {
      order.push('late');
    });
    scheduler.enqueue('hrt-early', agentId('a2'), SchedulePriority.HARD_RT, 100, () => {
      order.push('early');
    });
    scheduler.enqueue('hrt-mid', agentId('a3'), SchedulePriority.HARD_RT, 300, () => {
      order.push('mid');
    });

    await scheduler.tick();

    expect(order).toEqual(['early', 'mid', 'late']);
  });

  it('should trigger panic on HARD_RT deadline miss (R11)', async () => {
    // Enqueue a task with 0ms deadline — already expired
    scheduler.enqueue('hrt-expired', agentId('a1'), SchedulePriority.HARD_RT, 0, () => {
      // This should never execute — deadline already passed
    });

    // Need a small delay so the deadline is actually in the past
    await new Promise(resolve => setTimeout(resolve, 1));

    const result = await scheduler.tick();

    expect(result.panicTriggered).toBe(true);
    expect(result.deadlineMisses).toBeGreaterThan(0);
    expect(ks.isTriggered()).toBe(true);
    expect(ks.getPanicEvent()?.reason).toBe('DEADLINE_MISS');
  });

  it('should NOT panic on SOFT_RT deadline miss', async () => {
    scheduler.enqueue('srt-expired', agentId('a1'), SchedulePriority.SOFT_RT, 0, () => {
      // Runs despite missed deadline
    });

    await new Promise(resolve => setTimeout(resolve, 1));
    const result = await scheduler.tick();

    expect(result.deadlineMisses).toBeGreaterThan(0);
    expect(result.panicTriggered).toBe(false);
    expect(ks.isTriggered()).toBe(false);
  });

  it('should track queue depths', () => {
    scheduler.enqueue('1', agentId('a1'), SchedulePriority.HARD_RT, 100, () => {});
    scheduler.enqueue('2', agentId('a1'), SchedulePriority.HARD_RT, 200, () => {});
    scheduler.enqueue('3', agentId('a2'), SchedulePriority.SOFT_RT, 100, () => {});
    scheduler.enqueue('4', agentId('a3'), SchedulePriority.BEST_EFFORT, null, () => {});
    scheduler.enqueue('5', agentId('a3'), SchedulePriority.BEST_EFFORT, null, () => {});

    const depths = scheduler.getQueueDepths();
    expect(depths).toEqual({ hardRT: 2, softRT: 1, bestEffort: 2 });
  });

  it('should reject enqueue after kill switch', () => {
    ks.panic('MANUAL', 'KERNEL', 'Test');

    expect(() =>
      scheduler.enqueue('x', agentId('a1'), SchedulePriority.HARD_RT, 100, () => {}),
    ).toThrow('Kill switch');
  });

  it('should handle async tasks correctly', async () => {
    let resolved = false;

    scheduler.enqueue('async-1', agentId('a1'), SchedulePriority.HARD_RT, 5000, async () => {
      await new Promise(r => setTimeout(r, 1));
      resolved = true;
    });

    await scheduler.tick();
    expect(resolved).toBe(true);
  });

  it('should record completed tasks with timestamps', async () => {
    scheduler.enqueue('t1', agentId('a1'), SchedulePriority.BEST_EFFORT, null, () => {});
    scheduler.enqueue('t2', agentId('a2'), SchedulePriority.BEST_EFFORT, null, () => {});

    await scheduler.tick();

    const completed = scheduler.getCompleted();
    expect(completed).toHaveLength(2);
    expect(completed[0]!.completedAt).not.toBeNull();
    expect(completed[1]!.completedAt).not.toBeNull();
  });
});
