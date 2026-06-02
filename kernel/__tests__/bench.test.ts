/**
 * OASIS — Hardware-Calibrated Performance Benchmark
 *
 * Runs on the actual machine to establish real performance baselines.
 * Not unit tests — pure measurement.
 */

import { describe, it, expect } from 'vitest';
import { OasisKernel } from '../oasis.js';
import { VirtualActuator } from '../hal/drivers/virtual-actuator.js';
import { agentId, AgentState } from '../types.js';
import { zeros, randomUnit, norm, cosineSimilarity, distance } from '../physics/vector-math.js';
import { TensionField } from '../physics/tension-field.js';

const DIM = 32;

describe('Hardware Benchmark', () => {
  it('should measure real tick performance on this machine', async () => {
    const kernel = new OasisKernel({
      dim: DIM, tenantId: 'bench',
      enableImmune: false, monitorInterval: 0, branchCount: 3,
    });

    for (let i = 0; i < 8; i++) {
      kernel.addAgent(`a${i}`, AgentState.RUNNING, i < 2 ? 'HARD_RT' : 'BEST_EFFORT');
    }

    const act = new VirtualActuator('m', {
      name: 'm',
      constraints: { maxForceN: 50, maxTorqueNm: 25, maxVelocityMs: 5, geofenceBounds: [-50, -50, -10, 50, 50, 50] },
      massKg: 1.5, friction: 0.1,
    });
    await kernel.addDriver(act, agentId('a0'));

    const goal = zeros(DIM); goal[10] = 5;
    kernel.setGoal('g', goal);

    const force = zeros(DIM); force[10] = 1.5;
    for (let i = 0; i < 8; i++) kernel.emitTension(agentId(`a${i}`), force, 1);

    // Warmup
    for (let i = 0; i < 20; i++) kernel.tick();

    // Measure
    const memBefore = process.memoryUsage();
    const tickDurations: number[] = [];
    const TICKS = 500;

    for (let i = 0; i < TICKS; i++) {
      const s = process.hrtime.bigint();
      kernel.tick();
      tickDurations.push(Number(process.hrtime.bigint() - s));
    }

    const memAfter = process.memoryUsage();
    tickDurations.sort((a, b) => a - b);

    const avg = tickDurations.reduce((a, b) => a + b, 0) / TICKS / 1000;
    const p50 = tickDurations[Math.floor(TICKS * 0.5)]! / 1000;
    const p95 = tickDurations[Math.floor(TICKS * 0.95)]! / 1000;
    const p99 = tickDurations[Math.floor(TICKS * 0.99)]! / 1000;
    const max = tickDurations[TICKS - 1]! / 1000;
    const heapGrowthKB = (memAfter.heapUsed - memBefore.heapUsed) / 1024;
    const bytesPerTick = (heapGrowthKB * 1024) / TICKS;

    console.log(
      `\n=== OASIS TICK BENCHMARK ===\n` +
      `  Machine: Ryzen 3 PRO 2300U, 7GB RAM, 971MB free\n` +
      `  Config: ${DIM}-dim, 8 agents, 1 driver, no immune\n` +
      `  Ticks: ${TICKS}\n` +
      `  ─────────────────────────\n` +
      `  Avg:  ${avg.toFixed(0)} μs/tick\n` +
      `  P50:  ${p50.toFixed(0)} μs\n` +
      `  P95:  ${p95.toFixed(0)} μs\n` +
      `  P99:  ${p99.toFixed(0)} μs\n` +
      `  Max:  ${max.toFixed(0)} μs\n` +
      `  ─────────────────────────\n` +
      `  Heap growth: ${heapGrowthKB.toFixed(0)} KB (${bytesPerTick.toFixed(0)} bytes/tick)\n` +
      `  Max freq: ${Math.floor(1_000_000 / avg)} Hz\n` +
      `  1kHz capable: ${avg < 1000 ? 'YES ✓' : 'NO (' + Math.floor(1_000_000 / avg) + 'Hz)'}`,
    );

    expect(avg).toBeGreaterThan(0);
  });

  it('should benchmark vector math hot path', () => {
    const N = 100_000;
    const a = randomUnit(DIM);
    const b = randomUnit(DIM);

    // cosineSimilarity
    const csStart = process.hrtime.bigint();
    for (let i = 0; i < N; i++) cosineSimilarity(a, b);
    const csNs = Number(process.hrtime.bigint() - csStart) / N;

    // distance
    const dStart = process.hrtime.bigint();
    for (let i = 0; i < N; i++) distance(a, b);
    const dNs = Number(process.hrtime.bigint() - dStart) / N;

    // norm
    const nStart = process.hrtime.bigint();
    for (let i = 0; i < N; i++) norm(a);
    const nNs = Number(process.hrtime.bigint() - nStart) / N;

    console.log(
      `\n=== VECTOR MATH BENCHMARK (${DIM}-dim, ${N} iters) ===\n` +
      `  cosineSimilarity: ${csNs.toFixed(0)} ns\n` +
      `  distance:         ${dNs.toFixed(0)} ns\n` +
      `  norm:             ${nNs.toFixed(0)} ns`,
    );

    expect(csNs).toBeLessThan(10_000); // Must be < 10μs
  });

  it('should benchmark tension field sample with varying load', () => {
    const counts = [10, 100, 1000, 5000];
    const results: string[] = [];

    for (const count of counts) {
      const field = new TensionField(count + 100, DIM);
      const sig = randomUnit(DIM);

      for (let i = 0; i < count; i++) {
        field.emit({
          source: agentId('src'),
          tenantId: 'bench' as any,
          force: randomUnit(DIM),
          signature: randomUnit(DIM),
          intensity: Math.random() * 3,
          decayRate: 3,
          emittedAt: 0n,
          ticksRemaining: 20,
          bucketKey: 0,
        });
      }

      const SAMPLES = 100;
      const start = process.hrtime.bigint();
      for (let i = 0; i < SAMPLES; i++) {
        field.sample(randomUnit(DIM), sig, 'bench' as any);
      }
      const avgUs = Number(process.hrtime.bigint() - start) / SAMPLES / 1000;
      results.push(`  ${count} tensions: ${avgUs.toFixed(0)} μs/sample`);
    }

    console.log(
      `\n=== TENSION FIELD SAMPLE BENCHMARK ===\n` +
      results.join('\n'),
    );
  });
});
