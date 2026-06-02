/**
 * OASIS Bridge — Inter-Kernel Communication (sovereign, signed, rate-limited).
 * Each kernel owns an Ed25519 keypair; only PUBLIC keys live in the bridge.
 * Signals are DECLARATIONS signed over canonical JSON; receivers decide action.
 * Ring-buffered active set, type-indexed O(1) receive, token-bucket DoS guard.
 */

import { createPublicKey, generateKeyPairSync, sign as edSign, verify as edVerify, KeyObject } from 'crypto';
import { monotonicNow, nsToMs } from '../types.js';


export type SignalSeverity = 'INFO' | 'WARNING' | 'CRITICAL' | 'EMERGENCY';

export interface InterKernelSignal {
  readonly sourceKernelId: string;
  readonly type: string;
  readonly severity: SignalSeverity;
  readonly payload: Record<string, number | string | boolean>;
  /** Ed25519 signature over canonical JSON of {sourceKernelId,type,severity,payload,timestamp,ttlSeconds,nonce}. Base64. */
  readonly signature: string;
  readonly timestamp: bigint;
  readonly ttlSeconds: number;
  /** Random nonce to prevent replay of exact-duplicate signals */
  readonly nonce: string;
}

export type RejectionReason =
  | 'KERNEL_NOT_REGISTERED'
  | 'INVALID_SIGNATURE'
  | 'RATE_LIMITED'
  | 'EXPIRED_BEFORE_SUBMIT'
  | 'DUPLICATE_NONCE';

export interface SignalSubmissionResult {
  readonly accepted: boolean;
  readonly reason?: RejectionReason;
  readonly signal?: InterKernelSignal;
}


export interface Keypair {
  readonly privateKeyPem: string;
  readonly publicKeyPem: string;
}

/** Generate a fresh Ed25519 keypair for a new kernel. */
export function generateKernelKeypair(): Keypair {
  const { privateKey, publicKey } = generateKeyPairSync('ed25519');
  return {
    privateKeyPem: privateKey.export({ type: 'pkcs8', format: 'pem' }).toString(),
    publicKeyPem: publicKey.export({ type: 'spki', format: 'pem' }).toString(),
  };
}

/**
 * Kernel-side signer. Holds the kernel's private key. Signs signals
 * before submission. The bridge never sees the private key.
 */
import { createPrivateKey } from 'crypto';

export class KernelSigner {
  private readonly privateKey: KeyObject;
  public readonly publicKeyPem: string;

  constructor(keypair?: Keypair) {
    const kp = keypair ?? generateKernelKeypair();
    this.privateKey = createPrivateKey(kp.privateKeyPem);
    if (this.privateKey.asymmetricKeyType !== 'ed25519') throw new Error('Only Ed25519 supported');
    this.publicKeyPem = kp.publicKeyPem;
  }

  /** Sign a signal payload. Returns the fully-signed signal. */
  sign(
    sourceKernelId: string,
    type: string,
    severity: SignalSeverity,
    payload: Record<string, number | string | boolean>,
    ttlSeconds = 60,
  ): InterKernelSignal {
    const timestamp = monotonicNow();
    const nonce = generateNonce();
    const body = { sourceKernelId, type, severity, payload, timestamp: timestamp.toString(), ttlSeconds, nonce };
    const signature = edSign(null, Buffer.from(canonicalize(body)), this.privateKey).toString('base64');
    return { sourceKernelId, type, severity, payload, signature, timestamp, ttlSeconds, nonce };
  }
}


function canonicalize(value: unknown): string {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return '[' + value.map(canonicalize).join(',') + ']';
  const obj = value as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return '{' + keys.map(k => JSON.stringify(k) + ':' + canonicalize(obj[k])).join(',') + '}';
}

import { randomBytes } from 'crypto';
function generateNonce(): string { return randomBytes(12).toString('base64'); }


export interface KernelIdentity {
  readonly kernelId: string;
  readonly domain: string;
  readonly capabilities: readonly string[];
  readonly publicKeyPem: string;
  readonly registeredAt: bigint;
}

export interface SignalSubscription {
  readonly subscriberKernelId: string;
  readonly patterns: readonly string[];
  readonly minSeverity: SignalSeverity;
}


interface TokenBucket {
  tokens: number;
  lastRefillMs: number;
}
const DEFAULT_RATE_CAPACITY = 100; // max burst
const DEFAULT_RATE_REFILL_PER_SEC = 20;


const SEVERITY_ORDER: Record<SignalSeverity, number> = {
  INFO: 0, WARNING: 1, CRITICAL: 2, EMERGENCY: 3,
};

export class InterKernelBridge {
  private readonly kernels = new Map<string, KernelIdentity>();
  private readonly publicKeys = new Map<string, KeyObject>();
  private readonly subscriptions = new Map<string, SignalSubscription[]>();
  private readonly rateBuckets = new Map<string, TokenBucket>();
  private readonly seenNonces = new Map<string, Set<string>>(); // per-kernel recent nonces

  // Ring buffer for active signals (O(1) add, no shift)
  private readonly active: (InterKernelSignal | null)[];
  private activeCursor = 0;
  private activeCount = 0;
  // Index by type for O(|type|) receive
  private readonly activeByType = new Map<string, Set<InterKernelSignal>>();
  // Bounded log (ring buffer)
  private readonly log: (InterKernelSignal | null)[];
  private logCursor = 0;
  private readonly logCapacity: number;
  private rejectedCount = 0;

  constructor(
    readonly maxSignals = 1000,
    logCapacity = 10_000,
    private readonly rateCapacity = DEFAULT_RATE_CAPACITY,
    private readonly rateRefillPerSec = DEFAULT_RATE_REFILL_PER_SEC,
    private readonly nonceWindow = 500, // remember N most recent nonces per kernel
  ) {
    this.active = new Array(maxSignals).fill(null);
    this.logCapacity = logCapacity;
    this.log = new Array(logCapacity).fill(null);
  }

  // ─── Kernel Management ──────────────────────────────

  /**
   * Register a kernel. The kernel provides its PUBLIC key only; it keeps
   * its private key locally (via a KernelSigner). The bridge cannot forge
   * signals for this kernel — only the kernel's signer can.
   */
  registerKernel(
    kernelId: string,
    domain: string,
    capabilities: readonly string[],
    publicKeyPem: string,
  ): KernelIdentity {
    const identity: KernelIdentity = {
      kernelId, domain,
      capabilities: [...capabilities],
      publicKeyPem,
      registeredAt: monotonicNow(),
    };
    this.kernels.set(kernelId, identity);
    this.publicKeys.set(kernelId, createPublicKey(publicKeyPem));
    this.rateBuckets.set(kernelId, { tokens: this.rateCapacity, lastRefillMs: Date.now() });
    this.seenNonces.set(kernelId, new Set());
    return identity;
  }

  /**
   * Unregister a kernel. Purges identity, public key, subscriptions,
   * rate bucket, nonce cache. Active signals already in-flight remain
   * readable (they're content-addressed by signature + public key snapshot).
   */
  unregisterKernel(kernelId: string): boolean {
    const existed = this.kernels.delete(kernelId);
    this.publicKeys.delete(kernelId);
    this.subscriptions.delete(kernelId);
    this.rateBuckets.delete(kernelId);
    this.seenNonces.delete(kernelId);
    return existed;
  }

  subscribe(sub: SignalSubscription): void {
    const subs = this.subscriptions.get(sub.subscriberKernelId) ?? [];
    subs.push(sub);
    this.subscriptions.set(sub.subscriberKernelId, subs);
  }

  // ─── Signal Submission (pre-signed by kernel) ───────

  /**
   * Submit a signal that was pre-signed by the source kernel's KernelSigner.
   * The bridge verifies the signature with the stored public key; the bridge
   * cannot forge or modify signals. Rate limited per kernel.
   */
  submit(signal: InterKernelSignal): SignalSubmissionResult {
    const pubKey = this.publicKeys.get(signal.sourceKernelId);
    if (!pubKey) {
      this.rejectedCount++;
      return { accepted: false, reason: 'KERNEL_NOT_REGISTERED' };
    }

    // Rate limit check (before expensive crypto)
    if (!this.consumeToken(signal.sourceKernelId)) {
      this.rejectedCount++;
      return { accepted: false, reason: 'RATE_LIMITED' };
    }

    // Replay / duplicate nonce detection
    const nonces = this.seenNonces.get(signal.sourceKernelId)!;
    if (nonces.has(signal.nonce)) {
      this.rejectedCount++;
      return { accepted: false, reason: 'DUPLICATE_NONCE' };
    }

    // Signature verification over canonical JSON
    if (!this.verifySignal(signal, pubKey)) {
      this.rejectedCount++;
      return { accepted: false, reason: 'INVALID_SIGNATURE' };
    }

    // TTL check (reject if already expired at submit time)
    const ageMs = nsToMs(monotonicNow() - signal.timestamp);
    if (ageMs > signal.ttlSeconds * 1000) {
      this.rejectedCount++;
      return { accepted: false, reason: 'EXPIRED_BEFORE_SUBMIT' };
    }

    // Store — ring buffer
    this.storeSignal(signal);
    // Nonce cache — bounded
    nonces.add(signal.nonce);
    if (nonces.size > this.nonceWindow) {
      const first = nonces.values().next().value;
      if (first !== undefined) nonces.delete(first);
    }
    return { accepted: true, signal };
  }

  private storeSignal(signal: InterKernelSignal): void {
    // Ring buffer for active
    const evicted = this.active[this.activeCursor];
    if (evicted) {
      const bucket = this.activeByType.get(evicted.type);
      bucket?.delete(evicted);
      if (bucket && bucket.size === 0) this.activeByType.delete(evicted.type);
    }
    this.active[this.activeCursor] = signal;
    this.activeCursor = (this.activeCursor + 1) % this.maxSignals;
    if (this.activeCount < this.maxSignals) this.activeCount++;

    let bucket = this.activeByType.get(signal.type);
    if (!bucket) { bucket = new Set(); this.activeByType.set(signal.type, bucket); }
    bucket.add(signal);

    // Ring buffer for log
    this.log[this.logCursor] = signal;
    this.logCursor = (this.logCursor + 1) % this.logCapacity;
  }

  private consumeToken(kernelId: string): boolean {
    const b = this.rateBuckets.get(kernelId);
    if (!b) return false;
    const nowMs = Date.now();
    const elapsed = (nowMs - b.lastRefillMs) / 1000;
    b.tokens = Math.min(this.rateCapacity, b.tokens + elapsed * this.rateRefillPerSec);
    b.lastRefillMs = nowMs;
    if (b.tokens < 1) return false;
    b.tokens -= 1;
    return true;
  }

  // ─── Signal Reception ───────────────────────────────

  /** Get signals for a specific kernel (filtered by subscriptions). */
  receive(kernelId: string): InterKernelSignal[] {
    const subs = this.subscriptions.get(kernelId) ?? [];
    if (subs.length === 0) return [];

    const now = monotonicNow();
    const results: InterKernelSignal[] = [];
    const seen = new Set<InterKernelSignal>();

    for (const sub of subs) {
      const matchingTypes = this.matchingTypes(sub.patterns);
      for (const type of matchingTypes) {
        const bucket = this.activeByType.get(type);
        if (!bucket) continue;
        for (const signal of bucket) {
          if (seen.has(signal)) continue;
          if (signal.sourceKernelId === kernelId) continue;
          if (SEVERITY_ORDER[signal.severity] < SEVERITY_ORDER[sub.minSeverity]) continue;
          if (nsToMs(now - signal.timestamp) > signal.ttlSeconds * 1000) continue;
          results.push(signal);
          seen.add(signal);
        }
      }
    }
    return results;
  }

  /** Resolve subscription glob patterns to the concrete types currently indexed. */
  private matchingTypes(patterns: readonly string[]): string[] {
    const out: string[] = [];
    for (const type of this.activeByType.keys()) {
      for (const p of patterns) {
        if (p === '*' || p === type) { out.push(type); break; }
        if (p.endsWith('.*') && type.startsWith(p.slice(0, -2))) { out.push(type); break; }
      }
    }
    return out;
  }

  // ─── Signal Verification ────────────────────────────

  /** Verify a signal's authenticity with the stored public key. */
  verifySignal(signal: InterKernelSignal, pubKeyOverride?: KeyObject): boolean {
    const pubKey = pubKeyOverride ?? this.publicKeys.get(signal.sourceKernelId);
    if (!pubKey) return false;
    const body = {
      sourceKernelId: signal.sourceKernelId,
      type: signal.type,
      severity: signal.severity,
      payload: signal.payload,
      timestamp: signal.timestamp.toString(),
      ttlSeconds: signal.ttlSeconds,
      nonce: signal.nonce,
    };
    try {
      return edVerify(null, Buffer.from(canonicalize(body)), pubKey, Buffer.from(signal.signature, 'base64'));
    } catch { return false; }
  }

  // ─── Maintenance ────────────────────────────────────

  /** Remove expired signals from active buffer + type index. */
  cleanup(): number {
    const now = monotonicNow();
    let removed = 0;
    for (let i = 0; i < this.maxSignals; i++) {
      const s = this.active[i];
      if (!s) continue;
      if (nsToMs(now - s.timestamp) > s.ttlSeconds * 1000) {
        this.active[i] = null;
        const bucket = this.activeByType.get(s.type);
        bucket?.delete(s);
        if (bucket && bucket.size === 0) this.activeByType.delete(s.type);
        this.activeCount = Math.max(0, this.activeCount - 1);
        removed++;
      }
    }
    return removed;
  }

  // ─── Accessors ──────────────────────────────────────

  getKernelCount(): number { return this.kernels.size; }
  getActiveSignalCount(): number { return this.activeCount; }
  getRejectedCount(): number { return this.rejectedCount; }
  getSignalLog(): InterKernelSignal[] {
    const out: InterKernelSignal[] = [];
    for (let i = 0; i < this.logCapacity; i++) {
      const s = this.log[(this.logCursor + i) % this.logCapacity];
      if (s) out.push(s);
    }
    return out;
  }

  findKernelsByCapability(capability: string): KernelIdentity[] {
    const out: KernelIdentity[] = [];
    for (const k of this.kernels.values()) if (k.capabilities.includes(capability)) out.push(k);
    return out;
  }
}
