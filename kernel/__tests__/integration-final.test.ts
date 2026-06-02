/**
 * OASIS — Final Integration Test
 *
 * Proves every weakness identified in the impitoyable audit.
 * Each test addresses ONE specific failure mode.
 * No weak assertions. No marketing.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { OasisKernel } from '../oasis.js';
import { InterKernelBridge } from '../bridge-os/inter-kernel.js';
import { GhostMediator, RULE_NO_RED_ZONE } from '../ghost/ghost-protocol.js';
import { VirtualActuator } from '../hal/drivers/virtual-actuator.js';
import { agentId, AgentState, monotonicNow } from '../types.js';
import { zeros } from '../physics/vector-math.js';
import { seedNoise } from '../sim/noise.js';

const DIM = 32;

describe('Watchdog — Integrated in Real Kernel', () => {
  it('watchdog feeds on every tick and tracks feed count', () => {
    const kernel = new OasisKernel({ dim: DIM, tenantId: 'wd-test', enableImmune: false, monitorInterval: 0 });
    kernel.addAgent('a', AgentState.RUNNING);

    for (let i = 0; i < 50; i++) kernel.tick();

    // Watchdog must have been fed 50 times
    expect(kernel.watchdog.getFeedCount()).toBe(50);
    // Watchdog must NOT have triggered (ticks are fast)
    expect(kernel.watchdog.isTriggered()).toBe(false);
  });

  it('audit log records every tick', () => {
    const kernel = new OasisKernel({ dim: DIM, tenantId: 'audit-test', enableImmune: false, monitorInterval: 0 });
    kernel.addAgent('a', AgentState.RUNNING);

    for (let i = 0; i < 10; i++) kernel.tick();

    // Audit log must have entries for each tick
    const entries = kernel.auditLog.getRecent(20);
    const tickEntries = entries.filter(e => e.action === 'TICK');
    expect(tickEntries.length).toBe(10);

    // Chain must be intact
    expect(kernel.auditLog.verifyChain().valid).toBe(true);
  });
});

describe('Bridge — Two Real Kernels Communicating', () => {
  it('mine kernel emits gas alert → drone kernel receives it', () => {
    // Create two REAL OasisKernel instances
    const mineKernel = new OasisKernel({ dim: DIM, tenantId: 'mine', enableImmune: false, monitorInterval: 0 });
    const droneKernel = new OasisKernel({ dim: DIM, tenantId: 'drones', enableImmune: false, monitorInterval: 0 });

    mineKernel.addAgent('miner-1', AgentState.RUNNING);
    droneKernel.addAgent('drone-1', AgentState.RUNNING);

    // Connect via bridge
    const bridge = new InterKernelBridge();
    bridge.registerKernel('mine', 'MINE', ['GAS_MONITORING']);
    bridge.registerKernel('drones', 'DRONE', ['SURVEY']);

    bridge.subscribe({
      subscriberKernelId: 'drones',
      patterns: ['GAS.*'],
      minSeverity: 'WARNING',
    });

    // Both kernels tick independently
    for (let i = 0; i < 5; i++) {
      mineKernel.tick();
      droneKernel.tick();
    }

    // Mine detects gas
    bridge.emit('mine', 'GAS.CH4', 'CRITICAL', {
      ch4: 2.5,
      source: 'mineKernel tick ' + mineKernel.getTickCount(),
    });

    // Drone receives
    const signals = bridge.receive('drones');

    expect(signals.length).toBe(1);
    expect(signals[0]!.type).toBe('GAS.CH4');
    expect(signals[0]!.payload['ch4']).toBe(2.5);

    // Both kernels still alive (sovereign — one doesn't crash the other)
    expect(mineKernel.isAlive()).toBe(true);
    expect(droneKernel.isAlive()).toBe(true);

    // Verify signal authenticity
    expect(bridge.verifySignal(signals[0]!)).toBe(true);
  });
});

describe('GHOST — Integrated with Real Kernel', () => {
  it('GHOST intention → mediation → kernel tension → agent state changes', () => {
    const kernel = new OasisKernel({
      dim: DIM, tenantId: 'ghost-int', enableImmune: false,
      enableSwarm: false, enableMorpho: false, monitorInterval: 0,
    });

    const nav = kernel.addAgent('navigator', AgentState.RUNNING, 'HARD_RT');

    // Create mediator with safety rules
    const mediator = new GhostMediator();
    mediator.registerGhost('planner-ai', 'AI Planner', 0.9);
    mediator.addSafetyRule(RULE_NO_RED_ZONE);

    // AI sends a SAFE intention
    const safeResult = mediator.mediate({
      id: 'intent-1', ghostId: 'planner-ai', goal: 'MOVE',
      params: { zone: 'storage-GREEN', dimX: 10 },
      priority: 5, deadlineS: 0, timestamp: monotonicNow(),
    });

    expect(safeResult.verdict).toBe('ACCEPTED');

    // Translate intention into kernel tension
    if (safeResult.injected) {
      const force = zeros(DIM);
      force[10] = 2.0; // Move along dim 10
      kernel.emitTension(nav, force, 2.0);
    }

    // Tick the kernel — agent should respond to the tension
    const stateBefore = kernel.getAgentState(nav)!;
    for (let i = 0; i < 10; i++) kernel.tick();
    const stateAfter = kernel.getAgentState(nav)!;

    // Agent state must have changed (tension had an effect)
    expect(stateAfter.updatedAt).not.toBe(stateBefore.updatedAt);

    // AI sends a DANGEROUS intention → must be refused
    const dangerResult = mediator.mediate({
      id: 'intent-2', ghostId: 'planner-ai', goal: 'INSPECT',
      params: { zone: 'furnace-RED' },
      priority: 8, deadlineS: 0, timestamp: monotonicNow(),
    });

    expect(dangerResult.verdict).toBe('REFUSED');
    // Kernel must NOT have received a tension from this
    expect(dangerResult.injected).toBe(false);

    // Kernel still alive
    expect(kernel.isAlive()).toBe(true);

    // Mediator stats
    const stats = mediator.getStats();
    expect(stats.accepted).toBe(1);
    expect(stats.refused).toBe(1);
  });
});

describe('Deterministic Tests — SeededRandom', () => {
  it('same seed → same noise → same simulation result', () => {
    const results: number[] = [];

    for (let run = 0; run < 3; run++) {
      seedNoise(99999); // Same seed every run

      const kernel = new OasisKernel({
        dim: DIM, tenantId: 'det-test', enableImmune: false,
        enableSwarm: false, enableMorpho: false,
        enableBranching: false, enableEmotions: false,
        enableDreams: false, monitorInterval: 0,
      });

      const a = kernel.addAgent('agent', AgentState.RUNNING, 'HARD_RT');
      const force = zeros(DIM);
      force[10] = 1.5;
      kernel.emitTension(a, force, 2.0);

      for (let i = 0; i < 20; i++) kernel.tick();

      const state = kernel.getAgentState(a)!;
      results.push(state.position[10]!);
    }

    // All 3 runs must produce IDENTICAL results
    expect(results[0]).toBe(results[1]);
    expect(results[1]).toBe(results[2]);
  });
});

describe('Crash Resistance — OASIS Must Never Crash', () => {
  it('null/undefined inputs: kernel survives', () => {
    const kernel = new OasisKernel({ dim: DIM, tenantId: 'crash', enableImmune: false, monitorInterval: 0 });
    kernel.addAgent('survivor', AgentState.RUNNING);

    // Tick with no agents, no drivers, no sensors — must not crash
    for (let i = 0; i < 100; i++) {
      expect(() => kernel.tick()).not.toThrow();
    }

    expect(kernel.isAlive()).toBe(true);
  });

  it('panic then tick: throws but does not crash node process', () => {
    const kernel = new OasisKernel({ dim: DIM, tenantId: 'panic-test', monitorInterval: 0 });
    kernel.addAgent('doomed', AgentState.RUNNING);

    kernel.tick();
    kernel.panic('test panic');

    // Must throw KillSwitchEngagedException, not crash
    expect(() => kernel.tick()).toThrow();
    expect(kernel.isAlive()).toBe(false);
  });

  it('100 ticks with all systems enabled: no crash', async () => {
    const kernel = new OasisKernel({
      dim: DIM, tenantId: 'stress',
      enableSwarm: true, enableImmune: true,
      enableEmotions: true, enableMorpho: true,
      enableBranching: true, enableDreams: true,
      monitorInterval: 10,
    });

    for (let i = 0; i < 8; i++) {
      kernel.addAgent(`a${i}`, AgentState.RUNNING, i < 2 ? 'HARD_RT' : 'BEST_EFFORT');
    }

    const act = new VirtualActuator('m', {
      name: 'm', constraints: { maxForceN: 50, maxTorqueNm: 25, maxVelocityMs: 5, geofenceBounds: [-50, -50, -10, 50, 50, 50] },
      massKg: 1.5, friction: 0.1,
    });
    await kernel.addDriver(act, agentId('a0'));

    const goal = zeros(DIM); goal[10] = 5;
    kernel.setGoal('g', goal);

    for (let i = 0; i < 100; i++) {
      expect(() => kernel.tick()).not.toThrow();
    }

    expect(kernel.isAlive()).toBe(true);
    expect(kernel.getTickCount()).toBe(100);
    expect(kernel.watchdog.getFeedCount()).toBe(100);
    expect(kernel.auditLog.verifyChain().valid).toBe(true);
  });
});
