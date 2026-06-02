/**
 * OASIS Kernel — Flocking Algorithm
 *
 * Extracted from SwarmMind for R10 compliance.
 * Three boid rules adapted for N-dimensional latent space.
 */

import type { AgentId } from '../types.js';
import {
  type Vec,
  zeros,
  add,
  sub,
  scale,
  norm,
  normalize,
  distance,
  weightedCentroid,
} from '../physics/vector-math.js';
import type { HyperState } from '../physics/hyper-state.js';
import type { Swarm } from './swarm-mind.js';

export interface FlockingForces {
  readonly alignment: Vec;
  readonly cohesion: Vec;
  readonly separation: Vec;
  readonly combined: Vec;
}

export function computeFlockingForces(
  agentId: AgentId,
  agentState: HyperState,
  allStates: Map<AgentId, HyperState>,
  swarm: Swarm,
  dim: number,
  neighborRadius: number,
  separationDistance: number,
): FlockingForces {
  const zeroForce = zeros(dim);
  const neighbors: Array<{ state: HyperState; dist: number }> = [];

  for (const memberId of swarm.members) {
    if (memberId === agentId) continue;
    const memberState = allStates.get(memberId);
    if (!memberState) continue;

    const dist = distance(agentState.position, memberState.position);
    if (dist < neighborRadius) {
      neighbors.push({ state: memberState, dist });
    }
  }

  if (neighbors.length === 0) {
    return { alignment: zeroForce, cohesion: zeroForce, separation: zeroForce, combined: zeroForce };
  }

  // 1. ALIGNMENT: average momentum of neighbors
  let alignSum = zeros(dim);
  for (const n of neighbors) alignSum = add(alignSum, n.state.momentum);
  const alignment = norm(alignSum) > 0 ? scale(normalize(alignSum), 0.3) : zeroForce;

  // 2. COHESION: steer toward center of mass
  const center = weightedCentroid(neighbors.map(n => n.state.position), neighbors.map(() => 1));
  const toCenter = sub(center, agentState.position);
  const cohesion = norm(toCenter) > 0 ? scale(normalize(toCenter), 0.2) : zeroForce;

  // 3. SEPARATION: repel from too-close neighbors
  let sepSum = zeros(dim);
  let sepCount = 0;
  for (const n of neighbors) {
    if (n.dist < separationDistance && n.dist > 0.001) {
      sepSum = add(sepSum, scale(normalize(sub(agentState.position, n.state.position)), 1 / n.dist));
      sepCount++;
    }
  }
  const separation = sepCount > 0 ? scale(normalize(sepSum), 0.4) : zeroForce;

  return { alignment, cohesion, separation, combined: add(add(alignment, cohesion), separation) };
}
