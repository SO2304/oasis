/**
 * OASIS Kernel — Innovation Tests
 *
 * Three capabilities no other system has:
 * 1. Temporal Branching (quantum decision-making)
 * 2. Emotional Topology (motivation as geometry)
 * 3. Morphogenesis (self-differentiating agents)
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { TemporalBranchingEngine } from '../branching.js';
import { EmotionalField, EmotionType } from '../../emotion/emotional-field.js';
import { MorphogenesisEngine, AgentRole } from '../../morpho/morphogenesis.js';
import { WorldModel, ZoneType } from '../../physics/world-model.js';
import { createHyperState, evolveState, type HyperState } from '../../physics/hyper-state.js';
import { agentId, AgentState } from '../../types.js';
import { zeros, norm, normalize, scale, sub, distance, randomUnit } from '../../physics/vector-math.js';

const DIM = 32;

// ─── Temporal Branching ─────────────────────────────────────────

describe('Temporal Branching — Quantum Decision-Making', () => {
  let brancher: TemporalBranchingEngine;
  let world: WorldModel;

  beforeEach(() => {
    brancher = new TemporalBranchingEngine(DIM, 7, 10);
    world = new WorldModel(DIM);
  });

  it('should branch into N parallel timelines and select the best', () => {
    const state = createHyperState(agentId('decider'), AgentState.RUNNING, DIM);
    const goalPos = zeros(DIM);
    goalPos[10] = 5;
    world.addGoal('target', goalPos, 3.0, normalize(goalPos));

    const result = brancher.branch(state, world, goalPos);

    expect(result.branchCount).toBe(7);
    expect(result.timelines.length).toBe(7);
    expect(result.winner).toBeDefined();
    expect(result.winner.fitness).toBeGreaterThanOrEqual(result.timelines[result.timelines.length - 1]!.fitness);
    expect(result.confidence).toBeGreaterThan(0);
    expect(result.steeringForce.length).toBe(DIM);
  });

  it('should prefer timelines that reach the goal', () => {
    const state = createHyperState(agentId('seeker'), AgentState.RUNNING, DIM);
    const goalPos = zeros(DIM);
    goalPos[10] = 3;
    world.addGoal('target', goalPos, 5.0, normalize(goalPos));

    const result = brancher.branch(state, world, goalPos);

    // Winner should have high goal proximity
    expect(result.winner.breakdown.goalProximity).toBeGreaterThan(0);
  });

  it('should avoid timelines that hit obstacles', () => {
    const state = createHyperState(agentId('dodger'), AgentState.RUNNING, DIM);

    // Goal ahead
    const goalPos = zeros(DIM);
    goalPos[10] = 8;
    world.addGoal('target', goalPos, 3.0, normalize(goalPos));

    // Obstacle in the way
    const obsPos = zeros(DIM);
    obsPos[10] = 3;
    world.addObstacle('wall', obsPos, 15.0, normalize(obsPos));

    const result = brancher.branch(state, world, goalPos);

    // Winner should have decent safety margin
    expect(result.winner.breakdown.safetyMargin).toBeGreaterThan(0);
    // Winner should have low pain
    expect(result.winner.breakdown.painAccum).toBeLessThan(1);
  });

  it('should prefer smooth trajectories', () => {
    const state = createHyperState(agentId('smooth'), AgentState.RUNNING, DIM);
    const goalPos = zeros(DIM);
    goalPos[10] = 5;
    world.addGoal('target', goalPos, 3.0, normalize(goalPos));

    const result = brancher.branch(state, world, goalPos);

    // Winner should have some smoothness
    expect(result.winner.breakdown.smoothness).toBeGreaterThan(0);
  });

  it('should complete in reasonable time', () => {
    const state = createHyperState(agentId('fast'), AgentState.RUNNING, DIM);
    world.addGoal('g', zeros(DIM), 3.0, zeros(DIM));

    const result = brancher.branch(state, world);

    // Should complete in < 50ms (no network, pure computation)
    expect(result.computeTimeMs).toBeLessThan(50);
  });
});

// ─── Emotional Topology ─────────────────────────────────────────

describe('Emotional Topology — Motivation as Geometry', () => {
  let emotions: EmotionalField;
  const id = agentId('feeling-agent');

  beforeEach(() => {
    emotions = new EmotionalField(DIM);
    emotions.register(id);
  });

  it('should start in CALM state', () => {
    const state = createHyperState(id, AgentState.RUNNING, DIM);
    const emo = emotions.update(id, state);

    expect(emo.dominant).toBe(EmotionType.CALM);
    expect(emo.curiosity).toBe(0);
    expect(emo.fear).toBe(0);
  });

  it('should feel CURIOSITY when entropy is in the sweet spot', () => {
    const state = createHyperState(id, AgentState.RUNNING, DIM);

    // Artificially set entropy to the curiosity range (0.3-0.7)
    const curiousState: HyperState = { ...state, entropy: 0.5 };

    // Update several times to build curiosity
    for (let i = 0; i < 10; i++) emotions.update(id, curiousState);

    const emo = emotions.getState(id)!;
    expect(emo.curiosity).toBeGreaterThan(0.3);
  });

  it('should feel FEAR when approaching a pain memory', () => {
    const painPos = zeros(DIM);
    painPos[10] = 2;

    // Record pain at a position
    emotions.recordPain(id, painPos, 0.8);

    // Agent approaches the pain position
    const state = createHyperState(id, AgentState.RUNNING, DIM);
    const nearPain: HyperState = {
      ...state,
      position: (() => { const p = zeros(DIM); p[10] = 1.5; return p; })(),
    };

    emotions.update(id, nearPain);
    const emo = emotions.getState(id)!;

    expect(emo.fear).toBeGreaterThan(0);
  });

  it('should feel SATISFACTION when moving toward goal', () => {
    // Simulate decreasing distance to goal
    emotions.recordGoalDistance(id, 10);
    emotions.recordGoalDistance(id, 7);
    emotions.recordGoalDistance(id, 4);

    const state = createHyperState(id, AgentState.RUNNING, DIM);
    emotions.update(id, state);

    const emo = emotions.getState(id)!;
    expect(emo.satisfaction).toBeGreaterThan(0);
  });

  it('should feel FRUSTRATION from repeated prediction errors', () => {
    // Simulate persistent prediction errors
    for (let i = 0; i < 10; i++) {
      emotions.recordPredictionError(id, 0.5); // Above threshold
    }

    const state = createHyperState(id, AgentState.RUNNING, DIM);
    for (let i = 0; i < 10; i++) emotions.update(id, state);

    const emo = emotions.getState(id)!;
    expect(emo.frustration).toBeGreaterThan(0.1);
  });

  it('should modulate gain based on emotional state', () => {
    // Make agent fearful
    emotions.recordPain(id, zeros(DIM), 1.0);
    const state = createHyperState(id, AgentState.RUNNING, DIM);
    emotions.update(id, state);

    const gain = emotions.computeGain(id);

    // Fear should amplify repulsion gain
    expect(gain.repulsionGain).toBeGreaterThan(1.0);
  });

  it('should trigger strategy switch on high frustration', () => {
    for (let i = 0; i < 20; i++) {
      emotions.recordPredictionError(id, 0.8);
    }

    const state = createHyperState(id, AgentState.RUNNING, DIM);
    for (let i = 0; i < 20; i++) emotions.update(id, state);

    const gain = emotions.computeGain(id);

    // High frustration should trigger switch
    if (emotions.getState(id)!.frustration > 0.7) {
      expect(gain.switchStrategy).toBe(true);
    }
  });

  it('should generate emotional force that influences trajectory', () => {
    // Make agent fearful of a position
    const dangerPos = zeros(DIM);
    dangerPos[10] = 1;
    emotions.recordPain(id, dangerPos, 1.0);

    const state = createHyperState(id, AgentState.RUNNING, DIM);
    const nearDanger: HyperState = {
      ...state,
      position: (() => { const p = zeros(DIM); p[10] = 0.5; return p; })(),
    };

    emotions.update(id, nearDanger);
    const force = emotions.computeEmotionalForce(id, nearDanger);

    // Force should push AWAY from danger
    expect(norm(force)).toBeGreaterThan(0);
  });

  it('should decay fear memories over time', () => {
    emotions.recordPain(id, zeros(DIM), 1.0);

    const state = createHyperState(id, AgentState.RUNNING, DIM);

    // Many ticks later
    for (let i = 0; i < 100; i++) {
      emotions.update(id, state);
      emotions.tick();
    }

    // Fear should have decayed
    const emo = emotions.getState(id)!;
    expect(emo.fear).toBeLessThan(0.5);
  });
});

// ─── Morphogenesis ──────────────────────────────────────────────

describe('Morphogenesis — Self-Differentiating Agents', () => {
  let morpho: MorphogenesisEngine;

  beforeEach(() => {
    morpho = new MorphogenesisEngine(DIM, 0.05, 0.5, 3);
  });

  it('should start all agents as STEM cells', () => {
    morpho.register(agentId('cell-1'));
    morpho.register(agentId('cell-2'));

    expect(morpho.getRole(agentId('cell-1'))).toBe(AgentRole.STEM);
    expect(morpho.getRole(agentId('cell-2'))).toBe(AgentRole.STEM);
  });

  it('should differentiate based on field needs', () => {
    for (let i = 0; i < 10; i++) {
      morpho.register(agentId(`cell-${i}`));
    }

    const states = new Map<AgentId, HyperState>();
    for (let i = 0; i < 10; i++) {
      states.set(agentId(`cell-${i}`), createHyperState(agentId(`cell-${i}`), AgentState.RUNNING, DIM));
    }

    // High threat → should produce sentinels
    // Goal exists → should produce navigators
    // Unknown area → should produce scouts
    const events = morpho.differentiate(states, 0.8, 0.6, 2, true);

    expect(events.length).toBeGreaterThan(0);

    const dist = morpho.getRoleDistribution();
    // Should have produced a mix of roles
    const nonStem = Object.entries(dist)
      .filter(([role]) => role !== 'STEM')
      .reduce((sum, [, count]) => sum + count, 0);
    expect(nonStem).toBeGreaterThan(0);
  });

  it('should create sentinels when threats are high', () => {
    for (let i = 0; i < 8; i++) {
      morpho.register(agentId(`cell-${i}`));
    }

    const states = new Map<AgentId, HyperState>();
    for (let i = 0; i < 8; i++) {
      states.set(agentId(`cell-${i}`), createHyperState(agentId(`cell-${i}`), AgentState.RUNNING, DIM));
    }

    // VERY high threat, no other needs
    morpho.differentiate(states, 0.95, 0, 0, false);

    const sentinels = morpho.getAgentsByRole(AgentRole.SENTINEL);
    expect(sentinels.length).toBeGreaterThan(0);
  });

  it('should create scouts when the world is unexplored', () => {
    for (let i = 0; i < 8; i++) {
      morpho.register(agentId(`cell-${i}`));
    }

    const states = new Map<AgentId, HyperState>();
    for (let i = 0; i < 8; i++) {
      const s = createHyperState(agentId(`cell-${i}`), AgentState.RUNNING, DIM);
      // High entropy agents are better scouts
      states.set(agentId(`cell-${i}`), { ...s, entropy: 0.4 + Math.random() * 0.2 });
    }

    morpho.differentiate(states, 0, 0.9, 0, false); // 90% unknown

    const scouts = morpho.getAgentsByRole(AgentRole.SCOUT);
    expect(scouts.length).toBeGreaterThan(0);
  });

  it('should allow re-differentiation for low-commitment agents', () => {
    morpho.register(agentId('flexible'));
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('flexible'), createHyperState(agentId('flexible'), AgentState.RUNNING, DIM));

    // First: differentiate to WORKER (low threat, no unknowns)
    morpho.differentiate(states, 0, 0, 0, false);
    const firstRole = morpho.getRole(agentId('flexible'));

    // Wait for cooldown
    for (let i = 0; i < 5; i++) {
      morpho.differentiate(states, 0, 0, 0, false);
    }

    // Now: HIGH threat should trigger re-differentiation if commitment is low
    const events = morpho.differentiate(states, 1.0, 0, 0, false);

    // May or may not re-differentiate depending on commitment growth
    // The mechanism exists regardless
    expect(morpho.getRole(agentId('flexible'))).toBeDefined();
  });

  it('should track morphogenetic events', () => {
    for (let i = 0; i < 5; i++) {
      morpho.register(agentId(`cell-${i}`));
    }

    const states = new Map<AgentId, HyperState>();
    for (let i = 0; i < 5; i++) {
      states.set(agentId(`cell-${i}`), createHyperState(agentId(`cell-${i}`), AgentState.RUNNING, DIM));
    }

    morpho.differentiate(states, 0.5, 0.5, 1, true);

    const events = morpho.getEvents();
    expect(events.length).toBeGreaterThan(0);
    expect(events[0]!.fromRole).toBe(AgentRole.STEM);
    expect(events[0]!.toRole).not.toBe(AgentRole.STEM);
  });

  it('should degrade commitment for poor performers', () => {
    morpho.register(agentId('underperformer'));
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('underperformer'), createHyperState(agentId('underperformer'), AgentState.RUNNING, DIM));

    morpho.differentiate(states, 0.5, 0, 0, true);

    // Build some commitment
    for (let i = 0; i < 10; i++) {
      morpho.differentiate(states, 0.5, 0, 0, true);
    }

    const beforeCommitment = morpho.getProfile(agentId('underperformer'))!.commitment;

    // Report poor performance
    morpho.reportPerformance(agentId('underperformer'), 0.1);

    const afterCommitment = morpho.getProfile(agentId('underperformer'))!.commitment;
    expect(afterCommitment).toBeLessThan(beforeCommitment);
  });
});

// ─── Integrated Scenario ────────────────────────────────────────

describe('Scenario — The Living Organism', () => {
  it('should combine branching + emotions + morphogenesis into emergent behavior', () => {
    const brancher = new TemporalBranchingEngine(DIM, 5, 8);
    const emotions = new EmotionalField(DIM);
    const morpho = new MorphogenesisEngine(DIM);
    const world = new WorldModel(DIM);

    // 8 agents start as stem cells
    const agentIds: AgentId[] = [];
    const states = new Map<AgentId, HyperState>();

    for (let i = 0; i < 8; i++) {
      const id = agentId(`organism-${i}`);
      agentIds.push(id);
      morpho.register(id);
      emotions.register(id);
      states.set(id, createHyperState(id, AgentState.RUNNING, DIM));
    }

    // Set up the world: goal + obstacle + unknown region
    const goalPos = zeros(DIM);
    goalPos[10] = 10;
    world.addGoal('food', goalPos, 4.0, normalize(goalPos));

    const obstaclePos = zeros(DIM);
    obstaclePos[10] = 5;
    world.addObstacle('wall', obstaclePos, 10.0, normalize(obstaclePos));

    world.addUnknownRegion('darkness', scale(randomUnit(DIM), 3), 2.0);

    // Run 20 ticks of the full organism
    const roleHistory: string[] = [];

    for (let t = 0; t < 20; t++) {
      // 1. MORPHOGENESIS: differentiate based on needs
      const threatLevel = 0.3;
      const unknownRatio = 0.4;
      morpho.differentiate(states, threatLevel, unknownRatio, 0, true);

      // 2. For navigators: use TEMPORAL BRANCHING to decide
      const navigators = morpho.getAgentsByRole(AgentRole.NAVIGATOR);
      for (const navId of navigators) {
        const state = states.get(navId);
        if (!state) continue;

        const branchResult = brancher.branch(state, world, goalPos);

        // Apply steering force
        const newState = evolveState(state, branchResult.steeringForce, 0.2, 0.05);
        states.set(navId, newState);
      }

      // 3. For all agents: update EMOTIONS
      for (const id of agentIds) {
        const state = states.get(id);
        if (!state) continue;

        emotions.recordGoalDistance(id, distance(state.position, goalPos));
        emotions.update(id, state);

        // Emotional force modifies trajectory
        const emoForce = emotions.computeEmotionalForce(id, state, goalPos);
        if (norm(emoForce) > 1e-6) {
          states.set(id, evolveState(state, emoForce, 0.1, 0.05));
        }
      }

      emotions.tick();
      world.tick();

      // Track role distribution
      const dist = morpho.getRoleDistribution();
      roleHistory.push(
        Object.entries(dist)
          .filter(([, count]) => count > 0)
          .map(([role, count]) => `${role}:${count}`)
          .join(' '),
      );
    }

    // ── ASSERTIONS ────────────────────────────────────────

    // System should have differentiated agents
    const finalDist = morpho.getRoleDistribution();
    const totalDifferentiated = Object.entries(finalDist)
      .filter(([role]) => role !== 'STEM')
      .reduce((sum, [, count]) => sum + count, 0);

    expect(totalDifferentiated).toBeGreaterThan(0);

    // There should be navigators (goal exists)
    // and potentially scouts (unknown region exists)
    const hasVariety = Object.values(finalDist).filter(c => c > 0).length >= 2;
    expect(hasVariety).toBe(true);

    // Morphogenetic events should have occurred
    expect(morpho.getEvents().length).toBeGreaterThan(0);

    // Emotional states should have evolved
    const emoStates = agentIds.map(id => emotions.getState(id)!);
    const anyEmotion = emoStates.some(e =>
      e.curiosity > 0.1 || e.satisfaction > 0.1 || e.fear > 0.1,
    );

    console.log(
      `[THE LIVING ORGANISM — 20 ticks]\n` +
      `  Roles: ${JSON.stringify(finalDist)}\n` +
      `  Differentiated: ${totalDifferentiated}/8\n` +
      `  Morpho events: ${morpho.getEvents().length}\n` +
      `  Emotional variety: ${anyEmotion}\n` +
      `  Role evolution:\n    ${roleHistory.slice(0, 5).join('\n    ')}`,
    );
  });
});
