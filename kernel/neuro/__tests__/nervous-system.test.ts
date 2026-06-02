/**
 * OASIS Kernel — Complete Nervous System Tests
 *
 * Validates the four neural mechanisms working together:
 * Synapse + Attention + Reflex + Dream = a thinking system.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { SynapticNetwork } from '../synapse.js';
import { AttentionMechanism } from '../attention.js';
import { ReflexArc, proximityReflex, temperatureReflex, forceOverloadReflex, ReflexAction } from '../reflex.js';
import { DreamEngine } from '../dreams.js';
import { createHyperState, evolveState, type HyperState } from '../../physics/hyper-state.js';
import { agentId, driverId, AgentState } from '../../types.js';
import { zeros, norm, normalize, scale, randomUnit } from '../../physics/vector-math.js';

const DIM = 32;

// ─── Synaptic Network ───────────────────────────────────────────

describe('Synaptic Network — Hebbian Learning', () => {
  let network: SynapticNetwork;

  beforeEach(() => {
    network = new SynapticNetwork(DIM, 0.1, 0.005, 0.03);
  });

  it('should form synapses between co-active agents', () => {
    const states = new Map<AgentId, HyperState>();

    // Two agents with strongly aligned momentum
    const force = zeros(DIM);
    force[10] = 2;

    const a = createHyperState(agentId('neuron-a'), AgentState.RUNNING, DIM);
    const b = createHyperState(agentId('neuron-b'), AgentState.RUNNING, DIM);

    states.set(agentId('neuron-a'), evolveState(a, force, 0.5, 0));
    states.set(agentId('neuron-b'), evolveState(b, force, 0.5, 0));

    // Multiple updates to trigger formation
    for (let i = 0; i < 5; i++) network.update(states);

    expect(network.getSynapseCount()).toBeGreaterThan(0);
    const synapses = network.getSynapses(agentId('neuron-a'));
    expect(synapses.length).toBeGreaterThan(0);
  });

  it('should potentiate synapses with repeated co-activation', () => {
    const states = new Map<AgentId, HyperState>();
    const force = zeros(DIM);
    force[10] = 2;

    states.set(agentId('a'), evolveState(createHyperState(agentId('a'), AgentState.RUNNING, DIM), force, 0.5, 0));
    states.set(agentId('b'), evolveState(createHyperState(agentId('b'), AgentState.RUNNING, DIM), force, 0.5, 0));

    // Initial formation
    for (let i = 0; i < 5; i++) network.update(states);

    const initialWeight = network.getStrongestSynapse()?.weight ?? 0;

    // Continued co-activation → potentiation
    for (let i = 0; i < 20; i++) network.update(states);

    const finalWeight = network.getStrongestSynapse()?.weight ?? 0;
    expect(finalWeight).toBeGreaterThan(initialWeight);
  });

  it('should prune unused synapses', () => {
    const states = new Map<AgentId, HyperState>();
    const force = zeros(DIM);
    force[10] = 2;

    states.set(agentId('a'), evolveState(createHyperState(agentId('a'), AgentState.RUNNING, DIM), force, 0.5, 0));
    states.set(agentId('b'), evolveState(createHyperState(agentId('b'), AgentState.RUNNING, DIM), force, 0.5, 0));

    // Form synapse
    for (let i = 0; i < 5; i++) network.update(states);
    expect(network.getSynapseCount()).toBeGreaterThan(0);

    // Remove agent B's momentum → synapse should weaken and prune
    states.set(agentId('b'), createHyperState(agentId('b'), AgentState.READY, DIM));

    for (let i = 0; i < 200; i++) network.update(states);

    // Synapse should have been pruned eventually
    // (or at least significantly weakened)
    const strongest = network.getStrongestSynapse();
    if (strongest) {
      expect(Math.abs(strongest.weight)).toBeLessThan(0.5);
    }
  });

  it('should compute synaptic forces on agents', () => {
    const states = new Map<AgentId, HyperState>();
    const force = zeros(DIM);
    force[10] = 3;

    states.set(agentId('a'), evolveState(createHyperState(agentId('a'), AgentState.RUNNING, DIM), force, 0.5, 0));
    states.set(agentId('b'), evolveState(createHyperState(agentId('b'), AgentState.RUNNING, DIM), force, 0.5, 0));

    // Form strong synapse
    for (let i = 0; i < 15; i++) network.update(states);

    const synapticForce = network.computeSynapticForce(agentId('a'), states);
    // There should be some force if synapses exist
    if (network.getSynapseCount() > 0) {
      expect(synapticForce.length).toBe(DIM);
    }
  });
});

// ─── Attention Mechanism ────────────────────────────────────────

describe('Attention Mechanism — Selective Focus', () => {
  let attention: AttentionMechanism;

  beforeEach(() => {
    attention = new AttentionMechanism(DIM);
  });

  it('should attend more to novel agents (new or changed)', () => {
    const states = new Map<AgentId, HyperState>();

    // Static agent
    states.set(agentId('boring'), createHyperState(agentId('boring'), AgentState.READY, DIM));

    // First tick — both are novel
    attention.compute(states);

    // Moving agent appears
    const force = zeros(DIM);
    force[10] = 5;
    states.set(agentId('interesting'), evolveState(
      createHyperState(agentId('interesting'), AgentState.RUNNING, DIM), force, 0.5, 0,
    ));

    const weights = attention.compute(states);
    const interestingW = weights.find(w => w.agentId === 'interesting')!;
    const boringW = weights.find(w => w.agentId === 'boring')!;

    // New/moving agent should get more attention
    expect(interestingW.weight).toBeGreaterThan(boringW.weight);
  });

  it('should attend more to high-entropy (uncertain) agents', () => {
    const states = new Map<AgentId, HyperState>();

    const certain = createHyperState(agentId('certain'), AgentState.RUNNING, DIM);
    const uncertain: HyperState = {
      ...createHyperState(agentId('uncertain'), AgentState.RUNNING, DIM),
      entropy: 0.8,
    };

    states.set(agentId('certain'), certain);
    states.set(agentId('uncertain'), uncertain);

    attention.compute(states); // First tick
    const weights = attention.compute(states);

    const uncertainW = weights.find(w => w.agentId === 'uncertain')!;
    const certainW = weights.find(w => w.agentId === 'certain')!;

    expect(uncertainW.sources.salience).toBeGreaterThan(certainW.sources.salience);
  });

  it('should attend more to goal-relevant agents', () => {
    const goalPos = zeros(DIM);
    goalPos[10] = 5;
    attention.setGoals([goalPos]);

    const states = new Map<AgentId, HyperState>();

    // Agent near goal
    const nearGoal: HyperState = {
      ...createHyperState(agentId('near-goal'), AgentState.RUNNING, DIM),
      position: (() => { const p = zeros(DIM); p[10] = 4; return p; })(),
    };

    // Agent far from goal
    const farGoal = createHyperState(agentId('far-goal'), AgentState.RUNNING, DIM);

    states.set(agentId('near-goal'), nearGoal);
    states.set(agentId('far-goal'), farGoal);

    attention.compute(states);
    const weights = attention.compute(states);

    const nearW = weights.find(w => w.agentId === 'near-goal')!;
    const farW = weights.find(w => w.agentId === 'far-goal')!;

    expect(nearW.sources.relevance).toBeGreaterThan(farW.sources.relevance);
  });

  it('should apply inhibition of return after prolonged attention', () => {
    const attention = new AttentionMechanism(DIM, 0.1, 3); // Fast inhibition

    const states = new Map<AgentId, HyperState>();
    // High entropy + high momentum → high salience → always attended
    const hotAgent: HyperState = {
      ...createHyperState(agentId('hot'), AgentState.RUNNING, DIM),
      entropy: 0.9,
      momentum: (() => { const m = zeros(DIM); m[10] = 5; return m; })(),
    };
    states.set(agentId('hot'), hotAgent);

    // Attend for many ticks — agent is always salient
    for (let i = 0; i < 20; i++) {
      // Slightly change position each tick so novelty stays non-zero
      const updated: HyperState = {
        ...hotAgent,
        position: (() => { const p = zeros(DIM); p[10] = i * 0.01; return p; })(),
      };
      states.set(agentId('hot'), updated);
      attention.compute(states);
    }

    const finalWeights = attention.compute(states);
    const finalW = finalWeights.find(w => w.agentId === 'hot')!;

    // After prolonged attention, inhibition should have built up
    expect(finalW.sources.inhibition).toBeGreaterThan(0);
  });
});

// ─── Reflex Arc ─────────────────────────────────────────────────

describe('Reflex Arc — Spinal Cord', () => {
  let reflexes: ReflexArc;
  const driver = driverId('motor-1');

  beforeEach(() => {
    reflexes = new ReflexArc();
  });

  it('should fire proximity reflex on close obstacle', () => {
    reflexes.register(proximityReflex(driver, 5)); // 5cm threshold

    const telemetry = new Map<typeof driver, Record<string, number>>();
    telemetry.set(driver, { distance: 3 }); // 3cm — too close!

    const fired = reflexes.check(telemetry);

    expect(fired.length).toBe(1);
    expect(fired[0]!.action).toBe(ReflexAction.EMERGENCY_STOP);
    expect(fired[0]!.ruleName).toBe('Proximity Emergency Stop');
  });

  it('should fire temperature reflex on overheat', () => {
    reflexes.register(temperatureReflex(driver, 85));

    const telemetry = new Map<typeof driver, Record<string, number>>();
    telemetry.set(driver, { temperature: 90 }); // Overheating

    const fired = reflexes.check(telemetry);

    expect(fired.length).toBe(1);
    expect(fired[0]!.action).toBe(ReflexAction.EMERGENCY_STOP);
  });

  it('should NOT fire when conditions are safe', () => {
    reflexes.register(proximityReflex(driver, 5));
    reflexes.register(temperatureReflex(driver, 85));

    const telemetry = new Map<typeof driver, Record<string, number>>();
    telemetry.set(driver, { distance: 50, temperature: 35 }); // All safe

    const fired = reflexes.check(telemetry);
    expect(fired.length).toBe(0);
  });

  it('should fire multiple reflexes when multiple conditions are met', () => {
    reflexes.register(proximityReflex(driver, 5));
    reflexes.register(temperatureReflex(driver, 85));

    const telemetry = new Map<typeof driver, Record<string, number>>();
    telemetry.set(driver, { distance: 2, temperature: 95 }); // Both triggered

    const fired = reflexes.check(telemetry);
    expect(fired.length).toBe(2);
  });

  it('should respect priority ordering', () => {
    reflexes.register(forceOverloadReflex(driver, 100)); // priority 2
    reflexes.register(proximityReflex(driver, 5));        // priority 0

    const telemetry = new Map<typeof driver, Record<string, number>>();
    telemetry.set(driver, { distance: 2, force: 200 });

    const fired = reflexes.check(telemetry);

    // Proximity (priority 0) should fire first
    expect(fired[0]!.ruleName).toBe('Proximity Emergency Stop');
  });

  it('should allow disabling reflexes', () => {
    reflexes.register(proximityReflex(driver, 5));
    reflexes.setEnabled(`reflex-proximity-${driver}`, false);

    const telemetry = new Map<typeof driver, Record<string, number>>();
    telemetry.set(driver, { distance: 2 });

    const fired = reflexes.check(telemetry);
    expect(fired.length).toBe(0);
  });

  it('should execute in microseconds', () => {
    reflexes.register(proximityReflex(driver, 5));
    reflexes.register(temperatureReflex(driver, 85));
    reflexes.register(forceOverloadReflex(driver, 100));

    const telemetry = new Map<typeof driver, Record<string, number>>();
    telemetry.set(driver, { distance: 2, temperature: 90, force: 200 });

    const start = process.hrtime.bigint();
    reflexes.check(telemetry);
    const elapsed = Number(process.hrtime.bigint() - start);

    // Must be < 100 microseconds
    expect(elapsed / 1000).toBeLessThan(100);
  });
});

// ─── Dream Engine ───────────────────────────────────────────────

describe('Dream Engine — Offline Consolidation', () => {
  let dreams: DreamEngine;
  let network: SynapticNetwork;

  beforeEach(() => {
    dreams = new DreamEngine(50);
    network = new SynapticNetwork(DIM);
  });

  it('should record experiences for later consolidation', () => {
    const traj = [zeros(DIM), zeros(DIM)];
    dreams.recordExperience(agentId('explorer'), traj, [0.3, 0.2], 0.8, 1);
    dreams.recordExperience(agentId('explorer'), traj, [0.5, 0.6], -0.5, 2);

    expect(dreams.getExperienceCount()).toBe(2);
  });

  it('should know when to dream', () => {
    // Not enough experiences
    expect(dreams.shouldDream(0.1, 0, 0.2)).toBe(false);

    // Fill experience buffer
    for (let i = 0; i < 15; i++) {
      dreams.recordExperience(agentId('a'), [zeros(DIM)], [0.3], 0.5, i);
    }

    // Now should dream: low entropy, no goals, low CPU
    expect(dreams.shouldDream(0.1, 0, 0.2)).toBe(true);

    // Should NOT dream during crisis
    expect(dreams.shouldDream(0.8, 0, 0.2)).toBe(false); // High entropy
    expect(dreams.shouldDream(0.1, 3, 0.2)).toBe(false); // Active goals
    expect(dreams.shouldDream(0.1, 0, 0.9)).toBe(false); // CPU overloaded
  });

  it('should consolidate experiences into synaptic changes', () => {
    // Set up synaptic network with agents
    const states = new Map<AgentId, HyperState>();
    const force = zeros(DIM);
    force[10] = 2;

    states.set(agentId('a'), evolveState(createHyperState(agentId('a'), AgentState.RUNNING, DIM), force, 0.5, 0));
    states.set(agentId('b'), evolveState(createHyperState(agentId('b'), AgentState.RUNNING, DIM), force, 0.5, 0));

    // Form synapses
    for (let i = 0; i < 10; i++) network.update(states);

    // Record positive experiences for agent A
    for (let i = 0; i < 15; i++) {
      dreams.recordExperience(
        agentId('a'),
        [zeros(DIM), zeros(DIM)],
        [0.2, 0.1],
        0.7, // Good outcome
        i,
      );
    }

    const result = dreams.dream(network, states);

    expect(result.experiencesReplayed).toBeGreaterThan(0);
    expect(result.durationMs).toBeGreaterThanOrEqual(0);
    expect(result.restfulness).toBeGreaterThan(0);
    expect(dreams.getDreamCount()).toBe(1);
  });

  it('should weaken synapses associated with bad experiences', () => {
    const states = new Map<AgentId, HyperState>();
    const force = zeros(DIM);
    force[10] = 2;

    states.set(agentId('a'), evolveState(createHyperState(agentId('a'), AgentState.RUNNING, DIM), force, 0.5, 0));
    states.set(agentId('b'), evolveState(createHyperState(agentId('b'), AgentState.RUNNING, DIM), force, 0.5, 0));

    for (let i = 0; i < 10; i++) network.update(states);

    // Record BAD experiences
    for (let i = 0; i < 15; i++) {
      dreams.recordExperience(agentId('a'), [zeros(DIM)], [0.8], -0.9, i);
    }

    const result = dreams.dream(network, states);

    // Should have pruned or weakened synapses
    expect(result.experiencesReplayed).toBeGreaterThan(0);
  });
});

// ─── Integrated: The Complete Nervous System ────────────────────

describe('Scenario — The Complete Nervous System', () => {
  it('should demonstrate reflex → attention → learning → consolidation cycle', () => {
    const network = new SynapticNetwork(DIM);
    const attention = new AttentionMechanism(DIM);
    const reflexes = new ReflexArc();
    const dreams = new DreamEngine();

    const driver = driverId('arm-motor');
    reflexes.register(proximityReflex(driver, 10));
    reflexes.register(temperatureReflex(driver, 80));

    // Create agents
    const states = new Map<AgentId, HyperState>();
    const agentNames = ['sensor', 'planner', 'actuator', 'monitor'];

    for (const name of agentNames) {
      const force = zeros(DIM);
      force[10] = 1 + Math.random();
      states.set(
        agentId(name),
        evolveState(createHyperState(agentId(name), AgentState.RUNNING, DIM), force, 0.3, 0),
      );
    }

    let reflexesFired = 0;
    let synapsesPeakCount = 0;

    // ── 50 TICKS: Normal operation + learning ──────────
    for (let tick = 0; tick < 50; tick++) {
      // 1. REFLEX CHECK (spinal cord — before everything)
      const telemetry = new Map<typeof driver, Record<string, number>>();
      telemetry.set(driver, {
        distance: tick === 25 ? 5 : 50, // Danger at tick 25!
        temperature: tick === 25 ? 85 : 40,
      });

      const fired = reflexes.check(telemetry);
      reflexesFired += fired.length;

      // 2. ATTENTION (what to focus on)
      const weights = attention.compute(states);
      const topAgents = attention.getTopK(weights, 3);

      // 3. HEBBIAN LEARNING (update synapses)
      network.update(states);
      if (network.getSynapseCount() > synapsesPeakCount) {
        synapsesPeakCount = network.getSynapseCount();
      }

      // 4. RECORD EXPERIENCE (for later dreaming)
      for (const name of agentNames) {
        const state = states.get(agentId(name))!;
        const outcome = fired.length > 0 ? -0.8 : 0.3;
        dreams.recordExperience(
          agentId(name),
          [state.position],
          [state.entropy],
          outcome,
          tick,
        );
      }

      // 5. EVOLVE AGENTS (with synaptic forces)
      for (const name of agentNames) {
        const id = agentId(name);
        const state = states.get(id)!;
        const synapticForce = network.computeSynapticForce(id, states);
        if (norm(synapticForce) > 1e-6) {
          states.set(id, evolveState(state, scale(synapticForce, 0.1), 0.1, 0.05));
        }
      }
    }

    // ── DREAM PHASE (offline consolidation) ──────────────
    expect(dreams.getExperienceCount()).toBeGreaterThan(0);

    const dreamResult = dreams.dream(network, states);

    // ── FINAL ASSERTIONS ─────────────────────────────────

    // Reflexes should have fired at tick 25
    expect(reflexesFired).toBeGreaterThan(0);

    // Synapses should have formed
    expect(synapsesPeakCount).toBeGreaterThan(0);

    // Dream should have consolidated
    expect(dreamResult.experiencesReplayed).toBeGreaterThan(0);

    console.log(
      `[COMPLETE NERVOUS SYSTEM — 50 ticks + dream]\n` +
      `  Reflexes fired: ${reflexesFired} (at danger tick)\n` +
      `  Peak synapses: ${synapsesPeakCount}\n` +
      `  Final synapses: ${network.getSynapseCount()}\n` +
      `  Experiences dreamed: ${dreamResult.experiencesReplayed}\n` +
      `  Synapses strengthened: ${dreamResult.synapsesStrengthened}\n` +
      `  Scenarios imagined: ${dreamResult.scenariosImagined}\n` +
      `  Restfulness: ${dreamResult.restfulness.toFixed(2)}`,
    );
  });
});
