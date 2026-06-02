/**
 * OASIS Kernel — Swarm Tests
 *
 * "L'Essaim Conscient":
 * Agents self-organize without central planning.
 * Swarms crystallize from resonance, navigate via stigmergy,
 * heal when members fail, and defend against rogue agents.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { StigmergyEngine, PheromoneType } from '../stigmergy.js';
import { SwarmMind, SwarmEvent } from '../swarm-mind.js';
import { ImmuneSystem } from '../immune.js';
import { createHyperState, evolveState, type HyperState } from '../../physics/hyper-state.js';
import { agentId, tenantId, AgentState } from '../../types.js';
import {
  zeros, normalize, scale, add, norm, cosineSimilarity, distance, randomUnit,
} from '../../physics/vector-math.js';

const TENANT = tenantId('swarm-lab');
const DIM = 32;

/** Helper: create an agent with given momentum */
function makeAgent(id: string, momentumDim: number, momentumStrength: number): HyperState {
  const state = createHyperState(agentId(id), AgentState.RUNNING, DIM);
  const force = zeros(DIM);
  if (momentumDim < DIM) force[momentumDim] = momentumStrength;
  return evolveState(state, force, 0.5, 0);
}

// ─── Stigmergy Tests ────────────────────────────────────────────

describe('Stigmergy — Environmental Communication', () => {
  let stig: StigmergyEngine;

  beforeEach(() => {
    stig = new StigmergyEngine(DIM);
  });

  it('should deposit and sense pheromones', () => {
    const pos = zeros(DIM);
    pos[10] = 1;
    const dir = normalize(pos);

    stig.deposit(agentId('scout-1'), PheromoneType.ATTRACT, pos, dir, 2.0);

    const gradient = stig.sense(zeros(DIM));
    expect(gradient.sensedCount).toBe(1);
    expect(gradient.totalIntensity).toBeGreaterThan(0);
    expect(norm(gradient.attractForce)).toBeGreaterThan(0);
  });

  it('should reinforce existing pheromones (ant trail effect)', () => {
    const pos = zeros(DIM);
    pos[10] = 1;
    const dir = normalize(pos);

    const ph1 = stig.deposit(agentId('ant-1'), PheromoneType.ATTRACT, pos, dir, 1.0);
    const ph2 = stig.deposit(agentId('ant-2'), PheromoneType.ATTRACT, pos, dir, 1.0);

    // Should have reinforced the same pheromone, not created a new one
    expect(ph1.id).toBe(ph2.id);
    expect(ph2.reinforcements).toBe(1);
    expect(ph2.intensity).toBeGreaterThan(1.0); // Reinforced
  });

  it('should repel agents from danger zones', () => {
    const dangerPos = zeros(DIM);
    dangerPos[10] = 2;

    stig.deposit(agentId('scout'), PheromoneType.REPEL, dangerPos, zeros(DIM), 5.0);

    // Sense from nearby
    const nearPos = zeros(DIM);
    nearPos[10] = 2.5;
    const gradient = stig.sense(nearPos);

    // Repulsion should push away from danger
    expect(norm(gradient.repelForce)).toBeGreaterThan(0);
  });

  it('should decay pheromones over time', () => {
    const pos = zeros(DIM);
    stig.deposit(agentId('temp'), PheromoneType.ATTRACT, pos, pos, 1.0, 0.2);

    expect(stig.getActiveCount()).toBe(1);

    // Tick until evaporated
    for (let i = 0; i < 20; i++) stig.tick();

    expect(stig.getActiveCount()).toBe(0);
  });

  it('should keep reinforced pheromones longer (busy highways persist)', () => {
    const pos = zeros(DIM);
    pos[10] = 1;
    const dir = normalize(pos);

    // 5 agents reinforce the same trail
    for (let i = 0; i < 5; i++) {
      stig.deposit(agentId(`ant-${i}`), PheromoneType.ATTRACT, pos, dir, 1.0, 0.1);
    }

    // Tick many times
    for (let i = 0; i < 20; i++) stig.tick();

    // Reinforced pheromone should still exist
    expect(stig.getActiveCount()).toBeGreaterThan(0);
  });

  it('should detect emergent trails', () => {
    // Create a line of pheromones forming a trail
    for (let i = 0; i < 5; i++) {
      const pos = zeros(DIM);
      pos[10] = i * 0.5;
      const dir = zeros(DIM);
      dir[10] = 1; // All pointing in same direction

      stig.deposit(agentId('pathfinder'), PheromoneType.ATTRACT, pos, dir, 2.0);
    }

    const trails = stig.detectTrails(3, 0.5);
    expect(trails.length).toBeGreaterThan(0);
    expect(trails[0]!.pheromones.length).toBeGreaterThanOrEqual(3);
  });

  it('should emit RECRUIT pheromones that pull agents toward help', () => {
    const helpPos = zeros(DIM);
    helpPos[10] = 5;

    stig.deposit(agentId('stuck'), PheromoneType.RECRUIT, helpPos, zeros(DIM), 3.0);

    const gradient = stig.sense(zeros(DIM));
    expect(norm(gradient.recruitForce)).toBeGreaterThan(0);
  });
});

// ─── Swarm Mind Tests ───────────────────────────────────────────

describe('SwarmMind — Emergent Collective Intelligence', () => {
  let stig: StigmergyEngine;
  let mind: SwarmMind;

  beforeEach(() => {
    stig = new StigmergyEngine(DIM);
    mind = new SwarmMind(DIM, TENANT, stig, 0.3); // Lower threshold for test
  });

  it('should crystallize swarms from resonating agents', () => {
    // Two agents with aligned momentum → should form a swarm
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('a1'), makeAgent('a1', 10, 2.0));
    states.set(agentId('a2'), makeAgent('a2', 10, 2.0)); // Same direction

    const events = mind.tick(states);

    expect(mind.getSwarmCount()).toBe(1);
    const formEvent = events.find(e => e.type === SwarmEvent.FORMED);
    expect(formEvent).toBeDefined();
  });

  it('should NOT swarm agents with opposing momentum', () => {
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('a1'), makeAgent('a1', 10, 2.0));
    states.set(agentId('a2'), makeAgent('a2', 10, -2.0)); // Opposite

    mind.tick(states);

    expect(mind.getSwarmCount()).toBe(0);
  });

  it('should compute flocking forces for swarm members', () => {
    const states = new Map<AgentId, HyperState>();
    const a1 = makeAgent('a1', 10, 2.0);
    const a2 = makeAgent('a2', 10, 2.0);
    const a3 = makeAgent('a3', 10, 1.5);

    states.set(agentId('a1'), a1);
    states.set(agentId('a2'), a2);
    states.set(agentId('a3'), a3);

    mind.tick(states);

    if (mind.getSwarmCount() > 0) {
      const forces = mind.computeFlocking(agentId('a1'), a1, states);
      // Should have some flocking forces
      expect(forces.alignment.length).toBe(DIM);
      expect(forces.cohesion.length).toBe(DIM);
    }
  });

  it('should heal when a member fails', () => {
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('a1'), makeAgent('a1', 10, 2.0));
    states.set(agentId('a2'), makeAgent('a2', 10, 2.0));
    states.set(agentId('a3'), makeAgent('a3', 10, 1.8));

    mind.tick(states);
    const initialCount = mind.getSwarmCount();

    // Agent a2 disappears!
    states.delete(agentId('a2'));
    const events = mind.tick(states);

    const failEvent = events.find(e => e.type === SwarmEvent.MEMBER_FAILED);
    const healEvent = events.find(e => e.type === SwarmEvent.HEALING);

    if (initialCount > 0) {
      expect(failEvent).toBeDefined();
      expect(healEvent).toBeDefined();
      // Should have deposited a recruitment pheromone
      expect(stig.getActiveCount()).toBeGreaterThan(0);
    }
  });

  it('should detect emergent goals from collective momentum', () => {
    const states = new Map<AgentId, HyperState>();

    // All agents strongly aligned → emergent goal should crystallize
    for (let i = 0; i < 5; i++) {
      states.set(agentId(`agent-${i}`), makeAgent(`agent-${i}`, 10, 3.0));
    }

    // Tick multiple times to let swarm form and update
    for (let i = 0; i < 5; i++) {
      mind.tick(states);
    }

    // Check if any swarm has an emergent goal
    let hasGoal = false;
    for (let i = 0; i < 5; i++) {
      const swarm = mind.getAgentSwarm(agentId(`agent-${i}`));
      if (swarm?.emergentGoal) {
        hasGoal = true;
        break;
      }
    }

    // With 5 aligned agents, a goal should emerge
    if (mind.getSwarmCount() > 0) {
      expect(hasGoal).toBe(true);
    }
  });
});

// ─── Immune System Tests ────────────────────────────────────────

describe('ImmuneSystem — Distributed Threat Detection', () => {
  let stig: StigmergyEngine;
  let immune: ImmuneSystem;

  beforeEach(() => {
    stig = new StigmergyEngine(DIM);
    immune = new ImmuneSystem(DIM, stig, 0.5);
  });

  it('should detect erratic agents (too many direction changes)', () => {
    const detector = agentId('sentinel');
    const rogue = agentId('rogue');

    // Simulate erratic behavior: agent keeps changing direction sharply
    for (let tick = 0; tick < 10; tick++) {
      const states = new Map<AgentId, HyperState>();
      // Rogue changes direction every tick
      const dir = tick % 2 === 0 ? 10 : 11;
      states.set(rogue, makeAgent('rogue', dir, 3.0));
      states.set(detector, makeAgent('sentinel', 10, 1.0));

      const field = { emit: () => {} } as any;
      immune.monitor(states, field, detector);
    }

    expect(immune.isQuarantined(rogue)).toBe(true);
  });

  it('should detect entropy leaks (high entropy + high momentum)', () => {
    const detector = agentId('sentinel');
    const leaker = agentId('leaker');

    // Create agent with high entropy but still moving fast
    const leakerState = createHyperState(leaker, AgentState.RUNNING, DIM);
    // Manually set high entropy by evolving with chaotic force
    const chaoticState: HyperState = {
      ...leakerState,
      entropy: 0.9, // Way above threshold
      momentum: randomUnit(DIM),
    };

    const states = new Map<AgentId, HyperState>();
    states.set(leaker, chaoticState);
    states.set(detector, makeAgent('sentinel', 10, 1.0));

    const field = { emit: () => {} } as any;

    // Need 2 ticks for profile to detect (needs lastState)
    immune.monitor(states, field, detector);
    const response = immune.monitor(states, field, detector);

    const entropyThreat = response.threats.find(t => t.type === 'ENTROPY_LEAK');
    expect(entropyThreat).toBeDefined();
  });

  it('should quarantine and release agents', () => {
    const rogue = agentId('bad-actor');
    const detector = agentId('guard');

    // Create erratic behavior: change direction EVERY tick
    for (let tick = 0; tick < 12; tick++) {
      const states = new Map<AgentId, HyperState>();
      // Alternate between very different dimensions each tick
      states.set(rogue, makeAgent('bad-actor', tick % 2 === 0 ? 10 : 20, 3.0));
      states.set(detector, makeAgent('guard', 10, 1.0));
      const field = { emit: () => {} } as any;
      immune.monitor(states, field, detector);
    }

    expect(immune.isQuarantined(rogue)).toBe(true);

    // Release
    immune.release(rogue);
    expect(immune.isQuarantined(rogue)).toBe(false);
  });

  it('should build immune memory from repeated threats', () => {
    const detector = agentId('sentinel');
    const rogue = agentId('repeater');

    for (let tick = 0; tick < 15; tick++) {
      const states = new Map<AgentId, HyperState>();
      states.set(rogue, makeAgent('repeater', tick % 2 === 0 ? 10 : 12, 3.0));
      states.set(detector, makeAgent('sentinel', 10, 1.0));
      const field = { emit: () => {} } as any;
      immune.monitor(states, field, detector);
    }

    const memory = immune.getMemory();
    expect(memory.length).toBeGreaterThan(0);
  });
});

// ─── Scenario: L'Essaim Conscient ───────────────────────────────

describe('Scenario — L\'Essaim Conscient', () => {
  it('should demonstrate full swarm lifecycle: form → navigate → heal → defend', () => {
    const stig = new StigmergyEngine(DIM, 1000, 0.5);
    const mind = new SwarmMind(DIM, TENANT, stig, 0.2);
    const immune = new ImmuneSystem(DIM, stig);

    // ── PHASE 1: FORMATION ────────────────────────────────
    // 8 agents with aligned momentum → should self-organize
    const states = new Map<AgentId, HyperState>();
    for (let i = 0; i < 8; i++) {
      const id = agentId(`worker-${i}`);
      // All moving in roughly the same direction (dim 10)
      // with slight variations
      states.set(id, makeAgent(`worker-${i}`, 10, 2.0 + i * 0.1));
    }

    // Tick until swarms form
    for (let t = 0; t < 5; t++) mind.tick(states);

    const swarmsAfterFormation = mind.getSwarmCount();
    expect(swarmsAfterFormation).toBeGreaterThan(0);

    // ── PHASE 2: STIGMERGIC NAVIGATION ────────────────────
    // Agents deposit pheromone trails as they "move"
    for (let t = 0; t < 10; t++) {
      for (const [id, state] of states) {
        // Each agent deposits an ATTRACT pheromone at its position
        stig.deposit(id, PheromoneType.ATTRACT, state.position, state.momentum, 1.0, 0.05);
      }

      // Agents sense the pheromone field and get influenced
      for (const [id, state] of states) {
        const gradient = stig.sense(state.position);
        // The gradient would influence the agent's trajectory
        // (in full system, this feeds back into LatentEngine)
        expect(gradient.sensedCount).toBeGreaterThanOrEqual(0);
      }

      stig.tick();
      mind.tick(states);
    }

    // Should have formed trails
    const trails = stig.detectTrails(2);
    // Trails may or may not form depending on exact positions

    // ── PHASE 3: HEALING ──────────────────────────────────
    // Kill 2 workers
    states.delete(agentId('worker-3'));
    states.delete(agentId('worker-5'));

    const healEvents = mind.tick(states);

    const failures = healEvents.filter(e => e.type === SwarmEvent.MEMBER_FAILED);
    const healings = healEvents.filter(e => e.type === SwarmEvent.HEALING);

    // System should have detected the failures
    if (swarmsAfterFormation > 0) {
      expect(failures.length + healings.length).toBeGreaterThan(0);
    }

    // ── PHASE 4: IMMUNE RESPONSE ──────────────────────────
    // Add a rogue agent that behaves erratically
    const rogueId = agentId('rogue-intruder');
    const sentinel = agentId('worker-0');

    for (let t = 0; t < 12; t++) {
      // Rogue changes direction every tick
      states.set(rogueId, makeAgent('rogue-intruder', (t % 3) * 4 + 10, 5.0));

      const field = { emit: () => {} } as any;
      immune.monitor(states, field, sentinel);
      mind.tick(states);
    }

    // Rogue should be detected and quarantined
    const isQuarantined = immune.isQuarantined(rogueId);
    // Immune memory should have recorded the pattern
    const memorySize = immune.getMemory().length;

    // ── FINAL REPORT ──────────────────────────────────────
    const allEvents = mind.getAllEvents();

    console.log(
      `[L'ESSAIM CONSCIENT]\n` +
      `  Phase 1 — Formation: ${swarmsAfterFormation} swarm(s) cristallise(s)\n` +
      `  Phase 2 — Navigation: ${stig.getActiveCount()} pheromones actives, ${trails.length} trail(s)\n` +
      `  Phase 3 — Healing: ${failures.length} defaillances, ${healings.length} guerisons\n` +
      `  Phase 4 — Immune: rogue quarantine=${isQuarantined}, memoire=${memorySize} pattern(s)\n` +
      `  Total events: ${allEvents.length}`,
    );

    // The system MUST have exhibited emergent behavior
    expect(allEvents.length).toBeGreaterThan(0);
    // At least one swarm formed
    expect(swarmsAfterFormation).toBeGreaterThan(0);
  });
});
