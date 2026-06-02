/**
 * Sovereign Node — dedicated unit tests.
 */
import { describe, it, expect } from 'vitest';
import { SovereignNode } from '../sovereign-node.js';
import { agentId, tenantId } from '../../types.js';
import { randomUnit, scale } from '../../physics/vector-math.js';

const DIM = 32;

describe('SovereignNode', () => {
  it('should self-sign and verify', () => {
    const node = new SovereignNode('alpha');
    const signed = node.sign(agentId('a'), tenantId('t'), randomUnit(DIM));
    expect(node.verify(signed).valid).toBe(true);
  });

  it('should reject untrusted nodes', () => {
    const node = new SovereignNode('alpha');
    const rogue = new SovereignNode('rogue');
    expect(node.verify(rogue.sign(agentId('x'), tenantId('t'), randomUnit(DIM))).valid).toBe(false);
  });

  it('should trust then revoke permanently', () => {
    const node = new SovereignNode('a');
    const partner = new SovereignNode('b');
    node.trust(partner.identity);
    expect(node.isTrusted('b')).toBe(true);
    node.revoke('b');
    expect(node.isTrusted('b')).toBe(false);
    expect(() => node.trust(partner.identity)).toThrow('revoked');
  });

  it('should reject magnitude exploit (R20)', () => {
    const node = new SovereignNode('safe', 10);
    const signed = node.sign(agentId('a'), tenantId('t'), scale(randomUnit(DIM), 500));
    expect(node.verify(signed).valid).toBe(false);
    expect(node.verify(signed).reason).toContain('magnitude');
  });

  it('should log violations', () => {
    const node = new SovereignNode('a');
    const rogue = new SovereignNode('r');
    node.verify(rogue.sign(agentId('x'), tenantId('t'), randomUnit(DIM)));
    expect(node.getViolations().length).toBe(1);
  });
});
