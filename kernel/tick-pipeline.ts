/** OASIS — 14-phase tick pipeline. Phase order is CRITICAL. */

import type { AgentId, DriverId } from './types.js';
import { monotonicNow, nsToMs } from './types.js';
import type { HyperState } from './physics/hyper-state.js';
import { DeviationSeverity } from './physics/reflection-engine.js';
import { PheromoneType } from './swarm/stigmergy.js';
import { AgentRole } from './morpho/morphogenesis.js';
import type { SwarmEventRecord } from './swarm/swarm-mind.js';
import type { MorphoEvent } from './morpho/morphogenesis.js';
import type { ReflexEvent } from './neuro/reflex.js';
import type { DreamResult } from './neuro/dreams.js';
import type { AttentionWeight } from './neuro/attention.js';
import type { Swarm } from './swarm/swarm-mind.js';
import { norm, normalize, scale, add, distance, zeros } from './physics/vector-math.js';
import type { OasisKernel } from './oasis.js';

// ─── Tick Result ────────────────────────────────────────────────

export interface KernelTickResult {
  readonly tickNumber: number;
  readonly durationMs: number;
  readonly agentsProcessed: number;
  readonly agentsSkipped: number;
  readonly r14Blocked: number;
  readonly swarmEvents: SwarmEventRecord[];
  readonly morphoEvents: MorphoEvent[];
  readonly reflexesFired: ReflexEvent[];
  readonly threatsDetected: number;
  readonly quarantined: number;
  readonly dreamResult: DreamResult | null;
  readonly branchDecisions: number;
  readonly snapshot: ReturnType<OasisKernel['monitor']['capture']> | null;
}

// ─── Pipeline Execution ─────────────────────────────────────────

export function executeTick(k: OasisKernel, tickCount: number): KernelTickResult {
  const start = monotonicNow();
  const budgetNs = BigInt(Math.round(k.config.tickBudgetMs * 1_000_000));

  /** R18: Check if we've consumed > fraction of budget. Non-critical phases skip if over. */
  const overBudget = (fraction: number): boolean =>
    (monotonicNow() - start) > BigInt(Math.round(Number(budgetNs) * fraction));

  let agentsProcessed = 0;
  let agentsSkipped = 0;
  let r14Blocked = 0;
  let swarmEvents: SwarmEventRecord[] = [];
  let morphoEvents: MorphoEvent[] = [];
  let reflexesFired: ReflexEvent[] = [];
  let threatsDetected = 0;
  let dreamResult: DreamResult | null = null;
  let branchDecisions = 0;
  let snapshot: KernelTickResult['snapshot'] = null;

  reflexesFired = phaseReflex(k); // P0: reflex


  // ── 1. PERCEPTION ───────────────────────────────────
  k.bridge.tick();
  k.fusion.tick();
  // ── 2. ATTENTION → SCHEDULING ───────────────────────
  const { allStates, entropies, attentionWeights, toProcess } = phaseAttention(k);
  agentsSkipped = allStates.size - toProcess.length;
  // ── 3. EMOTIONS (gated: only process agents with attention > 0.3) ──
  if (k.config.enableEmotions) {
    phaseEmotions(k, allStates, attentionWeights);
  }
  // ── 4. LATENT ENGINE + forces ───────────────────────
  const engineOut = phaseLatentEngine(k, allStates, toProcess);
  r14Blocked = engineOut.r14Blocked;
  agentsProcessed = toProcess.length;
  // ── 5. TEMPORAL BRANCHING ───────────────────────────
  if (k.config.enableBranching) {
    branchDecisions = phaseBranching(k, allStates, attentionWeights);
  }
  // ── 6. SYNAPTIC UPDATE ──────────────────────────────
  if (tickCount % 3 === 0) {
    k.synapses.update(allStates);
  }
  // ── 6b. MOTOR OUTPUT (translate intentions → HAL commands) ──
  phaseMotorOutput(k, allStates);
  // ── 7. REFLECTION ───────────────────────────────────
  phaseReflection(k, allStates, tickCount);
  // ── 8. MORPHOGENESIS (R18: non-critical, budget-gated) ──
  if (k.config.enableMorpho && (tickCount <= 2 || tickCount % 10 === 0) && (tickCount <= 2 || !overBudget(0.7))) {
    morphoEvents = phaseMorpho(k, allStates);
  }
  // ── 9. SWARM (R18: non-critical, budget-gated) ─────
  if (k.config.enableSwarm && !overBudget(0.75)) {
    swarmEvents = k.swarm.tick(allStates);
  }
  // ── 10. IMMUNE (R18: non-critical, budget-gated) ───
  if (k.config.enableImmune && tickCount % 5 === 0 && !overBudget(0.8)) {
    const result = k.immune.monitor(allStates, k.field, k.sentinelId);
    threatsDetected = result.threats.length;
  }
  // ── 11. STIGMERGY ───────────────────────────────────
  if (!overBudget(0.85)) {
    for (const u of toProcess) {
      const state = allStates.get(u.agentId);
      if (state && norm(state.momentum) > 1e-6) {
        k.stigmergy.deposit(u.agentId, PheromoneType.ATTRACT, state.position, state.momentum, 0.3, 0.06);
      }
    }
    k.stigmergy.tick();
  }
  // ── 12. DREAM GATE ──────────────────────────────────
  if (k.config.enableDreams && tickCount % 200 === 0) {
    dreamResult = phaseDream(k, allStates, entropies);
  }
  // ── 13. MAINTENANCE (always runs — critical for decay) ──
  k.world.tick();
  k.emotions.tick();
  // ── 14. MONITOR (R18: non-critical, budget-gated) ──
  if (k.config.monitorInterval > 0 && tickCount % k.config.monitorInterval === 0) {
    snapshot = k.monitor.capture(
      tickCount, allStates, new Map<string, Swarm>(),
      k.field.getActiveTensionCount(),
      k.stigmergy.getActiveCount(),
      new Set(k.immune.getQuarantined()),
      k.killSwitch.isTriggered(),
    );
  }
  // ── 15. DORMANCY (R18: non-critical, budget-gated) ──
  if (k.dormancy && !overBudget(0.9)) {
    const avgE = allStates.size > 0
      ? [...entropies.values()].reduce((a, b) => a + b, 0) / allStates.size
      : 0;
    const inCrisis = threatsDetected > 0 || r14Blocked > 0;

    if (k.dormancy.shouldSnapshot(tickCount, avgE, inCrisis)) {
      const stateData: Record<string, unknown> = {
        tickCount, agentCount: allStates.size,
        agents: [...allStates.entries()].map(([id, s]) => ({
          id, position: Array.from(s.position),
          momentum: Array.from(s.momentum), entropy: s.entropy,
        })),
        worldZones: k.world.getAllZones().map(z => ({
          id: z.id, type: z.type, intensity: z.intensity, confidence: z.confidence,
        })),
        auditHead: k.auditLog.getHeadHash(),
        synapseCount: k.synapses.getSynapseCount(),
      };
      k.dormancy.snapshot(tickCount, avgE, avgE < 0.2 ? 2 : 1, stateData);
    }
  }

  const totalDurationMs = nsToMs(monotonicNow() - start);

  if (k.budget && totalDurationMs > k.config.tickBudgetMs * 3) {
    k.monitor.recordR14();
  }

  return {
    tickNumber: tickCount,
    durationMs: totalDurationMs,
    agentsProcessed,
    agentsSkipped,
    r14Blocked,
    swarmEvents,
    morphoEvents,
    reflexesFired,
    threatsDetected,
    quarantined: k.immune.getQuarantined().length,
    dreamResult,
    branchDecisions,
    snapshot,
  };
}

// ─── Phase Implementations ──────────────────────────────────────

function phaseReflex(k: OasisKernel): ReflexEvent[] {
  if (k.driverAgentMap.size === 0) return [];

  const telemetryMap = new Map<DriverId, Record<string, number>>();
  for (const [driverId] of k.driverAgentMap) {
    try {
      const t = k.hal.readTelemetry(driverId);
      telemetryMap.set(driverId, t.values);
    } catch { /* stopped */ }
  }

  const fired = k.reflexes.check(telemetryMap);

  for (const event of fired) {
    if (event.action === 'EMERGENCY_STOP' || event.action === 'PANIC') {
      k.hal.emergencyStopAll();
    }
    const ownerAgent = k.driverAgentMap.get(event.driverId);
    if (ownerAgent) {
      const state = k.engine.getState(ownerAgent);
      if (state) k.emotions.recordPain(ownerAgent, state.position, 0.8);
    }
  }

  // Sub-reflex stress detection: register proportional pain from sensor stress
  // even when reflexes don't fire (e.g. moderate vibrations, force spikes)
  if (k.config.enableEmotions) {
    for (const [driverId, values] of telemetryMap) {
      const stress = values['stress'] ?? 0;
      const motionIntensity = values['motionIntensity'] ?? 0;
      const gravityDeviation = values['gravityDeviation'] ?? 0;

      // Proportional pain from physical stress (not binary like reflexes)
      // Any non-zero deviation registers pain — even small vibrations accumulate
      const painLevel = Math.max(stress, motionIntensity, gravityDeviation * 0.5);
      if (painLevel > 0.005) {
        const ownerAgent = k.driverAgentMap.get(driverId);
        if (ownerAgent) {
          const state = k.engine.getState(ownerAgent);
          if (state) k.emotions.recordPain(ownerAgent, state.position, painLevel);
        }
      }
    }
  }

  return fired;
}

interface AttentionResult {
  allStates: Map<AgentId, HyperState>;
  entropies: Map<AgentId, number>;
  attentionWeights: AttentionWeight[];
  toProcess: Array<{ agentId: AgentId }>;
}

function phaseAttention(k: OasisKernel): AttentionResult {
  const allStates = new Map<AgentId, HyperState>();
  const entropies = new Map<AgentId, number>();

  for (const id of k.engine.getAgentIds()) {
    const state = k.engine.getState(id);
    if (state) {
      allStates.set(id, state);
      entropies.set(id, state.entropy);
    }
  }

  const attentionWeights = k.attention.compute(allStates);

  for (const aw of attentionWeights) {
    if (aw.weight > 0.6) k.lazy.markInput(aw.agentId);
  }

  const urgencies = k.lazy.computeUrgencies(entropies);
  const toProcess = urgencies.filter(u => u.shouldRecompute).map(u => ({ agentId: u.agentId }));

  for (const u of toProcess) k.lazy.markUpdated(u.agentId);

  return { allStates, entropies, attentionWeights, toProcess };
}

function phaseEmotions(k: OasisKernel, allStates: Map<AgentId, HyperState>, attentionWeights: AttentionWeight[]): void {
  // OPTIMIZATION: Only update emotions for attended agents.
  // Unattended agents coast on their previous emotional state.
  // This reduces cost from O(all_agents × pain_memories) to
  // O(attended_agents × pain_memories).
  const attended = new Set<AgentId>();
  for (const aw of attentionWeights) {
    if (aw.weight > 0.25) attended.add(aw.agentId);
  }

  for (const [id, state] of allStates) {
    if (!attended.has(id)) continue;

    k.emotions.update(id, state);
    for (const goalPos of k.goalPositions.values()) {
      k.emotions.recordGoalDistance(id, distance(state.position, goalPos));
      break;
    }
  }
}

function phaseLatentEngine(
  k: OasisKernel,
  allStates: Map<AgentId, HyperState>,
  toProcess: Array<{ agentId: AgentId }>,
): { r14Blocked: number } {
  for (const u of toProcess) {
    const state = allStates.get(u.agentId);
    if (!state) continue;

    const worldSample = k.world.sample(state.position);
    let totalForce = worldSample.netForce;

    if (k.config.enableEmotions) {
      const emo = k.emotions.getState(u.agentId);
      if (emo) {
        const gain = k.emotions.computeGain(u.agentId);
        // Apply attractionGain to world forces (goal-seeking amplified by satisfaction)
        if (norm(totalForce) > 1e-6) {
          totalForce = scale(totalForce, gain.attractionGain);
        }
        // Apply repulsionGain: amplify emotional flee force when fear is active
        if (emo.fear > 0.01 || emo.curiosity > 0.01 || emo.frustration > 0.01) {
          const emoForce = k.emotions.computeEmotionalForce(u.agentId, state);
          // repulsionGain amplifies the fear component of emotional force
          const amplifiedEmo = scale(emoForce, emo.fear > 0.01 ? gain.repulsionGain : 1.0);
          totalForce = add(totalForce, amplifiedEmo);
        }
      }
    }

    // Synaptic force — skip scan if no synapses exist
    if (k.synapses.getSynapseCount() > 0) {
      const synapticForce = k.synapses.computeSynapticForce(u.agentId, allStates);
      if (norm(synapticForce) > 1e-6) {
        totalForce = add(totalForce, scale(synapticForce, 0.3));
      }
    }

    if (norm(totalForce) > 1e-6) {
      k.engine.emitTension(u.agentId, totalForce, normalize(totalForce), 0.5, 3, 8);
    }
  }

  const result = k.engine.tick();
  return { r14Blocked: result.r14Blocked.length };
}

function phaseBranching(
  k: OasisKernel,
  allStates: Map<AgentId, HyperState>,
  attentionWeights: AttentionWeight[],
): number {
  if (k.goalPositions.size === 0) return 0;

  const navigators = k.config.enableMorpho
    ? k.morpho.getAgentsByRole(AgentRole.NAVIGATOR)
    : [];

  const goalPos = [...k.goalPositions.values()][0];
  let decisions = 0;

  for (const navId of navigators) {
    const state = allStates.get(navId);
    if (!state) continue;

    const aw = attentionWeights.find(w => w.agentId === navId);
    if (aw && aw.weight < 0.3) continue;

    const result = k.branching.branch(state, k.world, goalPos);
    k.engine.emitTension(navId, result.steeringForce, normalize(result.steeringForce), result.confidence, 3, 10);
    decisions++;
  }

  return decisions;
}

/**
 * MOTOR OUTPUT: Translate agent intentions into HAL commands.
 *
 * For each driver mapped to an agent, decode the agent's
 * latent state into a physical command and send it.
 * This closes the sensorimotor loop INSIDE the kernel —
 * agents don't just think, they ACT.
 */
function phaseMotorOutput(k: OasisKernel, allStates: Map<AgentId, HyperState>): void {
  for (const [driverId, agId] of k.driverAgentMap) {
    const state = allStates.get(agId);
    if (!state) continue;

    // Decode intention to motor command
    const twist = k.codec.decodeTwist(state.position);
    const force = Math.max(-50, Math.min(50, twist.linear.x * 10));

    if (Math.abs(force) < 0.001) continue;

    try {
      k.hal.sendCommand(driverId, 'MOVE', { force });
    } catch {
      // Driver may be stopped, jammed, or killed
    }
  }
}

function phaseReflection(
  k: OasisKernel,
  allStates: Map<AgentId, HyperState>,
  tickCount: number,
): void {
  for (const [driverId, agId] of k.driverAgentMap) {
    try {
      const telemetry = k.hal.readTelemetry(driverId);
      const proprioState = k.proprio.processTelemetry(telemetry);
      const agentState = allStates.get(agId);
      if (!agentState) continue;

      const twist = k.codec.decodeTwist(agentState.position);
      const commandForce = twist.linear.x * 10;

      k.reflection.predict(driverId, proprioState.positionImprint, proprioState.effortImprint, commandForce, 2);
      const dev = k.reflection.reflect(proprioState);

      if (dev) {
        k.reflection.tick([dev]);
        if (k.config.enableEmotions) {
          k.emotions.recordPredictionError(agId, dev.magnitude);
        }
        if (k.config.enableDreams && dev.magnitude > 0.1) {
          const outcome = dev.severity === DeviationSeverity.NOMINAL ? 0.5 : -0.5;
          k.dreams.recordExperience(agId, [agentState.position], [agentState.entropy], outcome, tickCount);
        }
        if (dev.severity === DeviationSeverity.DYSMORPHIA) {
          k.monitor.recordR16();
        }
      }
    } catch { /* driver stopped */ }
  }
}

function phaseMorpho(k: OasisKernel, allStates: Map<AgentId, HyperState>): MorphoEvent[] {
  const agentCount = Math.max(1, allStates.size);
  const threatLevel = k.immune.getQuarantined().length / agentCount;
  // Estimate unknownRatio from average agent entropy — high entropy = more unknown
  let totalEntropy = 0;
  for (const state of allStates.values()) totalEntropy += state.entropy;
  const avgEntropy = totalEntropy / agentCount;
  const unknownRatio = Math.min(1, avgEntropy * 1.5);
  // healingNeeded: count agents with very low momentum (stalled = need help)
  let stalledCount = 0;
  for (const state of allStates.values()) {
    if (norm(state.momentum) < 0.05) stalledCount++;
  }
  const healingNeeded = stalledCount / agentCount;
  return k.morpho.differentiate(allStates, threatLevel, unknownRatio, healingNeeded, k.goalPositions.size > 0);
}

function phaseDream(
  k: OasisKernel,
  allStates: Map<AgentId, HyperState>,
  entropies: Map<AgentId, number>,
): DreamResult | null {
  const avgEntropy = allStates.size > 0
    ? [...entropies.values()].reduce((a, b) => a + b, 0) / allStates.size
    : 0;

  if (k.dreams.shouldDream(avgEntropy, k.goalPositions.size, 0.3)) {
    return k.dreams.dream(k.synapses, allStates);
  }
  return null;
}
