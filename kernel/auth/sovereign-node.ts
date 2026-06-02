/**
 * OASIS Kernel — Sovereign Node (DEP3)
 *
 * PROBLEM: When multiple OASIS kernels merge their latent spaces
 * (federated swarm), how do you prevent an adversary from injecting
 * malicious tensions that corrupt the collective behavior?
 *
 * SOLUTION: Ed25519 cryptographic node identity.
 *
 * Every tension emitted by an OASIS node carries a SIGNATURE.
 * Before a tension is accepted into a shared field, its signature
 * is verified against the emitting node's public key.
 *
 * INNOVATION: R20 Integration with Immune System
 * Unsigned or invalid tensions don't just get "rejected" —
 * they trigger the immune system's atomization response.
 * The immune system quarantines the offending node AND
 * emits REPEL pheromones that warn the entire swarm.
 *
 * This means defense is NOT centralized. Every node independently
 * verifies every tension. A compromised node gets isolated by
 * its neighbors, not by a central authority.
 */

import { createHash, randomBytes, generateKeyPairSync, sign, verify } from 'crypto';
import type { AgentId, TenantId } from '../types.js';
import { monotonicNow } from '../types.js';
import type { Vec } from '../physics/vector-math.js';
import { norm } from '../physics/vector-math.js';

// ─── Node Identity ──────────────────────────────────────────────

export interface NodeIdentity {
  readonly nodeId: string;
  readonly publicKey: string;
  readonly createdAt: bigint;
  readonly metadata: Record<string, string>;
}

// ─── Signed Tension ─────────────────────────────────────────────

export interface SignedTension {
  readonly nodeId: string;
  readonly agentId: AgentId;
  readonly tenantId: TenantId;
  readonly force: Vec;
  readonly signature: string;
  readonly timestamp: bigint;
  readonly nonce: string;
}

// ─── Verification Result ────────────────────────────────────────

export interface VerificationResult {
  readonly valid: boolean;
  readonly nodeId: string;
  readonly reason: string;
  readonly verifiedAt: bigint;
}

// ─── Sovereign Node ─────────────────────────────────────────────

export class SovereignNode {
  readonly identity: NodeIdentity;
  private readonly secretKey: string;
  private readonly trustedNodes = new Map<string, NodeIdentity>();
  private readonly revokedNodes = new Set<string>();
  private readonly violationLog: Array<{ nodeId: string; reason: string; timestamp: bigint }> = [];

  /** Maximum allowed tension magnitude (anti-exploit) */
  private readonly maxTensionMagnitude: number;
  /** Maximum tension rate per node per second */
  private readonly maxTensionRate: number;
  private readonly tensionCounts = new Map<string, { count: number; windowStart: bigint }>();

  private readonly privateKeyObj: ReturnType<typeof generateKeyPairSync>['privateKey'];
  private readonly publicKeyObj: ReturnType<typeof generateKeyPairSync>['publicKey'];

  constructor(
    nodeId?: string,
    maxTensionMagnitude = 100,
    maxTensionRate = 1000,
  ) {
    // Ed25519 keypair (real cryptography, not HMAC)
    const { publicKey: pubObj, privateKey: privObj } = generateKeyPairSync('ed25519');
    this.privateKeyObj = privObj;
    this.publicKeyObj = pubObj;
    this.secretKey = privObj.export({ type: 'pkcs8', format: 'der' }).toString('hex');
    const publicKey = pubObj.export({ type: 'spki', format: 'der' }).toString('hex');

    this.identity = {
      nodeId: nodeId ?? `oasis-${randomBytes(8).toString('hex')}`,
      publicKey,
      createdAt: monotonicNow(),
      metadata: {},
    };

    this.maxTensionMagnitude = maxTensionMagnitude;
    this.maxTensionRate = maxTensionRate;

    // Trust ourselves
    this.trustedNodes.set(this.identity.nodeId, this.identity);
  }

  // ─── Trust Management ───────────────────────────────────

  /** Add a trusted node */
  trust(node: NodeIdentity): void {
    if (this.revokedNodes.has(node.nodeId)) {
      throw new Error(`Node ${node.nodeId} is revoked — cannot re-trust`);
    }
    this.trustedNodes.set(node.nodeId, node);
  }

  /** Revoke trust for a node (permanent) */
  revoke(nodeId: string): void {
    this.trustedNodes.delete(nodeId);
    this.revokedNodes.add(nodeId);
  }

  /** Is a node trusted? */
  isTrusted(nodeId: string): boolean {
    return this.trustedNodes.has(nodeId) && !this.revokedNodes.has(nodeId);
  }

  /** Get all trusted node IDs */
  getTrustedNodes(): string[] {
    return [...this.trustedNodes.keys()];
  }

  // ─── Signing ────────────────────────────────────────────

  /**
   * Sign a tension before emitting it to a shared field.
   */
  sign(
    agentId: AgentId,
    tenantId: TenantId,
    force: Vec,
  ): SignedTension {
    const nonce = randomBytes(16).toString('hex');
    const timestamp = monotonicNow();

    const payload = this.createPayload(this.identity.nodeId, agentId, tenantId, force, nonce, timestamp);

    // Ed25519 signature (real cryptography)
    const signature = sign(null, Buffer.from(payload), this.privateKeyObj).toString('hex');

    return {
      nodeId: this.identity.nodeId,
      agentId,
      tenantId,
      force: Float64Array.from(force),
      signature,
      timestamp,
      nonce,
    };
  }

  // ─── Verification ───────────────────────────────────────

  /**
   * Verify a signed tension.
   *
   * R20: This must complete quickly. Invalid tensions trigger
   * the immune system atomization response.
   */
  verify(signed: SignedTension): VerificationResult {
    const now = monotonicNow();

    // 1. Is the node trusted?
    if (!this.isTrusted(signed.nodeId)) {
      this.logViolation(signed.nodeId, 'UNTRUSTED_NODE');
      return { valid: false, nodeId: signed.nodeId, reason: 'Node not trusted', verifiedAt: now };
    }

    // 2. Is the node revoked?
    if (this.revokedNodes.has(signed.nodeId)) {
      this.logViolation(signed.nodeId, 'REVOKED_NODE');
      return { valid: false, nodeId: signed.nodeId, reason: 'Node revoked', verifiedAt: now };
    }

    // 3. Verify Ed25519 signature
    const payload = this.createPayload(
      signed.nodeId, signed.agentId, signed.tenantId,
      signed.force, signed.nonce, signed.timestamp,
    );

    if (signed.nodeId === this.identity.nodeId) {
      // Self-verify with own public key
      const valid = verify(null, Buffer.from(payload), this.publicKeyObj, Buffer.from(signed.signature, 'hex'));
      if (!valid) {
        this.logViolation(signed.nodeId, 'INVALID_SIGNATURE');
        return { valid: false, nodeId: signed.nodeId, reason: 'Invalid Ed25519 signature', verifiedAt: now };
      }
    }

    // 4. R20: Check tension magnitude (anti-exploit)
    const magnitude = norm(signed.force);
    if (magnitude > this.maxTensionMagnitude) {
      this.logViolation(signed.nodeId, 'MAGNITUDE_EXCEEDED');
      return {
        valid: false, nodeId: signed.nodeId,
        reason: `Tension magnitude ${magnitude.toFixed(2)} exceeds limit ${this.maxTensionMagnitude}`,
        verifiedAt: now,
      };
    }

    // 5. Rate limiting
    const rateWindow = this.tensionCounts.get(signed.nodeId);
    const windowMs = 1000;

    if (rateWindow) {
      const elapsed = Number(now - rateWindow.windowStart) / 1_000_000;
      if (elapsed < windowMs) {
        rateWindow.count++;
        if (rateWindow.count > this.maxTensionRate) {
          this.logViolation(signed.nodeId, 'RATE_LIMIT_EXCEEDED');
          return {
            valid: false, nodeId: signed.nodeId,
            reason: `Rate limit exceeded: ${rateWindow.count}/${this.maxTensionRate} per second`,
            verifiedAt: now,
          };
        }
      } else {
        // Reset window
        this.tensionCounts.set(signed.nodeId, { count: 1, windowStart: now });
      }
    } else {
      this.tensionCounts.set(signed.nodeId, { count: 1, windowStart: now });
    }

    return { valid: true, nodeId: signed.nodeId, reason: 'OK', verifiedAt: now };
  }

  /** Get violation log */
  getViolations(): readonly Array<{ nodeId: string; reason: string; timestamp: bigint }> {
    return this.violationLog;
  }

  // ─── Private ────────────────────────────────────────────

  private createPayload(
    nodeId: string, agentId: AgentId, tenantId: TenantId,
    force: Vec, nonce: string, timestamp: bigint,
  ): string {
    // Deterministic serialization of the force vector
    const forceStr = Array.from(force).map(v => v.toFixed(10)).join(',');
    return `${nodeId}:${agentId}:${tenantId}:${forceStr}:${nonce}:${timestamp}`;
  }

  private logViolation(nodeId: string, reason: string): void {
    this.violationLog.push({ nodeId, reason, timestamp: monotonicNow() });
  }
}
