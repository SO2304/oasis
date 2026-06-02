import { describe, it, expect } from 'vitest';
import { RTScheduler } from '../rt-scheduler.js';
import { __resetKillSwitchForTesting } from '../../kill-switch.js';
import { agentId, SchedulePriority } from '../../types.js';

describe('RTScheduler — Dedicated', () => {
  it('HARD_RT before SOFT_RT before BEST_EFFORT', async () => {
    const ks = __resetKillSwitchForTesting();
    const sched = new RTScheduler(ks);
    const order: number[] = [];

    sched.enqueue('be', agentId('a'), SchedulePriority.BEST_EFFORT, null, () => { order.push(3); });
    sched.enqueue('srt', agentId('a'), SchedulePriority.SOFT_RT, 1000, () => { order.push(2); });
    sched.enqueue('hrt', agentId('a'), SchedulePriority.HARD_RT, 1000, () => { order.push(1); });

    await sched.tick();
    expect(order).toEqual([1, 2, 3]);
  });

  it('EDF within same priority', async () => {
    const ks = __resetKillSwitchForTesting();
    const sched = new RTScheduler(ks);
    const order: string[] = [];

    sched.enqueue('late', agentId('a'), SchedulePriority.HARD_RT, 500, () => { order.push('late'); });
    sched.enqueue('early', agentId('a'), SchedulePriority.HARD_RT, 100, () => { order.push('early'); });

    await sched.tick();
    expect(order).toEqual(['early', 'late']);
  });

  it('records completed tasks', async () => {
    const ks = __resetKillSwitchForTesting();
    const sched = new RTScheduler(ks);
    sched.enqueue('t1', agentId('a'), SchedulePriority.BEST_EFFORT, null, () => {});
    await sched.tick();

    expect(sched.getCompleted().length).toBe(1);
    expect(sched.getCompleted()[0]!.completedAt).not.toBeNull();
  });
});
