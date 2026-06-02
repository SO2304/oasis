/**
 * OASIS Kernel — Correlation Discovery
 *
 * Extracted from LatentEngine for R10 compliance.
 * Cross-correlation + mutual information for trajectory analysis.
 */

import type { AgentId } from '../types.js';
import { type Vec, sub, norm, normalize, cosineSimilarity } from './vector-math.js';
import { estimateMutualInformation } from './mutual-info.js';
import type { TensionField } from './tension-field.js';

export interface LatentCorrelation {
  readonly agentA: AgentId;
  readonly agentB: AgentId;
  readonly strength: number;
  readonly lag: number;
  readonly correlationAxis: Vec;
  readonly isEmergent: boolean;
}

interface TrajectoryPoint {
  readonly position: Vec;
  readonly tick: number;
}

function computeDeltas(traj: TrajectoryPoint[]): Vec[] {
  const deltas: Vec[] = [];
  for (let i = 1; i < traj.length; i++) {
    deltas.push(sub(traj[i]!.position, traj[i - 1]!.position));
  }
  return deltas;
}

function crossCorrelate(a: Vec[], b: Vec[]): { strength: number; lag: number } {
  const maxLag = Math.min(5, Math.floor(Math.min(a.length, b.length) / 2));
  let bestStrength = 0;
  let bestLag = 0;

  for (let lag = -maxLag; lag <= maxLag; lag++) {
    let sumSim = 0;
    let count = 0;

    for (let i = 0; i < a.length && i + lag >= 0 && i + lag < b.length; i++) {
      const idxB = i + lag;
      if (idxB < 0 || idxB >= b.length) continue;
      sumSim += cosineSimilarity(a[i]!, b[idxB]!);
      count++;
    }

    if (count > 0) {
      const avgSim = sumSim / count;
      if (Math.abs(avgSim) > Math.abs(bestStrength)) {
        bestStrength = avgSim;
        bestLag = lag;
      }
    }
  }

  return { strength: bestStrength, lag: bestLag };
}

/**
 * Discover correlations between agent trajectories using
 * both linear cross-correlation AND mutual information.
 */
export function discoverCorrelations(
  trajectories: Map<AgentId, TrajectoryPoint[]>,
  correlations: LatentCorrelation[],
  field: TensionField,
  correlationThreshold: number,
): LatentCorrelation[] {
  const newCorrelations: LatentCorrelation[] = [];
  const agentList = [...trajectories.entries()];
  const minWindow = 10;

  for (let i = 0; i < agentList.length; i++) {
    for (let j = i + 1; j < agentList.length; j++) {
      const [idA, trajA] = agentList[i]!;
      const [idB, trajB] = agentList[j]!;

      if (trajA.length < minWindow || trajB.length < minWindow) continue;

      const deltasA = computeDeltas(trajA);
      const deltasB = computeDeltas(trajB);

      const bestCorr = crossCorrelate(deltasA, deltasB);

      const positionsA = trajA.map(t => t.position);
      const positionsB = trajB.map(t => t.position);
      const miResult = estimateMutualInformation(positionsA, positionsB, 8);

      const effectiveStrength = Math.max(
        Math.abs(bestCorr.strength),
        miResult.normalizedMI,
      );

      if (effectiveStrength >= correlationThreshold) {
        const lastA = trajA[trajA.length - 1]!.position;
        const lastB = trajB[trajB.length - 1]!.position;
        const axis = normalize(sub(lastA, lastB));

        const directTensionExists = field.getActiveTensionCount() > 0;
        const isEmergent = !directTensionExists || Math.abs(bestCorr.lag) > 2;

        const correlation: LatentCorrelation = {
          agentA: idA, agentB: idB,
          strength: bestCorr.strength,
          lag: bestCorr.lag,
          correlationAxis: axis,
          isEmergent,
        };

        newCorrelations.push(correlation);

        const existing = correlations.findIndex(
          c => (c.agentA === idA && c.agentB === idB) || (c.agentA === idB && c.agentB === idA),
        );
        if (existing >= 0) {
          correlations[existing] = correlation;
        } else {
          correlations.push(correlation);
        }
      }
    }
  }

  return newCorrelations;
}
