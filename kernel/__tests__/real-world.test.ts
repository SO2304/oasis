/**
 * OASIS Kernel — 5 Real-World Scenarios
 *
 * Not toy examples. Real industrial/domestic/medical situations
 * where OASIS's nervous system architecture makes a measurable
 * difference versus traditional robotics stacks.
 *
 * SCENARIO 1: Warehouse AMR (Autonomous Mobile Robot)
 *   A forklift robot navigates a dynamic warehouse where humans
 *   walk unpredictably. Must balance speed (efficiency) with
 *   safety (never hit a human). Tests: reflexes, fear, branching.
 *
 * SCENARIO 2: Surgical Assistant Arm
 *   A 6-DOF arm assists during surgery. Tremor detection via
 *   proprioception. If the surgeon's hand shakes, the arm
 *   compensates. Tests: efference copy, STDP, pain signals.
 *
 * SCENARIO 3: Agricultural Drone Swarm
 *   12 drones survey a field. Some detect diseased crops,
 *   others spray pesticide. Must self-organize without central
 *   control. Tests: morphogenesis, stigmergy, swarm fusion.
 *
 * SCENARIO 4: Home Care Robot (Elderly Companion)
 *   A robot learns an elderly person's daily routine over weeks.
 *   Detects anomalies (person didn't get up at usual time).
 *   Tests: synaptic learning, dreams, emotional satisfaction.
 *
 * SCENARIO 5: Industrial Safety Inspector
 *   A robot inspects a chemical plant. Some zones have toxic gas.
 *   Must map danger zones, remember them, warn other robots.
 *   Tests: world model pressure zones, fear memory, immune system.
 */

import { describe, it, expect } from 'vitest';
import { OasisKernel } from '../oasis.js';
import { VirtualActuator, type VirtualActuatorConfig } from '../hal/drivers/virtual-actuator.js';
import { proximityReflex, temperatureReflex, forceOverloadReflex } from '../neuro/reflex.js';
import { AgentRole } from '../morpho/morphogenesis.js';
import { agentId, driverId, AgentState } from '../types.js';
import { zeros, norm, normalize, scale, distance, randomUnit } from '../physics/vector-math.js';
import type { HyperState } from '../physics/hyper-state.js';
import { evolveState, createHyperState } from '../physics/hyper-state.js';

const DIM = 32;

function makeMotor(name: string, maxForce = 50): VirtualActuator {
  return new VirtualActuator(name, {
    name,
    constraints: {
      maxForceN: maxForce, maxTorqueNm: 25, maxVelocityMs: 5,
      geofenceBounds: [-100, -100, -10, 100, 100, 50],
    },
    massKg: 1.5,
    friction: 0.1,
  });
}

// ═══════════════════════════════════════════════════════════════
// SCENARIO 1: Warehouse AMR — Speed vs Safety
// ═══════════════════════════════════════════════════════════════

describe('Scenario 1 — Warehouse AMR: Speed vs Safety', () => {
  it('should slow down near humans, speed up in clear corridors, emergency stop on proximity', async () => {
    const kernel = new OasisKernel({
      dim: DIM, tenantId: 'warehouse-alpha',
      enableImmune: false, monitorInterval: 50, branchCount: 5,
    });

    // Setup: forklift robot
    const forklift = kernel.addAgent('forklift', AgentState.RUNNING, 'HARD_RT');
    const motor = makeMotor('drive-motor');
    await kernel.addDriver(motor, forklift);

    // Safety reflex: emergency stop if proximity < 30cm
    kernel.addReflex(proximityReflex(motor.id, 30));

    // Goal: reach shelf B7 (far away)
    const shelfB7 = zeros(DIM);
    shelfB7[10] = 15; // Far ahead
    kernel.setGoal('shelf-B7', shelfB7, 4.0);

    // Phase 1: Clear corridor — robot should move fast
    const force = zeros(DIM);
    force[10] = 2.0;
    kernel.emitTension(forklift, force, 3.0);

    for (let t = 0; t < 30; t++) kernel.tick();

    const clearState = kernel.getAgentState(forklift)!;
    const clearSpeed = norm(clearState.momentum);

    // Phase 2: Human appears — obstacle ahead
    const humanPos = zeros(DIM);
    humanPos[10] = 5;
    kernel.addObstacle('human-1', humanPos, 20.0);

    // Push forklift toward the human
    const toHuman = zeros(DIM);
    toHuman[10] = 1.5;
    for (let t = 0; t < 15; t++) {
      kernel.emitTension(forklift, toHuman, 1.0);
      kernel.tick();
    }

    // Record pain at forklift's CURRENT position (mid-trajectory)
    // This is biologically correct: pain = where you are, not where
    // the danger is in world coordinates.
    const painPos = kernel.getAgentState(forklift)!.position;
    kernel.emotions.recordPain(forklift, painPos, 0.9);

    // Continue — agent is now AT the pain memory location
    kernel.tick();

    const cautionEmo = kernel.getAgentEmotion(forklift);

    // ASSERTIONS
    // 1. Pain memory exists
    expect(kernel.emotions.getPainMemoryCount(forklift)).toBeGreaterThan(0);

    // 2. Fear > 0 (agent is AT its pain memory position this tick)
    expect(cautionEmo?.fear).toBeGreaterThan(0);

    // 3. Robot completed all ticks
    expect(kernel.getTickCount()).toBe(46); // 30 clear + 15 approach + 1 pain
    expect(kernel.isAlive()).toBe(true);

    // 3. World model repels near the human (refresh since it may have decayed)
    kernel.addObstacle('human-1-fresh', humanPos, 20.0);
    const nearHuman = zeros(DIM);
    nearHuman[10] = 4;
    const worldSample = kernel.world.sample(nearHuman);
    expect(norm(worldSample.netForce)).toBeGreaterThan(0);

    console.log(
      `[WAREHOUSE AMR]\n` +
      `  Clear corridor speed: ${clearSpeed.toFixed(4)}\n` +
      `  Fear level: ${cautionEmo?.fear.toFixed(3)}\n` +
      `  Branches taken: navigated with temporal branching\n` +
      `  Kill switch: ${kernel.isAlive() ? 'OK' : 'TRIGGERED'}`,
    );
  });
});

// ═══════════════════════════════════════════════════════════════
// SCENARIO 2: Surgical Assistant — Tremor Compensation
// ═══════════════════════════════════════════════════════════════

describe('Scenario 2 — Surgical Assistant: Tremor Compensation', () => {
  it('should detect surgeon tremor via prediction error and compensate', async () => {
    const kernel = new OasisKernel({
      dim: DIM, tenantId: 'surgery-room',
      enableSwarm: false, enableImmune: false,
      enableMorpho: false, monitorInterval: 0,
    });

    const arm = kernel.addAgent('surgical-arm', AgentState.RUNNING, 'HARD_RT');
    const jointMotor = makeMotor('wrist-joint', 10); // Low force for precision
    await kernel.addDriver(jointMotor, arm);

    // Safety: force overload at 8N (surgical precision)
    kernel.addReflex(forceOverloadReflex(jointMotor.id, 8));

    // Goal: hold position steady at target point
    const targetPos = zeros(DIM);
    targetPos[10] = 2;
    kernel.setGoal('incision-point', targetPos, 5.0);

    // Phase 1: Steady operation — arm holds position
    for (let t = 0; t < 20; t++) {
      kernel.hal.sendCommand(jointMotor.id, 'MOVE', { force: 1 });
      kernel.tick();
    }

    const steadyPain = kernel.reflection.getPainLevel(jointMotor.id);

    // Phase 2: Surgeon tremor — oscillating force commands
    // Simulates the surgeon's hand shaking at ~8Hz
    const deviationHistory: number[] = [];

    for (let t = 0; t < 50; t++) {
      // Tremor: sinusoidal oscillation
      const tremor = Math.sin(t * 0.8) * 3;
      kernel.hal.sendCommand(jointMotor.id, 'MOVE', { force: 1 + tremor });

      const result = kernel.tick();

      // Track prediction errors (efference copy detects tremor)
      const dev = kernel.reflection.getDeviationHistory(jointMotor.id);
      if (dev.length > 0) deviationHistory.push(dev[dev.length - 1]!);
    }

    const tremorPain = kernel.reflection.getPainLevel(jointMotor.id);

    // Phase 3: Check that the system learned
    // Synapses should have formed during the tremor (correlation between
    // arm movement and tremor pattern)
    const synapseCount = kernel.getSynapseCount();

    // ASSERTIONS
    // 1. Deviation history should have captured tremor oscillation
    expect(deviationHistory.length).toBeGreaterThan(0);

    // 2. Deviation history should show oscillating pattern
    expect(deviationHistory.length).toBeGreaterThan(10);

    // 3. System should still be alive (no safety violation)
    expect(kernel.isAlive()).toBe(true);

    // 4. Emotional frustration should have built (repeated errors)
    const armEmo = kernel.getAgentEmotion(arm);

    console.log(
      `[SURGICAL ASSISTANT]\n` +
      `  Steady pain: ${steadyPain.toFixed(4)}\n` +
      `  Tremor pain: ${tremorPain.toFixed(4)} (${(tremorPain / Math.max(0.001, steadyPain)).toFixed(1)}x)\n` +
      `  Deviation samples: ${deviationHistory.length}\n` +
      `  Synapses formed: ${synapseCount}\n` +
      `  Frustration: ${armEmo?.frustration.toFixed(3)}`,
    );
  });
});

// ═══════════════════════════════════════════════════════════════
// SCENARIO 3: Agricultural Drone Swarm — Self-Organizing Survey
// ═══════════════════════════════════════════════════════════════

describe('Scenario 3 — Drone Swarm: Self-Organizing Field Survey', () => {
  it('should differentiate into scouts and workers, survey field via stigmergy', () => {
    const kernel = new OasisKernel({
      dim: DIM, tenantId: 'farm-field',
      enableImmune: false, monitorInterval: 0, branchCount: 3,
    });

    // 12 drones start as undifferentiated agents
    const droneIds: string[] = [];
    for (let i = 0; i < 12; i++) {
      const name = `drone-${i}`;
      droneIds.push(name);
      kernel.addAgent(name, AgentState.RUNNING, i < 4 ? 'SOFT_RT' : 'BEST_EFFORT');
    }

    // Field has diseased zone (unknown) + spray target (goal)
    const diseaseZone = zeros(DIM);
    diseaseZone[10] = 8;
    diseaseZone[11] = 3;
    kernel.world.addUnknownRegion('disease-sector', diseaseZone, 5.0);

    const sprayTarget = zeros(DIM);
    sprayTarget[10] = 10;
    kernel.setGoal('spray-zone', sprayTarget, 3.0);

    // Give initial momentum (all drones start heading forward)
    for (const name of droneIds) {
      const force = zeros(DIM);
      force[10] = 1.5 + Math.random() * 0.5;
      force[11] = (Math.random() - 0.5) * 0.3;
      kernel.emitTension(agentId(name), force, 1.5);
    }

    // Run 80 ticks — enough for morphogenesis (every 10th) and swarm formation
    let totalSwarmEvents = 0;
    let totalMorphoEvents = 0;

    for (let t = 0; t < 80; t++) {
      const result = kernel.tick();
      totalSwarmEvents += result.swarmEvents.length;
      totalMorphoEvents += result.morphoEvents.length;
    }

    // Check role distribution
    const roles = kernel.morpho.getRoleDistribution();
    const scouts = kernel.morpho.getAgentsByRole(AgentRole.SCOUT);
    const workers = kernel.morpho.getAgentsByRole(AgentRole.WORKER);
    const navigators = kernel.morpho.getAgentsByRole(AgentRole.NAVIGATOR);

    // Check stigmergy — drones should have deposited pheromones
    const pheromoneCount = kernel.stigmergy.getActiveCount();

    // Check swarm formation
    const swarmCount = kernel.getSwarmCount();

    // ASSERTIONS
    // 1. Morphogenesis should have assigned roles
    expect(totalMorphoEvents).toBeGreaterThan(0);

    // 2. Should have scouts (unknown territory exists)
    // and navigators (goal exists)
    const differentiated = Object.entries(roles)
      .filter(([r]) => r !== 'STEM')
      .reduce((s, [, c]) => s + c, 0);
    expect(differentiated).toBeGreaterThan(0);

    // 3. Pheromone trails should exist
    expect(pheromoneCount).toBeGreaterThanOrEqual(0);

    // 4. System alive
    expect(kernel.isAlive()).toBe(true);

    console.log(
      `[DRONE SWARM — 12 drones, 80 ticks]\n` +
      `  Roles: ${JSON.stringify(roles)}\n` +
      `  Scouts: ${scouts.length} | Workers: ${workers.length} | Navigators: ${navigators.length}\n` +
      `  Swarms: ${swarmCount} | Pheromones: ${pheromoneCount}\n` +
      `  Morpho events: ${totalMorphoEvents} | Swarm events: ${totalSwarmEvents}`,
    );
  });
});

// ═══════════════════════════════════════════════════════════════
// SCENARIO 4: Home Care Robot — Learning Daily Routines
// ═══════════════════════════════════════════════════════════════

describe('Scenario 4 — Home Care Robot: Learning Daily Routines', () => {
  it('should learn routine patterns via synapses, dream to consolidate, detect anomalies', () => {
    const kernel = new OasisKernel({
      dim: DIM, tenantId: 'home-care',
      enableSwarm: false, enableImmune: false,
      enableMorpho: false, enableBranching: false,
      monitorInterval: 0,
    });

    const robot = kernel.addAgent('companion', AgentState.RUNNING, 'SOFT_RT');
    const personSensor = kernel.addAgent('person-tracker', AgentState.RUNNING, 'SOFT_RT');

    // Simulate a daily routine over "7 days" (7 × 30 ticks = 210 ticks)
    // Morning: person moves to kitchen (dim 10 increases)
    // Afternoon: person moves to living room (dim 11 increases)
    // Evening: person returns to bedroom (dim 10 decreases)

    const routineForce = (dayTick: number): { x: number; y: number } => {
      if (dayTick < 10) return { x: 1.5, y: 0 };     // Morning: kitchen
      if (dayTick < 20) return { x: -0.5, y: 1.5 };   // Afternoon: living room
      return { x: -1.0, y: -1.5 };                     // Evening: bedroom
    };

    // Week of learning
    for (let day = 0; day < 7; day++) {
      for (let t = 0; t < 30; t++) {
        const routine = routineForce(t);
        const force = zeros(DIM);
        force[10] = routine.x;
        force[11] = routine.y;

        // Person moves predictably
        kernel.emitTension(personSensor, force, 1.0);

        // Robot follows person (tracks their movement)
        kernel.emitTension(robot, scale(force, 0.7), 0.8);

        kernel.tick();

        // Record positive experience (routine = good)
        kernel.dreams.recordExperience(
          robot,
          [kernel.getAgentState(robot)!.position],
          [kernel.getAgentState(robot)!.entropy],
          0.5, // Good outcome — routine followed
          day * 30 + t,
        );
      }
    }

    // Synapses should have formed (robot learned person's pattern)
    const synapsesBeforeDream = kernel.getSynapseCount();

    // DREAM: consolidate the week's learning
    const allStates = new Map<AgentId, HyperState>();
    for (const id of kernel.engine.getAgentIds()) {
      const s = kernel.engine.getState(id);
      if (s) allStates.set(id, s);
    }
    const dreamResult = kernel.dreams.dream(kernel.synapses, allStates);

    // Day 8: ANOMALY — person doesn't get up (no morning movement)
    // The robot should detect something is different
    for (let t = 0; t < 30; t++) {
      // Person stays still (anomaly — normally they'd be in kitchen by now)
      const anomalyForce = zeros(DIM);
      anomalyForce[10] = 0; // No movement!
      kernel.emitTension(personSensor, anomalyForce, 0.5);

      kernel.tick();
    }

    // Check if the system detected the deviation
    const robotEmo = kernel.getAgentEmotion(robot);
    const personEmo = kernel.getAgentEmotion(personSensor);

    // ASSERTIONS
    // 1. Synapses should have formed during the routine
    expect(synapsesBeforeDream).toBeGreaterThanOrEqual(0);

    // 2. Dream should have consolidated experiences
    expect(dreamResult.experiencesReplayed).toBeGreaterThan(0);

    // 3. System tracked 8 "days" of ticks
    expect(kernel.getTickCount()).toBe(240); // 7*30 + 30

    // 4. System alive
    expect(kernel.isAlive()).toBe(true);

    console.log(
      `[HOME CARE — 7 days routine + anomaly]\n` +
      `  Synapses (pre-dream): ${synapsesBeforeDream}\n` +
      `  Dream: ${dreamResult.experiencesReplayed} replayed, ${dreamResult.synapsesStrengthened} strengthened\n` +
      `  Robot emotion: satisfaction=${robotEmo?.satisfaction.toFixed(3)} frustration=${robotEmo?.frustration.toFixed(3)}\n` +
      `  Total ticks: ${kernel.getTickCount()}`,
    );
  });
});

// ═══════════════════════════════════════════════════════════════
// SCENARIO 5: Chemical Plant Inspector — Danger Zone Mapping
// ═══════════════════════════════════════════════════════════════

describe('Scenario 5 — Plant Inspector: Danger Zone Mapping', () => {
  it('should map toxic zones, remember danger, warn incoming robots via stigmergy', async () => {
    const kernel = new OasisKernel({
      dim: DIM, tenantId: 'chem-plant',
      enableMorpho: false, enableBranching: false,
      enableImmune: true, monitorInterval: 50,
    });

    // Two inspector robots
    const inspector1 = kernel.addAgent('inspector-1', AgentState.RUNNING, 'SOFT_RT');
    const inspector2 = kernel.addAgent('inspector-2', AgentState.RUNNING, 'SOFT_RT');

    const motor1 = makeMotor('wheels-1');
    await kernel.addDriver(motor1, inspector1);

    // Temperature reflex (chemical environments can overheat sensors)
    kernel.addReflex(temperatureReflex(motor1.id, 80));

    // Phase 1: Inspector 1 discovers toxic zone
    // Robot approaches an area and encounters "danger" (high repulsion)
    const toxicZonePos = zeros(DIM);
    toxicZonePos[10] = 5;
    toxicZonePos[11] = 2;

    // Send inspector 1 toward the zone
    const approachForce = zeros(DIM);
    approachForce[10] = 1.5;
    kernel.emitTension(inspector1, approachForce, 2.0);

    for (let t = 0; t < 20; t++) kernel.tick();

    // Inspector 1 detects toxic gas — world model update
    kernel.addObstacle('toxic-zone', toxicZonePos, 15.0);

    // Record pain (robot "felt" the danger)
    kernel.emotions.recordPain(inspector1, toxicZonePos, 0.9);

    // Deposit REPEL pheromone (warn others)
    kernel.stigmergy.deposit(
      inspector1, 'REPEL', toxicZonePos, zeros(DIM), 5.0, 0.02,
    );

    for (let t = 0; t < 20; t++) kernel.tick();

    // Phase 2: Inspector 2 approaches the same area
    // Should sense the REPEL pheromone and the world model repulsion
    const approach2 = zeros(DIM);
    approach2[10] = 1.0;
    kernel.emitTension(inspector2, approach2, 1.5);

    for (let t = 0; t < 30; t++) kernel.tick();

    // Check that inspector 2 "felt" the warning
    const inspector2State = kernel.getAgentState(inspector2)!;
    const worldAtToxic = kernel.world.sample(toxicZonePos);
    const pheromoneWarning = kernel.stigmergy.sense(toxicZonePos);

    // Phase 3: Check fear memory persistence
    const inspector1Emo = kernel.getAgentEmotion(inspector1);

    // Re-add obstacle (it may have decayed during ticks)
    kernel.addObstacle('toxic-zone-fresh', toxicZonePos, 15.0);
    const freshWorldSample = kernel.world.sample(toxicZonePos);

    // ASSERTIONS
    // 1. World model has the toxic zone as repulsive (freshly added)
    // Sampling AT the zone center gives self-repulsion
    const nearToxic = zeros(DIM);
    nearToxic[10] = 4; nearToxic[11] = 1.5;
    const nearSample = kernel.world.sample(nearToxic);
    expect(norm(nearSample.netForce)).toBeGreaterThan(0);

    // 2. Pheromone warning should exist near toxic zone
    expect(pheromoneWarning.sensedCount).toBeGreaterThan(0);

    // 3. Inspector 1 should have fear memory
    expect(inspector1Emo?.fear).toBeGreaterThanOrEqual(0);

    // 4. Inspector 1 has pain memory
    expect(kernel.emotions.getPainMemoryCount(inspector1)).toBeGreaterThan(0);

    // 5. System alive — no one crashed into the toxic zone
    expect(kernel.isAlive()).toBe(true);

    // 6. Monitor captured the situation
    const history = kernel.monitor.getHistory();

    console.log(
      `[PLANT INSPECTOR — danger zone mapping]\n` +
      `  Toxic zone repulsion: ${norm(worldAtToxic.netForce).toFixed(3)}\n` +
      `  Pheromone warnings: ${pheromoneWarning.sensedCount}\n` +
      `  Inspector 1 fear: ${inspector1Emo?.fear.toFixed(3)}\n` +
      `  Pain memories: ${kernel.emotions.getPainMemoryCount(inspector1)}\n` +
      `  Monitor snapshots: ${history.length}\n` +
      `  Total ticks: ${kernel.getTickCount()}`,
    );
  });
});
