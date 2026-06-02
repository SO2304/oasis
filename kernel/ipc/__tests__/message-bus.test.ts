import { describe, it, expect } from 'vitest';
import { MessageBus } from '../message-bus.js';
import { __resetKillSwitchForTesting } from '../../kill-switch.js';
import { tenantId, agentId, MessageType as MT } from '../../types.js';

describe('MessageBus', () => {
  it('should deliver messages to matching subscribers', () => {
    const ks = __resetKillSwitchForTesting();
    const bus = new MessageBus(ks);
    const received: string[] = [];

    bus.subscribe('agent.*', tenantId('t1'), (msg) => { received.push(msg.topic); });
    bus.publishSync(bus.createMessage(MT.EVENT, 'agent.ready', 'KERNEL', 'BROADCAST', {}, tenantId('t1')));
    bus.publishSync(bus.createMessage(MT.EVENT, 'hal.loaded', 'KERNEL', 'BROADCAST', {}, tenantId('t1')));

    expect(received).toEqual(['agent.ready']); // Only matching topic
  });

  it('should isolate tenants (R4)', () => {
    const ks = __resetKillSwitchForTesting();
    const bus = new MessageBus(ks);
    let count = 0;

    bus.subscribe('test', tenantId('tenant-A'), () => { count++; });
    bus.publishSync(bus.createMessage(MT.EVENT, 'test', 'KERNEL', 'BROADCAST', {}, tenantId('tenant-B')));

    expect(count).toBe(0); // Different tenant → not delivered
  });

  it('PANIC bypasses tenant isolation', () => {
    const ks = __resetKillSwitchForTesting();
    const bus = new MessageBus(ks);
    let panicReceived = false;

    bus.subscribe('kernel.panic', tenantId('any'), () => { panicReceived = true; });
    bus.publishSync(bus.createMessage(MT.PANIC, 'kernel.panic', 'KERNEL', 'BROADCAST', {}, '*' as any));

    expect(panicReceived).toBe(true);
  });

  it('tracks message and subscription counts', () => {
    const ks = __resetKillSwitchForTesting();
    const bus = new MessageBus(ks);

    bus.subscribe('a', tenantId('t'), () => {});
    bus.subscribe('b', tenantId('t'), () => {});
    bus.publishSync(bus.createMessage(MT.EVENT, 'a', 'KERNEL', 'BROADCAST', {}, tenantId('t')));

    expect(bus.getSubscriptionCount()).toBe(2);
    expect(bus.getMessageCount()).toBeGreaterThan(0);
  });
});
