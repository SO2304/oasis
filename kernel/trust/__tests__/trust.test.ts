import { describe, it, expect } from 'vitest';
import { TrustEngine } from '../trust-engine.js';
import { __resetKillSwitchForTesting } from '../../kill-switch.js';
import { agentId } from '../../types.js';

describe('TrustEngine — Dedicated', () => {
  it('registers agents with initial trust score', () => {
    const ks = __resetKillSwitchForTesting();
    const trust = new TrustEngine(ks);
    trust.register(agentId('a1'), 'NATIVE');

    const t = trust.getEffectiveTrust(agentId('a1'));
    expect(t.cyber).toBeGreaterThan(0.8);
  });

  it('success increases trust', () => {
    const ks = __resetKillSwitchForTesting();
    const trust = new TrustEngine(ks);
    trust.register(agentId('a1'), 'VERIFIED');

    const before = trust.getEffectiveTrust(agentId('a1')).cyber;
    for (let i = 0; i < 10; i++) trust.recordSuccess(agentId('a1'));
    const after = trust.getEffectiveTrust(agentId('a1')).cyber;

    expect(after).toBeGreaterThanOrEqual(before);
  });

  it('violations decrease trust', () => {
    const ks = __resetKillSwitchForTesting();
    const trust = new TrustEngine(ks);
    trust.register(agentId('a1'), 'ENTERPRISE');

    const before = trust.getEffectiveTrust(agentId('a1')).cyber;
    trust.recordViolation(agentId('a1'), 'TRUST_VIOLATION', 'test');
    const after = trust.getEffectiveTrust(agentId('a1')).cyber;

    expect(after).toBeLessThan(before);
  });

  it('3 physical violations trigger kill switch', () => {
    const ks = __resetKillSwitchForTesting();
    const trust = new TrustEngine(ks);
    trust.register(agentId('bad'), 'EXPERIMENTAL', 'SIM_ONLY');

    trust.recordViolation(agentId('bad'), 'GEOFENCE_BREACH', '1');
    trust.recordViolation(agentId('bad'), 'FORCE_EXCEEDED', '2');
    trust.recordViolation(agentId('bad'), 'GEOFENCE_BREACH', '3');

    expect(ks.isTriggered()).toBe(true);
  });

  it('meetsMinimumTrust filters correctly', () => {
    const ks = __resetKillSwitchForTesting();
    const trust = new TrustEngine(ks);
    trust.register(agentId('high'), 'NATIVE');
    trust.register(agentId('low'), 'EXPERIMENTAL');

    expect(trust.meetsMinimumTrust(agentId('high'), 0.9)).toBe(true);
    expect(trust.meetsMinimumTrust(agentId('low'), 0.9)).toBe(false);
  });
});
