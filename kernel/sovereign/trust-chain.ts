/**
 * OASIS Sovereign — Trust Chain Verifier
 *
 * Every tension that enters the shared field MUST be verified:
 * 1. Is the source agent registered? (identity)
 * 2. Is it within its rate limit? (R3)
 * 3. Is the tension magnitude within bounds? (R20)
 * 4. Is the agent currently quarantined? (immune system)
 *
 * If ANY check fails, the tension is REJECTED and the event
 * is logged in the audit trail.
 *
 * This is the GATE between agents and the tension field.
 * Nothing gets through without passing ALL checks.
 */

import { AuditLog } from './audit-log.js';
import { RateLimiter, type RateLimitConfig, DEFAULT_RATE_LIMIT } from './rate-limiter.js';

export interface TrustCheckResult {
  readonly allowed: boolean;
  readonly reason: string;
  readonly agentId: string;
}

export class TrustChain {
  private readonly registeredAgents = new Set<string>();
  private readonly quarantined = new Set<string>();
  private readonly rateLimiter: RateLimiter;
  private readonly auditLog: AuditLog;
  private readonly maxTensionMagnitude: number;
  private rejectionCount = 0;

  constructor(
    auditLog: AuditLog,
    rateConfig: RateLimitConfig = DEFAULT_RATE_LIMIT,
    maxTensionMagnitude = 100,
  ) {
    this.auditLog = auditLog;
    this.rateLimiter = new RateLimiter(rateConfig);
    this.maxTensionMagnitude = maxTensionMagnitude;
  }

  /** Register an agent (must be done before it can emit tensions) */
  register(agentId: string): void {
    this.registeredAgents.add(agentId);
    this.auditLog.append(agentId, 'REGISTER', {});
  }

  /** Quarantine an agent (blocked from emitting) */
  quarantine(agentId: string, reason: string): void {
    this.quarantined.add(agentId);
    this.auditLog.append(agentId, 'QUARANTINE', { reason });
  }

  /** Release from quarantine */
  release(agentId: string): void {
    this.quarantined.delete(agentId);
    this.rateLimiter.reset(agentId);
    this.auditLog.append(agentId, 'RELEASE', {});
  }

  /**
   * Verify a tension before it enters the field.
   *
   * @param agentId Source agent
   * @param tensionMagnitude Magnitude of the tension vector
   * @param nowSeconds Current time in seconds
   */
  verify(agentId: string, tensionMagnitude: number, nowSeconds: number): TrustCheckResult {
    // 1. Identity check
    if (!this.registeredAgents.has(agentId)) {
      this.reject(agentId, 'UNREGISTERED');
      return { allowed: false, reason: 'Agent not registered', agentId };
    }

    // 2. Quarantine check
    if (this.quarantined.has(agentId)) {
      this.reject(agentId, 'QUARANTINED');
      return { allowed: false, reason: 'Agent quarantined', agentId };
    }

    // 3. Rate limit check (R3)
    const rateResult = this.rateLimiter.check(agentId, nowSeconds);
    if (!rateResult.allowed) {
      const reason = rateResult.lockedOut ? 'Rate limit lockout' : 'Rate limit exceeded';
      this.reject(agentId, 'RATE_LIMIT');
      return { allowed: false, reason, agentId };
    }

    // 4. Magnitude check (R20)
    if (tensionMagnitude > this.maxTensionMagnitude) {
      this.reject(agentId, 'MAGNITUDE_EXCEEDED');
      return { allowed: false, reason: `Magnitude ${tensionMagnitude.toFixed(1)} > ${this.maxTensionMagnitude}`, agentId };
    }

    // All checks passed
    this.auditLog.append(agentId, 'TENSION_EMITTED', { magnitude: tensionMagnitude });
    return { allowed: true, reason: 'OK', agentId };
  }

  /** Get total rejections */
  getRejectionCount(): number { return this.rejectionCount; }

  /** Get rate limit violations for an agent */
  getRateLimitViolations(agentId: string): number {
    return this.rateLimiter.getViolations(agentId);
  }

  /** Get the audit log */
  getAuditLog(): AuditLog { return this.auditLog; }

  private reject(agentId: string, reason: string): void {
    this.rejectionCount++;
    this.auditLog.append(agentId, 'REJECTED', { reason });
  }
}
