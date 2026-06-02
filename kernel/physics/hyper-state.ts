/**
 * OASIS Kernel — HyperState
 *
 * REPLACES discrete states (IDLE, RUNNING, etc.) with a continuous
 * N-dimensional state vector + uncertainty quantification.
 *
 * CORE INSIGHT: An agent's "state" is not a label — it's a POSITION
 * in a high-dimensional space. The discrete labels are just projections
 * (like shadows on a wall). The real state lives in the latent manifold.
 *
 * Key concepts:
 * - State vector: where the agent IS in latent space
 * - Momentum vector: where the agent is GOING
 * - Entropy: how UNCERTAIN we are about both
 * - Collapse: projecting the continuous state to a discrete label
 *   (like quantum measurement — we lose information but gain actionability)
 */

import type { AgentId } from '../types.js';
import { AgentState } from '../types.js';
import {
  type Vec,
  zeros,
  vec,
  add,
  sub,
  scale,
  norm,
  normalize,
  dot,
  cosineSimilarity,
  lerp,
  distance,
} from './vector-math.js';

/** Default latent space dimensionality */
export const DEFAULT_DIM = 128;

/** Entropy threshold for R14 (quantum safety) */
export const R14_ENTROPY_CRITICAL = 0.85;

// ─── HyperState ─────────────────────────────────────────────────

export interface HyperState {
  /** Agent this state belongs to */
  readonly agentId: AgentId;
  /** Position in latent space (where the agent IS) */
  readonly position: Vec;
  /** Momentum in latent space (where the agent is GOING) */
  readonly momentum: Vec;
  /** Shannon entropy: measure of uncertainty [0, 1] */
  readonly entropy: number;
  /** Monotonic timestamp of last state update */
  readonly updatedAt: bigint;
  /** Discrete projection (the "shadow" — computed, not primary) */
  readonly collapsed: AgentState;
}

/**
 * Anchors in latent space for each discrete state.
 * These are fixed reference points — the continuous state
 * is "nearest" to one of these, determining its collapsed label.
 *
 * INNOVATION: The anchors are NOT orthogonal. RUNNING is "close to"
 * READY (smooth transition), but "far from" FAILED (catastrophic jump).
 * This encodes the topology of state transitions directly in geometry.
 */
const STATE_ANCHORS: Record<AgentState, number[]> = {
  CREATED:      [1, 0, 0, 0, 0, 0, 0, 0, 0],
  INITIALIZING: [0.7, 0.7, 0, 0, 0, 0, 0, 0, 0],
  READY:        [0, 1, 0, 0, 0, 0, 0, 0, 0],
  RUNNING:      [0, 0.3, 1, 0, 0, 0, 0, 0, 0],
  PAUSED:       [0, 0.5, 0.3, 0, 0, 0, 0, 1, 0],
  STOPPING:     [0, 0, 0.3, 1, 0, 0, 0, 0, 0],
  STOPPED:      [0, 0, 0, 0.3, 1, 0, 0, 0, 0],
  FAILED:       [0, 0, 0, 0, 0, 1, 0, 0, 0],
  KILLED:       [0, 0, 0, 0, 0, 0, 1, 0, 0],
};

/**
 * Create the anchor vector for a discrete state, padded to N dimensions.
 * The first 9 dimensions encode state identity, remaining dims are free
 * for the agent to use as its own semantic representation.
 */
function anchorVec(state: AgentState, dim: number): Vec {
  const anchor = STATE_ANCHORS[state];
  const v = zeros(dim);
  for (let i = 0; i < anchor.length && i < dim; i++) {
    v[i] = anchor[i]!;
  }
  return v;
}

// ─── HyperState Operations ─────────────────────────────────────

/**
 * Create a new HyperState at a discrete anchor position.
 * Entropy starts at 0 (fully certain).
 */
export function createHyperState(
  agentId: AgentId,
  initialState: AgentState,
  dim: number = DEFAULT_DIM,
): HyperState {
  return {
    agentId,
    position: anchorVec(initialState, dim),
    momentum: zeros(dim),
    entropy: 0,
    updatedAt: process.hrtime.bigint(),
    collapsed: initialState,
  };
}

/**
 * Evolve a HyperState by applying a force vector.
 *
 * Physics metaphor:
 * - Force pushes the state through latent space
 * - Momentum accumulates (with damping)
 * - Entropy increases with movement (uncertainty grows when state changes)
 * - Entropy decreases near anchor points (being near a known state = certainty)
 *
 * @param dt Time step (0-1, fraction of a "tick")
 * @param damping Momentum damping factor (0 = no damping, 1 = full stop)
 */
export function evolveState(
  state: HyperState,
  force: Vec,
  dt: number = 0.1,
  damping: number = 0.05,
): HyperState {
  const dim = state.position.length;
  const newMomentum = new Float64Array(dim);
  const newPosition = new Float64Array(dim);
  const dampFactor = 1 - damping;

  // Single pass: momentum + position update (2 arrays instead of 5)
  for (let i = 0; i < dim; i++) {
    newMomentum[i] = state.momentum[i]! * dampFactor + force[i]! * dt;
    newPosition[i] = state.position[i]! + newMomentum[i]! * dt;
  }

  // Calculate entropy based on distance to nearest anchor
  const newEntropy = calculateEntropy(newPosition);

  // Collapse to discrete state (nearest anchor)
  const newCollapsed = collapseState(newPosition);

  return {
    agentId: state.agentId,
    position: newPosition,
    momentum: newMomentum,
    entropy: newEntropy,
    updatedAt: process.hrtime.bigint(),
    collapsed: newCollapsed,
  };
}

/**
 * Calculate Shannon entropy based on proximity to anchor states.
 *
 * INSIGHT: When a state is close to one anchor, entropy is LOW (certain).
 * When it's equidistant from multiple anchors, entropy is HIGH (uncertain).
 *
 * We use Euclidean distance in the ANCHOR SUBSPACE (first 9 dimensions)
 * to avoid dilution in high-dimensional spaces. The remaining dimensions
 * are the agent's "semantic freedom" and don't affect state certainty.
 *
 * Returns [0, 1] where 0 = fully certain, 1 = maximum uncertainty.
 */
export function calculateEntropy(position: Vec): number {
  const states = Object.keys(STATE_ANCHORS) as AgentState[];

  // Compute distances in anchor subspace (first ANCHOR_DIM dimensions)
  const distances: number[] = [];
  let minDist = Infinity;

  for (const state of states) {
    const anchor = STATE_ANCHORS[state];
    let distSq = 0;
    for (let i = 0; i < anchor.length && i < position.length; i++) {
      const d = (position[i] ?? 0) - anchor[i]!;
      distSq += d * d;
    }
    const dist = Math.sqrt(distSq);
    distances.push(dist);
    if (dist < minDist) minDist = dist;
  }

  // Convert distances to probabilities using softmin (inverse softmax)
  // Closer = higher probability
  const temperature = 0.3;
  let sumExp = 0;
  const probs: number[] = [];
  for (const dist of distances) {
    const e = Math.exp(-(dist - minDist) / temperature);
    probs.push(e);
    sumExp += e;
  }

  // Shannon entropy: H = -Σ p(i) * log(p(i))
  let entropy = 0;
  const maxEntropy = Math.log(states.length);

  for (let i = 0; i < probs.length; i++) {
    const p = probs[i]! / sumExp;
    if (p > 1e-10) {
      entropy -= p * Math.log(p);
    }
  }

  // Normalize to [0, 1]
  return Math.min(1, entropy / maxEntropy);
}

/**
 * Collapse the continuous HyperState to a discrete AgentState.
 * Finds the anchor with smallest Euclidean distance in anchor subspace.
 *
 * Like quantum measurement: information is lost, but we get
 * a concrete, actionable label.
 */
export function collapseState(position: Vec): AgentState {
  const states = Object.keys(STATE_ANCHORS) as AgentState[];

  let bestState: AgentState = AgentState.CREATED;
  let bestDist = Infinity;

  for (const state of states) {
    const anchor = STATE_ANCHORS[state];
    let distSq = 0;
    for (let i = 0; i < anchor.length && i < position.length; i++) {
      const d = (position[i] ?? 0) - anchor[i]!;
      distSq += d * d;
    }
    if (distSq < bestDist) {
      bestDist = distSq;
      bestState = state;
    }
  }

  return bestState;
}

/**
 * Compute the "transition probability" from current state to target.
 *
 * Returns [0, 1]:
 * - 1.0: the momentum is pointing directly at the target
 * - 0.5: orthogonal (no tendency either way)
 * - 0.0: moving directly away from target
 */
export function transitionProbability(
  state: HyperState,
  target: AgentState,
): number {
  const dim = state.position.length;
  const targetAnchor = anchorVec(target, dim);
  const directionToTarget = normalize(sub(targetAnchor, state.position));

  if (norm(state.momentum) < 1e-10) {
    // No momentum — probability based purely on proximity
    return Math.max(0, cosineSimilarity(state.position, targetAnchor));
  }

  const momentumDir = normalize(state.momentum);
  const alignment = cosineSimilarity(momentumDir, directionToTarget);

  // Map [-1, 1] to [0, 1]
  return (alignment + 1) / 2;
}

/**
 * R14 CHECK: Is the entropy below the critical threshold?
 * If not, physical actions MUST be blocked.
 */
export function isActionSafe(state: HyperState, threshold = R14_ENTROPY_CRITICAL): boolean {
  return state.entropy < threshold;
}

/**
 * Create a "steering force" that pulls the state toward a target anchor.
 * Used by the scheduler to drive state transitions.
 *
 * @param strength How hard to pull (0-1)
 */
export function steerToward(
  state: HyperState,
  target: AgentState,
  strength: number = 0.5,
): Vec {
  const dim = state.position.length;
  const targetAnchor = anchorVec(target, dim);
  const direction = sub(targetAnchor, state.position);
  return scale(normalize(direction), strength);
}
