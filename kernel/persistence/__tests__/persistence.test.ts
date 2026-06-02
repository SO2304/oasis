/**
 * CheckpointManager — dedicated unit tests.
 */
import { describe, it, expect } from 'vitest';
import { CheckpointManager } from '../checkpoint.js';
import { createHyperState, evolveState } from '../../physics/hyper-state.js';
import { agentId, tenantId, AgentState } from '../../types.js';
import { zeros } from '../../physics/vector-math.js';

const DIM = 32;

describe('CheckpointManager', () => {
  it('should save and restore agents with fidelity', () => {
    const mgr = new CheckpointManager();
    const agents = new Map();
    for (let i = 0; i < 3; i++) {
      const id = agentId(`a${i}`);
      const force = zeros(DIM); force[10] = i;
      agents.set(id, evolveState(createHyperState(id, AgentState.RUNNING, DIM), force, 0.3, 0));
    }

    const cp = mgr.save(DIM, tenantId('t'), 99, agents, [], [], [], []);
    expect(mgr.validate(cp).valid).toBe(true);

    const json = mgr.toJSON(cp);
    const restored = mgr.fromJSON(json);
    expect(mgr.validate(restored).valid).toBe(true);

    const { agents: ra, warnings } = mgr.restoreAgents(restored);
    expect(warnings).toHaveLength(0);
    expect(ra.size).toBe(3);

    for (const [id, orig] of agents) {
      const rest = ra.get(id)!;
      for (let d = 0; d < DIM; d++) {
        expect(rest.position[d]).toBeCloseTo(orig.position[d]!, 10);
      }
    }
  });

  it('should detect checksum corruption', () => {
    const mgr = new CheckpointManager();
    const cp = mgr.save(DIM, tenantId('t'), 1, new Map(), [], [], [], []);
    const corrupted = { ...cp, checksum: 'corrupted' };
    expect(mgr.validate(corrupted).valid).toBe(false);
  });

  it('should detect dimension mismatch', () => {
    const mgr = new CheckpointManager();
    const agents = new Map();
    agents.set(agentId('a'), createHyperState(agentId('a'), AgentState.READY, DIM));

    const cp = mgr.save(DIM, tenantId('t'), 1, agents, [], [], [], []);
    // Manually corrupt dimension
    const bad = { ...cp, kernel: { ...cp.kernel, dim: 64 }, checksum: 'x' };
    const result = mgr.validate(bad);
    expect(result.valid).toBe(false);
  });
});
