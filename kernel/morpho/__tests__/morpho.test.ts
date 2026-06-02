/**
 * MorphogenesisEngine — dedicated unit tests.
 */
import { describe, it, expect } from 'vitest';
import { MorphogenesisEngine, AgentRole } from '../morphogenesis.js';
import { createHyperState, evolveState, type HyperState } from '../../physics/hyper-state.js';
import { agentId, AgentState } from '../../types.js';
import { zeros } from '../../physics/vector-math.js';

const DIM = 32;

describe('MorphogenesisEngine', () => {
  it('should start as STEM', () => {
    const m = new MorphogenesisEngine(DIM);
    m.register(agentId('cell'));
    expect(m.getRole(agentId('cell'))).toBe(AgentRole.STEM);
  });

  it('should differentiate toward SENTINEL under threat', () => {
    const m = new MorphogenesisEngine(DIM);
    for (let i = 0; i < 5; i++) m.register(agentId(`c${i}`));
    const states = new Map<AgentId, HyperState>();
    for (let i = 0; i < 5; i++) {
      states.set(agentId(`c${i}`), createHyperState(agentId(`c${i}`), AgentState.RUNNING, DIM));
    }
    m.differentiate(states, 0.9, 0, 0, false);
    expect(m.getAgentsByRole(AgentRole.SENTINEL).length).toBeGreaterThan(0);
  });

  it('should differentiate toward SCOUT in unknown territory', () => {
    const m = new MorphogenesisEngine(DIM);
    for (let i = 0; i < 5; i++) m.register(agentId(`c${i}`));
    const states = new Map<AgentId, HyperState>();
    for (let i = 0; i < 5; i++) {
      states.set(agentId(`c${i}`), { ...createHyperState(agentId(`c${i}`), AgentState.RUNNING, DIM), entropy: 0.5 });
    }
    m.differentiate(states, 0, 0.9, 0, false);
    expect(m.getAgentsByRole(AgentRole.SCOUT).length).toBeGreaterThan(0);
  });

  it('should differentiate toward NAVIGATOR with goal', () => {
    const m = new MorphogenesisEngine(DIM);
    for (let i = 0; i < 5; i++) m.register(agentId(`c${i}`));
    const states = new Map<AgentId, HyperState>();
    const force = zeros(DIM); force[10] = 2;
    for (let i = 0; i < 5; i++) {
      states.set(agentId(`c${i}`), evolveState(createHyperState(agentId(`c${i}`), AgentState.RUNNING, DIM), force, 0.5, 0));
    }
    m.differentiate(states, 0, 0, 0, true);
    expect(m.getAgentsByRole(AgentRole.NAVIGATOR).length).toBeGreaterThan(0);
  });

  it('should track events with from/to roles', () => {
    const m = new MorphogenesisEngine(DIM);
    m.register(agentId('cell'));
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('cell'), createHyperState(agentId('cell'), AgentState.RUNNING, DIM));
    m.differentiate(states, 0.5, 0.5, 1, true);
    const events = m.getEvents();
    expect(events.length).toBeGreaterThan(0);
    expect(events[0]!.fromRole).toBe(AgentRole.STEM);
  });

  it('should reduce commitment for poor performers', () => {
    const m = new MorphogenesisEngine(DIM, 0.1);
    m.register(agentId('bad'));
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('bad'), createHyperState(agentId('bad'), AgentState.RUNNING, DIM));
    m.differentiate(states, 0.5, 0, 0, true);
    for (let i = 0; i < 5; i++) m.differentiate(states, 0.5, 0, 0, true);
    const before = m.getProfile(agentId('bad'))!.commitment;
    m.reportPerformance(agentId('bad'), 0.05);
    expect(m.getProfile(agentId('bad'))!.commitment).toBeLessThan(before);
  });
});
