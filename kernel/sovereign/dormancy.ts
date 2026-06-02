/**
 * OASIS Sovereign — Dormancy (Phase 15)
 *
 * NOT a cron job. NOT an interval. A GATED snapshot:
 *
 * Gate 1: ticksSinceSnapshot > minInterval (cooldown)
 * Gate 2: avgEntropy < threshold (system is stable)
 * Gate 3: no active emergency (don't snapshot chaos)
 *
 * When all gates pass: serialize → hash → write to disk.
 * Also triggers on SIGTERM (graceful shutdown).
 *
 * WHY ENTROPY-GATED:
 * Snapshotting a system in crisis is HARMFUL. If you restore
 * from a snapshot where all robots are panicking, the restored
 * system immediately re-panics. Better to snapshot when calm —
 * the restored system starts from a known-good state and
 * re-discovers the current reality via its sensors.
 *
 * WHAT TO SAVE (tiered):
 * - TIER 1 (always): agents, world zones, audit head
 * - TIER 2 (if calm): synapses, emotions, pheromones
 * - TIER 3 (on SIGTERM only): full state dump
 */

import { createHash } from 'crypto';
import { writeFileSync, existsSync, mkdirSync, readFileSync } from 'fs';
import { join } from 'path';
import { monotonicNow, nsToMs } from '../types.js';

export interface DormancyConfig {
  /** Minimum ticks between snapshots */
  minInterval: number;
  /** Maximum entropy to allow snapshot */
  maxEntropy: number;
  /** Directory to write snapshots */
  snapshotDir: string;
  /** Include tier 2 data (synapses, emotions)? */
  includeTier2: boolean;
}

export const DEFAULT_DORMANCY: DormancyConfig = {
  minInterval: 500,
  maxEntropy: 0.3,
  snapshotDir: '.oasis/snapshots',
  includeTier2: true,
};

export interface SnapshotMetadata {
  readonly tickCount: number;
  readonly timestamp: string;
  readonly avgEntropy: number;
  readonly agentCount: number;
  readonly checksum: string;
  readonly tier: 1 | 2 | 3;
  readonly sizeBytes: number;
  readonly serializeMs: number;
}

export class DormancyManager {
  private readonly config: DormancyConfig;
  private lastSnapshotTick = 0;
  private snapshotCount = 0;
  private totalSerializeMs = 0;
  private lastMetadata: SnapshotMetadata | null = null;

  constructor(config: DormancyConfig = DEFAULT_DORMANCY) {
    this.config = config;
  }

  /**
   * CHECK: Should we snapshot now?
   * Multi-gate — cheapest checks first (like DreamTask pattern).
   */
  shouldSnapshot(tickCount: number, avgEntropy: number, inCrisis: boolean): boolean {
    // Gate 1 (free): cooldown
    if (tickCount - this.lastSnapshotTick < this.config.minInterval) return false;

    // Gate 2 (free): no crisis
    if (inCrisis) return false;

    // Gate 3 (free): entropy low enough
    if (avgEntropy > this.config.maxEntropy) return false;

    return true;
  }

  /**
   * SNAPSHOT: Serialize and persist the kernel state.
   * Returns metadata about the snapshot.
   */
  snapshot(
    tickCount: number,
    avgEntropy: number,
    tier: 1 | 2 | 3,
    stateData: Record<string, unknown>,
  ): SnapshotMetadata {
    const start = monotonicNow();

    // Serialize
    const json = JSON.stringify(stateData);
    const checksum = createHash('sha256').update(json).digest('hex');

    // Write to disk
    try {
      if (!existsSync(this.config.snapshotDir)) {
        mkdirSync(this.config.snapshotDir, { recursive: true });
      }

      const filename = `snapshot-${tickCount}-t${tier}.json`;
      const filepath = join(this.config.snapshotDir, filename);
      writeFileSync(filepath, json);

      // Also write a "latest" symlink-like file
      writeFileSync(
        join(this.config.snapshotDir, 'latest.json'),
        JSON.stringify({ file: filename, checksum, tickCount, tier }),
      );
    } catch {
      // Disk write failure is NOT fatal — the kernel continues
      // The snapshot is lost but the system is still alive
    }

    const serializeMs = nsToMs(monotonicNow() - start);

    this.lastSnapshotTick = tickCount;
    this.snapshotCount++;
    this.totalSerializeMs += serializeMs;

    const metadata: SnapshotMetadata = {
      tickCount,
      timestamp: new Date().toISOString(),
      avgEntropy,
      agentCount: (stateData['agentCount'] as number) ?? 0,
      checksum,
      tier,
      sizeBytes: json.length,
      serializeMs,
    };

    this.lastMetadata = metadata;
    return metadata;
  }

  /**
   * RESTORE: Load the latest snapshot from disk.
   * Returns null if no snapshot exists or if corrupted.
   */
  restore(): { data: Record<string, unknown>; metadata: SnapshotMetadata } | null {
    try {
      const latestPath = join(this.config.snapshotDir, 'latest.json');
      if (!existsSync(latestPath)) return null;

      const latestInfo = JSON.parse(readFileSync(latestPath, 'utf-8'));
      const snapshotPath = join(this.config.snapshotDir, latestInfo.file);

      if (!existsSync(snapshotPath)) return null;

      const json = readFileSync(snapshotPath, 'utf-8');

      // Verify checksum
      const checksum = createHash('sha256').update(json).digest('hex');
      if (checksum !== latestInfo.checksum) {
        return null; // Corrupted
      }

      const data = JSON.parse(json);

      return {
        data,
        metadata: {
          tickCount: latestInfo.tickCount,
          timestamp: new Date().toISOString(),
          avgEntropy: 0,
          agentCount: data.agentCount ?? 0,
          checksum,
          tier: latestInfo.tier,
          sizeBytes: json.length,
          serializeMs: 0,
        },
      };
    } catch {
      return null; // Any error = no restore
    }
  }

  /** Get stats */
  getStats(): { count: number; avgSerializeMs: number; lastMetadata: SnapshotMetadata | null } {
    return {
      count: this.snapshotCount,
      avgSerializeMs: this.snapshotCount > 0 ? this.totalSerializeMs / this.snapshotCount : 0,
      lastMetadata: this.lastMetadata,
    };
  }

  getLastSnapshotTick(): number { return this.lastSnapshotTick; }
}
