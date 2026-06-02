/**
 * OasisMonitor — dedicated unit tests.
 */
import { describe, it, expect } from 'vitest';
import { OasisMonitor } from '../oasis-monitor.js';
import { createHyperState } from '../../physics/hyper-state.js';
import { agentId, AgentState } from '../../types.js';

const DIM = 32;

describe('OasisMonitor', () => {
  it('should capture snapshots with correct agent data', () => {
    const mon = new OasisMonitor();
    const states = new Map();
    states.set(agentId('a'), createHyperState(agentId('a'), AgentState.RUNNING, DIM));

    const snap = mon.capture(1, states, new Map(), 42, 10, new Set(), false);
    expect(snap.agents.length).toBe(1);
    expect(snap.fieldMetrics.activeTensionCount).toBe(42);
    expect(snap.fieldMetrics.pheromoneCount).toBe(10);
    expect(snap.systemHealth.killSwitchEngaged).toBe(false);
  });

  it('should limit history to maxSnapshots', () => {
    const mon = new OasisMonitor(5);
    const states = new Map();
    states.set(agentId('a'), createHyperState(agentId('a'), AgentState.READY, DIM));

    for (let i = 0; i < 10; i++) mon.capture(i, states, new Map(), 0, 0, new Set(), false);
    expect(mon.getHistory().length).toBe(5);
  });

  it('should provide time-series metrics', () => {
    const mon = new OasisMonitor();
    const states = new Map();
    states.set(agentId('a'), createHyperState(agentId('a'), AgentState.RUNNING, DIM));

    for (let i = 0; i < 5; i++) mon.capture(i, states, new Map(), i * 10, 0, new Set(), false);
    const series = mon.getTimeSeries('tensionCount');
    expect(series).toEqual([0, 10, 20, 30, 40]);
  });

  it('should track R14 and R16 violations', () => {
    const mon = new OasisMonitor();
    mon.recordR14();
    mon.recordR14();
    mon.recordR16();

    const states = new Map();
    states.set(agentId('a'), createHyperState(agentId('a'), AgentState.RUNNING, DIM));
    const snap = mon.capture(1, states, new Map(), 0, 0, new Set(), false);
    expect(snap.systemHealth.r14Violations).toBe(2);
    expect(snap.systemHealth.r16Violations).toBe(1);
  });

  it('should mark quarantined agents', () => {
    const mon = new OasisMonitor();
    const states = new Map();
    states.set(agentId('good'), createHyperState(agentId('good'), AgentState.RUNNING, DIM));
    states.set(agentId('bad'), createHyperState(agentId('bad'), AgentState.RUNNING, DIM));

    const snap = mon.capture(1, states, new Map(), 0, 0, new Set([agentId('bad')]), false);
    expect(snap.agents.find(a => a.id === 'bad')!.quarantined).toBe(true);
    expect(snap.agents.find(a => a.id === 'good')!.quarantined).toBe(false);
  });
});
