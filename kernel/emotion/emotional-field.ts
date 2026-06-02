/**
 * OASIS Kernel — Emotional Topology
 *
 * NO OTHER SYSTEM HAS THIS.
 *
 * Emotions are not decorative. They are NAVIGATION SIGNALS.
 *
 * In biological organisms, emotions evolved as fast heuristics
 * for survival decisions. Fear makes you avoid. Curiosity makes
 * you explore. Satisfaction makes you repeat. Frustration makes
 * you try something different.
 *
 * In OASIS, emotions are FORCES in the latent space:
 *
 * - CURIOSITY: attraction toward high-entropy zones
 *   (unknown = interesting = explore it)
 *   Biological analogue: dopaminergic exploration drive
 *
 * - FEAR: repulsion from zones where pain was experienced
 *   (learned avoidance, not just obstacle avoidance)
 *   Biological analogue: amygdala fear conditioning
 *
 * - SATISFACTION: reinforcement of trajectories that reduced
 *   the distance to a goal (reward signal)
 *   Biological analogue: endorphin release
 *
 * - FRUSTRATION: accumulated from repeated failed predictions
 *   (efference copy errors). Triggers strategy switching.
 *   Biological analogue: anterior cingulate cortex conflict
 *
 * - URGENCY: time pressure amplifier — all emotions are
 *   stronger when deadlines approach
 *   Biological analogue: cortisol stress response
 *
 * KEY INSIGHT: Emotions modulate the GAIN of the tension field.
 * A curious agent feels attractions more strongly.
 * A fearful agent feels repulsions more strongly.
 * This changes behavior WITHOUT changing the world model.
 */

import type { AgentId } from '../types.js';
import { monotonicNow } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  sub,
  scale,
  norm,
  normalize,
  distance,
  cosineSimilarity,
} from '../physics/vector-math.js';
import type { HyperState } from '../physics/hyper-state.js';

// ─── Emotional State ────────────────────────────────────────────

export interface EmotionalState {
  /** Attraction to unknown regions [0, 1] */
  curiosity: number;
  /** Avoidance of previously painful regions [0, 1] */
  fear: number;
  /** Reward signal from goal progress [0, 1] */
  satisfaction: number;
  /** Accumulated prediction failure [-1, 1] where -1 = deep frustration */
  frustration: number;
  /** Time pressure multiplier [1, 5] */
  urgency: number;
  /** Dominant emotion name */
  dominant: EmotionType;
}

export const EmotionType = {
  CURIOSITY: 'CURIOSITY',
  FEAR: 'FEAR',
  SATISFACTION: 'SATISFACTION',
  FRUSTRATION: 'FRUSTRATION',
  CALM: 'CALM',
} as const;
export type EmotionType = (typeof EmotionType)[keyof typeof EmotionType];

// ─── Pain Memory (for fear conditioning) ────────────────────────

interface PainMemory {
  readonly position: Vec;
  readonly intensity: number;
  readonly timestamp: number;
  /** Decay rate — memories fade over time */
  decayedIntensity: number;
}

// ─── Emotional Gain Modulation ──────────────────────────────────

export interface EmotionalGain {
  /** Multiplier for attractive forces */
  attractionGain: number;
  /** Multiplier for repulsive forces */
  repulsionGain: number;
  /** Multiplier for exploration forces */
  explorationGain: number;
  /** Whether to switch strategy (frustration threshold crossed) */
  switchStrategy: boolean;
}

// ─── Emotional Field ────────────────────────────────────────────

export class EmotionalField {
  private readonly states = new Map<AgentId, EmotionalState>();
  private readonly painMemories = new Map<AgentId, PainMemory[]>();
  private readonly goalHistory = new Map<AgentId, number[]>(); // distance-to-goal history
  private readonly predictionErrors = new Map<AgentId, number[]>();
  private readonly dim: number;
  private tickCount = 0;

  /** Curiosity is triggered when entropy is in this range */
  private readonly curiosityRange: [number, number];
  /** Fear is triggered when approaching a pain memory closer than this */
  private readonly fearRadius: number;
  /** Frustration builds when prediction error exceeds this */
  private readonly frustrationThreshold: number;
  /** Memories decay by this factor per tick */
  private readonly memoryDecay: number;
  /** Strategy switch threshold for frustration */
  private readonly switchThreshold: number;

  constructor(
    dim: number,
    curiosityRange: [number, number] = [0.3, 0.7],
    fearRadius = 2.0,
    frustrationThreshold = 0.3,
    memoryDecay = 0.995,
    switchThreshold = 0.7,
  ) {
    this.dim = dim;
    this.curiosityRange = curiosityRange;
    this.fearRadius = fearRadius;
    this.frustrationThreshold = frustrationThreshold;
    this.memoryDecay = memoryDecay;
    this.switchThreshold = switchThreshold;
  }

  /** Register an agent for emotional processing */
  register(agentId: AgentId): void {
    this.states.set(agentId, {
      curiosity: 0,
      fear: 0,
      satisfaction: 0,
      frustration: 0,
      urgency: 1,
      dominant: EmotionType.CALM,
    });
    this.painMemories.set(agentId, []);
    this.goalHistory.set(agentId, []);
    this.predictionErrors.set(agentId, []);
  }

  /**
   * Record a pain event — creates a fear memory at that position.
   * The agent will feel FEAR when approaching this location again.
   */
  recordPain(agentId: AgentId, position: Vec, intensity: number): void {
    const memories = this.painMemories.get(agentId);
    if (!memories) return;

    memories.push({
      position: Float64Array.from(position),
      intensity,
      timestamp: this.tickCount,
      decayedIntensity: intensity,
    });

    // Cap memory count
    if (memories.length > 50) memories.shift();
  }

  /**
   * Record goal progress — used to compute satisfaction.
   */
  recordGoalDistance(agentId: AgentId, distanceToGoal: number): void {
    const history = this.goalHistory.get(agentId);
    if (!history) return;
    history.push(distanceToGoal);
    if (history.length > 20) history.shift();
  }

  /**
   * Record prediction error — used to compute frustration.
   */
  recordPredictionError(agentId: AgentId, error: number): void {
    const errors = this.predictionErrors.get(agentId);
    if (!errors) return;
    errors.push(error);
    if (errors.length > 20) errors.shift();
  }

  /**
   * SET urgency (time pressure from deadline proximity).
   */
  setUrgency(agentId: AgentId, urgency: number): void {
    const state = this.states.get(agentId);
    if (state) state.urgency = Math.max(1, Math.min(5, urgency));
  }

  /**
   * UPDATE emotional state based on current observations.
   * Call this every tick for each agent.
   */
  update(agentId: AgentId, hyperState: HyperState): EmotionalState {
    const emo = this.states.get(agentId);
    if (!emo) throw new Error(`Agent ${agentId} not registered for emotions`);

    // ── CURIOSITY ─────────────────────────────────────────
    // High when entropy is in the "interesting" range
    // Too low = boring (already known). Too high = dangerous (R14).
    const e = hyperState.entropy;
    if (e >= this.curiosityRange[0] && e <= this.curiosityRange[1]) {
      emo.curiosity = Math.min(1, emo.curiosity + 0.1);
    } else {
      emo.curiosity = Math.max(0, emo.curiosity - 0.05);
    }

    // ── FEAR ──────────────────────────────────────────────
    // High when approaching a position where pain was experienced
    const memories = this.painMemories.get(agentId) ?? [];
    let maxFear = 0;
    for (const mem of memories) {
      mem.decayedIntensity *= this.memoryDecay;
      const dist = distance(hyperState.position, mem.position);
      if (dist < this.fearRadius) {
        const fear = mem.decayedIntensity * (1 - dist / this.fearRadius);
        maxFear = Math.max(maxFear, fear);
      }
    }
    emo.fear = maxFear;

    // ── SATISFACTION ──────────────────────────────────────
    // High when consistently moving toward goal
    const goalHist = this.goalHistory.get(agentId) ?? [];
    if (goalHist.length >= 3) {
      const recent = goalHist.slice(-3);
      const improving = recent[2]! < recent[0]!; // Getting closer
      const delta = recent[0]! - recent[2]!;
      emo.satisfaction = improving ? Math.min(1, delta * 2) : 0;
    }

    // ── FRUSTRATION ──────────────────────────────────────
    // High when prediction errors persist
    const errors = this.predictionErrors.get(agentId) ?? [];
    if (errors.length >= 5) {
      const recent = errors.slice(-5);
      const avgError = recent.reduce((a, b) => a + b, 0) / recent.length;
      if (avgError > this.frustrationThreshold) {
        emo.frustration = Math.min(1, emo.frustration + 0.05);
      } else {
        emo.frustration = Math.max(0, emo.frustration - 0.02);
      }
    }

    // ── CROSS-MODULATION (Claim 5 — Patent-optimized) ──────
    // Emotions are NOT independent. They inhibit/excite each other
    // like neurotransmitter systems in the biological brain:
    //
    // Fear SUPPRESSES curiosity (danger = don't explore)
    // Satisfaction SUPPRESSES frustration (winning = not stuck)
    // Frustration AMPLIFIES curiosity (stuck = try something new)
    // Fear AMPLIFIES urgency (danger = act faster)
    //
    // This cross-modulation creates emergent emotional DYNAMICS —
    // the same external stimulus produces different behavior depending
    // on the agent's current emotional state. This is what makes
    // the patent claim non-obvious: the behavior is not from the
    // world model, but from the INTERACTION between emotions.

    emo.curiosity = Math.max(0, emo.curiosity * (1 - emo.fear * 0.7));      // Fear suppresses curiosity
    emo.frustration = Math.max(0, emo.frustration * (1 - emo.satisfaction * 0.8)); // Satisfaction calms frustration
    emo.curiosity = Math.min(1, emo.curiosity + emo.frustration * 0.3);     // Frustration breeds curiosity
    if (emo.fear > 0.5) emo.urgency = Math.min(5, emo.urgency * (1 + emo.fear)); // Fear amplifies urgency

    // ── DOMINANT EMOTION ─────────────────────────────────
    const intensities = [
      { type: EmotionType.CURIOSITY as EmotionType, val: emo.curiosity },
      { type: EmotionType.FEAR as EmotionType, val: emo.fear },
      { type: EmotionType.SATISFACTION as EmotionType, val: emo.satisfaction },
      { type: EmotionType.FRUSTRATION as EmotionType, val: emo.frustration },
    ];
    const strongest = intensities.reduce((a, b) => b.val > a.val ? b : a);
    emo.dominant = strongest.val > 0.2 ? strongest.type : EmotionType.CALM;

    return emo;
  }

  /**
   * Compute GAIN MODULATION — how emotions change the agent's
   * sensitivity to different types of forces.
   *
   * This is the output that feeds back into the tension field.
   * A curious agent amplifies exploration. A fearful agent
   * amplifies avoidance. A frustrated agent switches strategy.
   */
  computeGain(agentId: AgentId): EmotionalGain {
    const emo = this.states.get(agentId);
    if (!emo) return { attractionGain: 1, repulsionGain: 1, explorationGain: 1, switchStrategy: false };

    const u = emo.urgency;

    return {
      // Satisfaction increases goal-seeking
      attractionGain: (1 + emo.satisfaction * 0.5) * u,
      // Fear increases avoidance (dramatically)
      repulsionGain: (1 + emo.fear * 3.0) * u,
      // Curiosity increases exploration
      explorationGain: (1 + emo.curiosity * 2.0) * (1 - emo.fear * 0.5),
      // Frustration triggers strategy switch
      switchStrategy: emo.frustration > this.switchThreshold,
    };
  }

  /**
   * Generate an EMOTIONAL FORCE — a direct force in latent space
   * based on the current emotional state.
   *
   * This force is ADDED to the tension field forces.
   * It's how emotions influence behavior at the physics level.
   */
  computeEmotionalForce(agentId: AgentId, hyperState: HyperState, goalPosition?: Vec): Vec {
    const emo = this.states.get(agentId);
    if (!emo) return zeros(this.dim);

    let force = zeros(this.dim);

    // CURIOSITY → drift toward higher entropy regions
    if (emo.curiosity > 0.3) {
      // Random exploration direction, scaled by curiosity
      const explorationDir = zeros(this.dim);
      // Use tick count for deterministic pseudo-random exploration
      const idx = (this.tickCount * 7 + agentId.charCodeAt(0)) % this.dim;
      explorationDir[idx] = emo.curiosity * 0.3;
      force = add(force, explorationDir);
    }

    // FEAR → flee from pain memories
    const memories = this.painMemories.get(agentId) ?? [];
    for (const mem of memories) {
      const dist = distance(hyperState.position, mem.position);
      if (dist < this.fearRadius && dist > 0.01) {
        const away = normalize(sub(hyperState.position, mem.position));
        const fleeForce = scale(away, emo.fear * mem.decayedIntensity / (dist + 0.1));
        force = add(force, fleeForce);
      }
    }

    // SATISFACTION → amplify current direction (keep going!)
    if (emo.satisfaction > 0.3 && norm(hyperState.momentum) > 1e-6) {
      const boost = scale(normalize(hyperState.momentum), emo.satisfaction * 0.2);
      force = add(force, boost);
    }

    // FRUSTRATION → perpendicular to current direction (try something new)
    if (emo.frustration > this.switchThreshold && norm(hyperState.momentum) > 1e-6) {
      const current = normalize(hyperState.momentum);
      // Rotate 90 degrees in the first two significant dimensions
      const perpendicular = Float64Array.from(current);
      if (perpendicular.length >= 2) {
        const tmp = perpendicular[0]!;
        perpendicular[0] = -perpendicular[1]!;
        perpendicular[1] = tmp;
      }
      const switchForce = scale(perpendicular, emo.frustration * 0.5);
      force = add(force, switchForce);
    }

    return force;
  }

  /** Advance tick — decay memories */
  tick(): void {
    this.tickCount++;
  }

  /** Get emotional state */
  getState(agentId: AgentId): EmotionalState | undefined {
    return this.states.get(agentId);
  }

  /** Get pain memory count for an agent */
  getPainMemoryCount(agentId: AgentId): number {
    return this.painMemories.get(agentId)?.length ?? 0;
  }
}
