/**
 * OASIS Sovereign — Rate Limiter (R3)
 *
 * Every agent has a tension emission budget.
 * Exceeding the budget = tension rejected + immune alert.
 *
 * Prevents:
 * - Denial of service (agent floods the field)
 * - Resource exhaustion (memory from too many tensions)
 * - Amplification attacks (rogue agent overwhelms others)
 *
 * METHOD: Token bucket algorithm.
 * Each agent gets N tokens per second. Each tension costs 1 token.
 * Tokens refill at a constant rate. Burst allowed up to bucket size.
 */

export interface RateLimitConfig {
  /** Tokens per second (sustained rate) */
  readonly tokensPerSecond: number;
  /** Maximum burst size (bucket capacity) */
  readonly bucketSize: number;
  /** Penalty: seconds of lockout after exceeding limit */
  readonly lockoutSeconds: number;
}

export const DEFAULT_RATE_LIMIT: RateLimitConfig = {
  tokensPerSecond: 50,  // 50 tensions/second sustained
  bucketSize: 200,       // Burst up to 200
  lockoutSeconds: 5,     // 5s lockout on violation
};

interface AgentBucket {
  tokens: number;
  lastRefill: number; // Timestamp in seconds
  lockedUntil: number;
  violations: number;
}

export interface RateLimitResult {
  readonly allowed: boolean;
  readonly remaining: number;
  readonly lockedOut: boolean;
  readonly violation: boolean;
}

export class RateLimiter {
  private readonly config: RateLimitConfig;
  private readonly buckets = new Map<string, AgentBucket>();

  constructor(config: RateLimitConfig = DEFAULT_RATE_LIMIT) {
    this.config = config;
  }

  /** Check if an agent can emit a tension. Consumes 1 token if allowed. */
  check(agentId: string, nowSeconds: number): RateLimitResult {
    let bucket = this.buckets.get(agentId);

    if (!bucket) {
      bucket = {
        tokens: this.config.bucketSize,
        lastRefill: nowSeconds,
        lockedUntil: 0,
        violations: 0,
      };
      this.buckets.set(agentId, bucket);
    }

    // Check lockout
    if (nowSeconds < bucket.lockedUntil) {
      return { allowed: false, remaining: 0, lockedOut: true, violation: false };
    }

    // Refill tokens based on elapsed time
    const elapsed = nowSeconds - bucket.lastRefill;
    bucket.tokens = Math.min(
      this.config.bucketSize,
      bucket.tokens + elapsed * this.config.tokensPerSecond,
    );
    bucket.lastRefill = nowSeconds;

    // Check if token available
    if (bucket.tokens < 1) {
      bucket.violations++;
      bucket.lockedUntil = nowSeconds + this.config.lockoutSeconds;
      return { allowed: false, remaining: 0, lockedOut: false, violation: true };
    }

    // Consume token
    bucket.tokens -= 1;

    return {
      allowed: true,
      remaining: Math.floor(bucket.tokens),
      lockedOut: false,
      violation: false,
    };
  }

  /** Get violation count for an agent */
  getViolations(agentId: string): number {
    return this.buckets.get(agentId)?.violations ?? 0;
  }

  /** Reset an agent's bucket (e.g., after quarantine release) */
  reset(agentId: string): void {
    this.buckets.delete(agentId);
  }
}
