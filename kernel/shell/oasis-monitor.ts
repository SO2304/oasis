/**
 * OASIS Kernel — Monitor (DEP1)
 *
 * READ-ONLY observer that captures kernel state WITHOUT
 * interfering with the deterministic control loop.
 *
 * DESIGN CONSTRAINT: The monitor NEVER:
 * - Allocates memory in the hot path
 * - Holds locks during sampling
 * - Modifies any kernel state
 * - Blocks the RT scheduler
 *
 * It takes SNAPSHOTS — frozen-in-time views of the kernel
 * state that can be consumed by any frontend (web, TUI, VR).
 *
 * INNOVATION: Layered Projection
 * The 128-dim latent space is meaningless to humans.
 * The monitor projects it into interpretable layers:
 * - Topology layer: where are agents relative to each other?
 * - Energy layer: who's moving, who's stuck?
 * - Health layer: entropy, pain, stress indicators
 * - Social layer: swarm membership, pheromone trails
 */

import type { AgentId, TenantId } from '../types.js';
import { monotonicNow, nsToMs } from '../types.js';
import {
  type Vec,
  norm,
  cosineSimilarity,
  distance,
} from '../physics/vector-math.js';
import type { HyperState } from '../physics/hyper-state.js';
import type { Swarm } from '../swarm/swarm-mind.js';

// ─── Snapshot Types ─────────────────────────────────────────────

export interface KernelSnapshot {
  readonly timestamp: bigint;
  readonly tickNumber: number;
  readonly agents: AgentSnapshot[];
  readonly swarms: SwarmSnapshot[];
  readonly fieldMetrics: FieldMetrics;
  readonly systemHealth: SystemHealth;
}

export interface AgentSnapshot {
  readonly id: AgentId;
  /** 2D projection of 128-dim position (for visualization) */
  readonly projectedPosition: { x: number; y: number };
  /** Momentum magnitude — how fast is this agent moving */
  readonly speed: number;
  /** Momentum direction projected to 2D */
  readonly heading: { dx: number; dy: number };
  /** Entropy [0,1] */
  readonly entropy: number;
  /** Collapsed discrete state name */
  readonly state: string;
  /** Pain level [0,1] */
  readonly painLevel: number;
  /** Swarm membership */
  readonly swarmId: string | null;
  /** Is quarantined by immune system? */
  readonly quarantined: boolean;
}

export interface SwarmSnapshot {
  readonly id: string;
  readonly memberCount: number;
  readonly coherence: number;
  readonly hasEmergentGoal: boolean;
  readonly age: number;
  readonly centroid: { x: number; y: number };
}

export interface FieldMetrics {
  readonly activeTensionCount: number;
  readonly totalFieldEnergy: number;
  readonly avgEntropy: number;
  readonly maxEntropy: number;
  readonly pheromoneCount: number;
}

export interface SystemHealth {
  readonly killSwitchEngaged: boolean;
  readonly quarantinedCount: number;
  readonly r14Violations: number;
  readonly r16Violations: number;
  readonly uptimeMs: number;
}

// ─── 2D Projection ─────────────────────────────────────────────

/**
 * Project a 128-dim vector to 2D using the first two principal
 * dimensions that carry the most variance.
 *
 * This is a simplified PCA-like projection: we use the two
 * dimensions with the largest absolute values as X and Y.
 * It's fast (O(d)) and gives meaningful spatial layout.
 */
function projectTo2D(vec: Vec): { x: number; y: number } {
  if (vec.length < 2) return { x: vec[0] ?? 0, y: 0 };

  // Find the two dimensions with largest absolute values
  let maxI = 0, maxVal = 0;
  let secondI = 1, secondVal = 0;

  for (let i = 0; i < vec.length; i++) {
    const abs = Math.abs(vec[i]!);
    if (abs > maxVal) {
      secondI = maxI;
      secondVal = maxVal;
      maxI = i;
      maxVal = abs;
    } else if (abs > secondVal) {
      secondI = i;
      secondVal = abs;
    }
  }

  return { x: vec[maxI]!, y: vec[secondI]! };
}

// ─── Monitor ────────────────────────────────────────────────────

export class OasisMonitor {
  private readonly snapshots: KernelSnapshot[] = [];
  private readonly maxSnapshots: number;
  private readonly startTime: bigint;
  private r14Count = 0;
  private r16Count = 0;

  constructor(maxSnapshots = 1000) {
    this.maxSnapshots = maxSnapshots;
    this.startTime = monotonicNow();
  }

  /**
   * Capture a snapshot of the kernel state.
   *
   * This is the ONLY method that reads kernel state.
   * It must be called from outside the RT loop (e.g., from a
   * monitoring tick that runs at a lower frequency).
   */
  capture(
    tickNumber: number,
    agentStates: Map<AgentId, HyperState>,
    swarms: Map<string, Swarm>,
    tensionCount: number,
    pheromoneCount: number,
    quarantined: Set<AgentId>,
    killSwitchEngaged: boolean,
    painLevels?: Map<AgentId, number>,
  ): KernelSnapshot {
    // Project agents
    const agents: AgentSnapshot[] = [];
    let totalEntropy = 0;
    let maxEntropy = 0;
    let totalEnergy = 0;

    for (const [id, state] of agentStates) {
      const speed = norm(state.momentum);
      const proj = projectTo2D(state.position);
      const headingProj = projectTo2D(state.momentum);

      agents.push({
        id,
        projectedPosition: proj,
        speed,
        heading: { dx: headingProj.x, dy: headingProj.y },
        entropy: state.entropy,
        state: state.collapsed,
        painLevel: painLevels?.get(id) ?? 0,
        swarmId: this.findAgentSwarm(id, swarms),
        quarantined: quarantined.has(id),
      });

      totalEntropy += state.entropy;
      if (state.entropy > maxEntropy) maxEntropy = state.entropy;
      totalEnergy += speed;
    }

    // Project swarms
    const swarmSnapshots: SwarmSnapshot[] = [];
    for (const [id, swarm] of swarms) {
      swarmSnapshots.push({
        id,
        memberCount: swarm.members.size,
        coherence: swarm.coherence,
        hasEmergentGoal: swarm.emergentGoal !== null,
        age: swarm.age,
        centroid: projectTo2D(swarm.collectiveState),
      });
    }

    const snapshot: KernelSnapshot = {
      timestamp: monotonicNow(),
      tickNumber,
      agents,
      swarms: swarmSnapshots,
      fieldMetrics: {
        activeTensionCount: tensionCount,
        totalFieldEnergy: totalEnergy,
        avgEntropy: agents.length > 0 ? totalEntropy / agents.length : 0,
        maxEntropy,
        pheromoneCount,
      },
      systemHealth: {
        killSwitchEngaged,
        quarantinedCount: quarantined.size,
        r14Violations: this.r14Count,
        r16Violations: this.r16Count,
        uptimeMs: nsToMs(monotonicNow() - this.startTime),
      },
    };

    this.snapshots.push(snapshot);
    if (this.snapshots.length > this.maxSnapshots) this.snapshots.shift();

    return snapshot;
  }

  /** Record an R14 violation (for health tracking) */
  recordR14(): void { this.r14Count++; }

  /** Record an R16 violation */
  recordR16(): void { this.r16Count++; }

  /** Get the latest snapshot */
  getLatest(): KernelSnapshot | null {
    return this.snapshots[this.snapshots.length - 1] ?? null;
  }

  /** Get snapshot history */
  getHistory(count?: number): readonly KernelSnapshot[] {
    if (count) return this.snapshots.slice(-count);
    return this.snapshots;
  }

  /** Get a time-series of a specific metric */
  getTimeSeries(
    metric: 'avgEntropy' | 'maxEntropy' | 'tensionCount' | 'quarantinedCount',
  ): number[] {
    return this.snapshots.map(s => {
      switch (metric) {
        case 'avgEntropy': return s.fieldMetrics.avgEntropy;
        case 'maxEntropy': return s.fieldMetrics.maxEntropy;
        case 'tensionCount': return s.fieldMetrics.activeTensionCount;
        case 'quarantinedCount': return s.systemHealth.quarantinedCount;
      }
    });
  }

  private findAgentSwarm(agentId: AgentId, swarms: Map<string, Swarm>): string | null {
    for (const [id, swarm] of swarms) {
      if (swarm.members.has(agentId)) return id;
    }
    return null;
  }
}
