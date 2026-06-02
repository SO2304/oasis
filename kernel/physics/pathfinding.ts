/**
 * OASIS Kernel — Least-Resistance Pathfinding
 *
 * Extracted from WorldModel for R10 compliance.
 * Gradient descent through continuous pressure fields.
 * No A*, no RRT* — pure field physics.
 */

import {
  type Vec,
  zeros,
  add,
  scale,
  norm,
  normalize,
  distance,
} from './vector-math.js';
import type { WorldModel, PressureZone } from './world-model.js';

export interface TrajectoryPoint {
  readonly position: Vec;
  readonly force: Vec;
  readonly entropy: number;
  readonly speed: number;
}

export interface LeastResistancePath {
  readonly points: TrajectoryPoint[];
  readonly totalEntropy: number;
  readonly totalResistance: number;
  readonly feasible: boolean;
  readonly reason: string;
}

/**
 * Compute the path of LEAST RESISTANCE from start to goal zone.
 *
 * The path follows the negative gradient of the combined potential field:
 * - Repulsive zones push the path away from obstacles
 * - Attractive zones pull the path toward the goal
 * - Entropy zones increase uncertainty along the path
 *
 * Speed at each point is inversely proportional to semantic pressure.
 */
export function computeLeastResistancePath(
  world: WorldModel,
  start: Vec,
  goalZoneId: string,
  maxSteps = 200,
  stepSize = 0.05,
  entropyLimit = 0.85,
): LeastResistancePath {
  const goal = world.getZone(goalZoneId);
  if (!goal) {
    return { points: [], totalEntropy: 0, totalResistance: 0, feasible: false, reason: `Goal zone ${goalZoneId} not found` };
  }

  const points: TrajectoryPoint[] = [];
  let position = Float64Array.from(start);
  let totalEntropy = 0;
  let totalResistance = 0;
  const dim = start.length;

  for (let step = 0; step < maxSteps; step++) {
    const worldSample = world.sample(position);

    if (worldSample.localEntropy > entropyLimit) {
      points.push({ position: Float64Array.from(position), force: worldSample.netForce, entropy: worldSample.localEntropy, speed: 0 });
      return { points, totalEntropy, totalResistance, feasible: false, reason: `Entropy limit exceeded at step ${step}: ${worldSample.localEntropy.toFixed(3)} > ${entropyLimit}` };
    }

    const resistance = norm(worldSample.netForce);
    const speed = 1 / (1 + worldSample.semanticPressure);

    points.push({ position: Float64Array.from(position), force: worldSample.netForce, entropy: worldSample.localEntropy, speed });

    totalEntropy += worldSample.localEntropy;
    totalResistance += resistance;

    const distToGoal = distance(position, goal.center);
    if (distToGoal < stepSize * 2) {
      return { points, totalEntropy, totalResistance, feasible: true, reason: `Reached goal in ${step + 1} steps` };
    }

    const goalPull = scale(normalize(Float64Array.from(goal.center).map((v, i) => v - (position[i] ?? 0)) as Vec), goal.intensity * 0.5);
    const combinedForce = add(worldSample.netForce, goalPull);

    if (norm(combinedForce) < 1e-10) {
      const perturbation = zeros(dim);
      perturbation[step % dim] = 0.01;
      position = Float64Array.from(add(position, perturbation));
    } else {
      position = Float64Array.from(add(position, scale(normalize(combinedForce), stepSize * speed)));
    }
  }

  return { points, totalEntropy, totalResistance, feasible: false, reason: `Max steps (${maxSteps}) reached without reaching goal` };
}
