/**
 * OASIS Sovereign — Immutable Audit Log
 *
 * Every action in the kernel is logged with:
 * - WHO (agent ID + node signature)
 * - WHAT (action type + parameters hash)
 * - WHEN (monotonic timestamp)
 * - HASH (SHA-256 chain — each entry includes previous hash)
 *
 * The chain is tamper-evident: modifying ANY entry breaks
 * the hash chain of ALL subsequent entries.
 *
 * This is NOT a blockchain (no consensus, no mining).
 * It's a hash chain — the simplest tamper-evident structure.
 * O(1) append, O(1) verify last entry, O(n) full audit.
 */

import { createHash } from 'crypto';
import { monotonicNow } from '../types.js';

export interface AuditEntry {
  readonly sequence: number;
  readonly timestamp: bigint;
  readonly agentId: string;
  readonly action: string;
  readonly paramsHash: string;
  readonly prevHash: string;
  readonly hash: string;
}

export class AuditLog {
  private readonly entries: AuditEntry[] = [];
  private lastHash = '0'.repeat(64); // Genesis hash
  private sequence = 0;

  /** Append an entry to the log. Returns the entry hash. */
  append(agentId: string, action: string, params: Record<string, unknown>): string {
    const timestamp = monotonicNow();
    const paramsHash = createHash('sha256')
      .update(JSON.stringify(params))
      .digest('hex')
      .slice(0, 16); // Truncated for efficiency

    const payload = `${this.sequence}:${timestamp}:${agentId}:${action}:${paramsHash}:${this.lastHash}`;
    const hash = createHash('sha256').update(payload).digest('hex');

    const entry: AuditEntry = {
      sequence: this.sequence++,
      timestamp,
      agentId,
      action,
      paramsHash,
      prevHash: this.lastHash,
      hash,
    };

    this.entries.push(entry);
    this.lastHash = hash;

    return hash;
  }

  /** Verify the entire chain is intact. Returns first broken entry or null. */
  verifyChain(): { valid: boolean; brokenAt: number | null } {
    let prevHash = '0'.repeat(64);

    for (let i = 0; i < this.entries.length; i++) {
      const entry = this.entries[i]!;

      if (entry.prevHash !== prevHash) {
        return { valid: false, brokenAt: i };
      }

      const payload = `${entry.sequence}:${entry.timestamp}:${entry.agentId}:${entry.action}:${entry.paramsHash}:${entry.prevHash}`;
      const expectedHash = createHash('sha256').update(payload).digest('hex');

      if (entry.hash !== expectedHash) {
        return { valid: false, brokenAt: i };
      }

      prevHash = entry.hash;
    }

    return { valid: true, brokenAt: null };
  }

  /** Get the last N entries */
  getRecent(n: number): readonly AuditEntry[] {
    return this.entries.slice(-n);
  }

  /** Get entries by agent */
  getByAgent(agentId: string): readonly AuditEntry[] {
    return this.entries.filter(e => e.agentId === agentId);
  }

  /** Total entries */
  getCount(): number {
    return this.entries.length;
  }

  /** Get last hash (for external verification) */
  getHeadHash(): string {
    return this.lastHash;
  }
}
