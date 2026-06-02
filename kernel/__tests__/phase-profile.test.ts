/**
 * Phase-by-phase profiling of the tick pipeline.
 * Identifies which phase costs the most on this hardware.
 */

import { describe, it, expect } from 'vitest';
import { OasisKernel } from '../oasis.js';
import { VirtualActuator } from '../hal/drivers/virtual-actuator.js';
import { agentId, AgentState, monotonicNow } from '../types.js';
import { zeros } from '../physics/vector-math.js';

const DIM = 32;

describe('Phase Profiler', () => {
  it('should identify the costliest phase', async () => {
    // Create kernels with different features to isolate costs
    const configs = [
      { name: 'BARE (engine only)', enableSwarm: false, enableImmune: false, enableEmotions: false, enableMorpho: false, enableBranching: false, enableDreams: false },
      { name: '+EMOTIONS', enableSwarm: false, enableImmune: false, enableEmotions: true, enableMorpho: false, enableBranching: false, enableDreams: false },
      { name: '+MORPHO', enableSwarm: false, enableImmune: false, enableEmotions: true, enableMorpho: true, enableBranching: false, enableDreams: false },
      { name: '+BRANCHING', enableSwarm: false, enableImmune: false, enableEmotions: true, enableMorpho: true, enableBranching: true, enableDreams: false },
      { name: '+SWARM', enableSwarm: true, enableImmune: false, enableEmotions: true, enableMorpho: true, enableBranching: true, enableDreams: false },
      { name: 'FULL (all)', enableSwarm: true, enableImmune: false, enableEmotions: true, enableMorpho: true, enableBranching: true, enableDreams: true },
    ];

    const TICKS = 100;
    const results: string[] = [];

    for (const cfg of configs) {
      const kernel = new OasisKernel({ dim: DIM, tenantId: 'prof', monitorInterval: 0, branchCount: 3, ...cfg });

      for (let i = 0; i < 8; i++) kernel.addAgent(`a${i}`, AgentState.RUNNING, i < 2 ? 'HARD_RT' : 'BEST_EFFORT');
      const act = new VirtualActuator('m', { name: 'm', constraints: { maxForceN: 50, maxTorqueNm: 25, maxVelocityMs: 5, geofenceBounds: [-50, -50, -10, 50, 50, 50] }, massKg: 1.5, friction: 0.1 });
      await kernel.addDriver(act, agentId('a0'));
      const g = zeros(DIM); g[10] = 5;
      kernel.setGoal('g', g);
      const f = zeros(DIM); f[10] = 1.5;
      for (let i = 0; i < 8; i++) kernel.emitTension(agentId(`a${i}`), f, 1);

      // Warmup
      for (let i = 0; i < 5; i++) kernel.tick();

      const heapBefore = process.memoryUsage().heapUsed;
      const start = process.hrtime.bigint();
      for (let i = 0; i < TICKS; i++) kernel.tick();
      const elapsed = Number(process.hrtime.bigint() - start) / TICKS / 1000;
      const heapAfter = process.memoryUsage().heapUsed;
      const heapPerTick = (heapAfter - heapBefore) / TICKS;

      results.push(`  ${cfg.name.padEnd(25)} ${elapsed.toFixed(0).padStart(7)} μs/tick  ${(heapPerTick/1024).toFixed(0).padStart(5)} KB/tick`);
    }

    console.log(
      `\n=== PHASE COST BREAKDOWN (${TICKS} ticks, 8 agents) ===\n` +
      `  ${'Config'.padEnd(25)} ${'Time'.padStart(7)}          ${'Heap'.padStart(5)}\n` +
      `  ${'─'.repeat(50)}\n` +
      results.join('\n'),
    );

    expect(results.length).toBe(6);
  });
});
