/**
 * OASIS Bridge tests — sovereign kernels, Ed25519 signatures,
 * rate limiting, ring-buffer active signals, bounded log, unregister.
 */
import { describe, it, expect } from 'vitest';
import {
  InterKernelBridge,
  KernelSigner,
  generateKernelKeypair,
  type InterKernelSignal,
} from '../inter-kernel.js';

/** Test helper: register a kernel and return its signer. */
function bootstrapKernel(
  bridge: InterKernelBridge,
  id: string,
  domain: string,
  caps: readonly string[] = [],
): KernelSigner {
  const signer = new KernelSigner();
  bridge.registerKernel(id, domain, caps, signer.publicKeyPem);
  return signer;
}

describe('Inter-Kernel Bridge — P1 Semantic Mesh', () => {
  it('4 kernels register and receive sovereign identities', () => {
    const bridge = new InterKernelBridge();
    const mine = bootstrapKernel(bridge, 'mine-01', 'MINE', ['GAS_MONITORING', 'STRUCTURAL']);
    bootstrapKernel(bridge, 'drone-swarm-01', 'DRONE', ['SURVEY', 'DELIVERY']);
    bootstrapKernel(bridge, 'fleet-01', 'FLEET', ['TRANSPORT', 'DELIVERY']);
    bootstrapKernel(bridge, 'steel-01', 'STEEL', ['INSPECTION', 'MAINTENANCE']);

    expect(bridge.getKernelCount()).toBe(4);
    expect(mine.publicKeyPem).toContain('BEGIN PUBLIC KEY');
  });

  it('signal emitted + received via canonical subscription', () => {
    const bridge = new InterKernelBridge();
    const mine = bootstrapKernel(bridge, 'mine-01', 'MINE', ['GAS_MONITORING']);
    bootstrapKernel(bridge, 'drone-01', 'DRONE', ['SURVEY']);

    bridge.subscribe({
      subscriberKernelId: 'drone-01',
      patterns: ['GAS.*', 'EMERGENCY.*'],
      minSeverity: 'WARNING',
    });

    const result = bridge.submit(
      mine.sign('mine-01', 'GAS.CH4', 'CRITICAL', { ch4Percent: 2.5, zone: 'B-12' }),
    );
    expect(result.accepted).toBe(true);

    const signals = bridge.receive('drone-01');
    expect(signals.length).toBe(1);
    expect(signals[0]!.type).toBe('GAS.CH4');
    expect(signals[0]!.payload['ch4Percent']).toBe(2.5);
  });

  it('severity filter — INFO dropped when WARNING+ required', () => {
    const bridge = new InterKernelBridge();
    const steel = bootstrapKernel(bridge, 'steel-01', 'STEEL');
    bootstrapKernel(bridge, 'fleet-01', 'FLEET');

    bridge.subscribe({
      subscriberKernelId: 'fleet-01',
      patterns: ['*'],
      minSeverity: 'WARNING',
    });

    bridge.submit(steel.sign('steel-01', 'STATUS.PRODUCTION', 'INFO', { phase: 'MELT', temp: 1500 }));
    expect(bridge.receive('fleet-01').length).toBe(0);
  });

  it('kernel does NOT receive its own signals', () => {
    const bridge = new InterKernelBridge();
    const self = bootstrapKernel(bridge, 'self', 'TEST');

    bridge.subscribe({
      subscriberKernelId: 'self',
      patterns: ['*'],
      minSeverity: 'INFO',
    });
    bridge.submit(self.sign('self', 'TEST', 'INFO', {}));
    expect(bridge.receive('self').length).toBe(0);
  });
});

describe('Sovereignty (R4, R20) — No Kernel Can Be Compromised', () => {
  it('unregistered kernel cannot submit signals', () => {
    const bridge = new InterKernelBridge();
    const rogue = new KernelSigner();
    const res = bridge.submit(rogue.sign('rogue', 'ATTACK', 'EMERGENCY', { payload: 'malicious' }));
    expect(res.accepted).toBe(false);
    expect(res.reason).toBe('KERNEL_NOT_REGISTERED');
    expect(bridge.getRejectedCount()).toBe(1);
  });

  it('forged signal detected by Ed25519 verification', () => {
    const bridge = new InterKernelBridge();
    const legit = bootstrapKernel(bridge, 'legit', 'MINE');
    const original = legit.sign('legit', 'GAS.CO', 'WARNING', { co: 30 });
    const submitted = bridge.submit(original);
    expect(submitted.accepted).toBe(true);

    // Mutate payload — signature should no longer match
    const forged: InterKernelSignal = { ...original, payload: { co: 0 } };
    expect(bridge.verifySignal(forged)).toBe(false);
    expect(bridge.verifySignal(original)).toBe(true);
  });

  it('bridge holds only public keys (cannot forge)', () => {
    const bridge = new InterKernelBridge();
    const legitKp = generateKernelKeypair();
    const legit = new KernelSigner(legitKp);
    bridge.registerKernel('legit', 'MINE', [], legit.publicKeyPem);

    // An attacker who compromises the bridge only has public keys.
    // They cannot produce a valid signature for 'legit' without the private key.
    const attackerSigner = new KernelSigner(); // different keypair
    const spoofed = attackerSigner.sign('legit', 'GAS.CO', 'EMERGENCY', { co: 9999 });
    const res = bridge.submit(spoofed);
    expect(res.accepted).toBe(false);
    expect(res.reason).toBe('INVALID_SIGNATURE');
  });

  it('duplicate nonce rejected (replay prevention)', () => {
    const bridge = new InterKernelBridge();
    const k = bootstrapKernel(bridge, 'k', 'TEST');
    const s = k.sign('k', 'HELLO', 'INFO', {});
    expect(bridge.submit(s).accepted).toBe(true);
    const res = bridge.submit(s);
    expect(res.accepted).toBe(false);
    expect(res.reason).toBe('DUPLICATE_NONCE');
  });
});

describe('DoS resistance', () => {
  it('rate limiting rejects burst beyond capacity', () => {
    const bridge = new InterKernelBridge(1000, 10_000, /*cap*/ 5, /*refill/s*/ 1);
    const k = bootstrapKernel(bridge, 'spammer', 'TEST');

    let accepted = 0; let rateLimited = 0;
    for (let i = 0; i < 20; i++) {
      const r = bridge.submit(k.sign('spammer', 'SPAM', 'INFO', { i }));
      if (r.accepted) accepted++;
      if (r.reason === 'RATE_LIMITED') rateLimited++;
    }
    expect(accepted).toBeLessThanOrEqual(5);
    expect(rateLimited).toBeGreaterThanOrEqual(10);
  });

  it('active-signal ring buffer evicts oldest without shift()', () => {
    const bridge = new InterKernelBridge(/*maxSignals*/ 5, 1000, 10_000, 10_000);
    const k = bootstrapKernel(bridge, 'k', 'TEST');
    for (let i = 0; i < 12; i++) {
      bridge.submit(k.sign('k', `T.${i}`, 'INFO', { i }));
    }
    expect(bridge.getActiveSignalCount()).toBe(5);
  });

  it('unregister purges identity and rejects subsequent signals', () => {
    const bridge = new InterKernelBridge();
    const k = bootstrapKernel(bridge, 'k', 'TEST');
    expect(bridge.unregisterKernel('k')).toBe(true);
    expect(bridge.unregisterKernel('k')).toBe(false);
    const res = bridge.submit(k.sign('k', 'T', 'INFO', {}));
    expect(res.accepted).toBe(false);
    expect(res.reason).toBe('KERNEL_NOT_REGISTERED');
  });
});

describe('Cross-Domain Scenarios', () => {
  it('offshore storm → fleet reroutes → drones ground', () => {
    const bridge = new InterKernelBridge();
    const offshore = bootstrapKernel(bridge, 'offshore-01', 'OFFSHORE', ['WEATHER', 'GAS_MONITORING']);
    bootstrapKernel(bridge, 'fleet-supply', 'FLEET', ['TRANSPORT']);
    bootstrapKernel(bridge, 'drone-survey', 'DRONE', ['SURVEY']);

    bridge.subscribe({ subscriberKernelId: 'fleet-supply', patterns: ['WEATHER.*', 'STATUS.*'], minSeverity: 'WARNING' });
    bridge.subscribe({ subscriberKernelId: 'drone-survey', patterns: ['WEATHER.*'], minSeverity: 'WARNING' });

    bridge.submit(offshore.sign('offshore-01', 'WEATHER.STORM', 'CRITICAL', {
      windKnots: 55, waveHeight: 7.0, heliOps: false,
    }));

    const fleetSignals = bridge.receive('fleet-supply');
    const droneSignals = bridge.receive('drone-survey');
    expect(fleetSignals.length).toBe(1);
    expect(droneSignals.length).toBe(1);
    expect(fleetSignals[0]!.severity).toBe('CRITICAL');
  });

  it('mine gas leak → drones survey → steel secures downwind', () => {
    const bridge = new InterKernelBridge();
    const mine = bootstrapKernel(bridge, 'mine', 'MINE', ['GAS_MONITORING']);
    bootstrapKernel(bridge, 'drones', 'DRONE', ['SURVEY', 'GAS_DETECTION']);
    bootstrapKernel(bridge, 'steel', 'STEEL', ['INSPECTION']);

    bridge.subscribe({ subscriberKernelId: 'drones', patterns: ['GAS.*'], minSeverity: 'WARNING' });
    bridge.subscribe({ subscriberKernelId: 'steel', patterns: ['GAS.*'], minSeverity: 'CRITICAL' });

    bridge.submit(mine.sign('mine', 'GAS.CH4', 'EMERGENCY', { ch4: 4.2, zone: 'A-07' }));
    expect(bridge.receive('drones').length).toBe(1);
    expect(bridge.receive('steel').length).toBe(1);
  });
});

describe('Capability Discovery', () => {
  it('find kernels by capability', () => {
    const bridge = new InterKernelBridge();
    bootstrapKernel(bridge, 'mine-01', 'MINE', ['GAS_MONITORING']);
    bootstrapKernel(bridge, 'drone-01', 'DRONE', ['SURVEY', 'GAS_DETECTION']);
    bootstrapKernel(bridge, 'drone-02', 'DRONE', ['SURVEY']);

    const gasCapable = bridge.findKernelsByCapability('GAS_DETECTION');
    expect(gasCapable.length).toBe(1);
    expect(gasCapable[0]!.kernelId).toBe('drone-01');
    expect(gasCapable[0]!.publicKeyPem).toContain('BEGIN PUBLIC KEY');
  });
});

describe('Cleanup', () => {
  it('expired signals removed by cleanup()', async () => {
    const bridge = new InterKernelBridge();
    const k = bootstrapKernel(bridge, 'k', 'TEST');
    bridge.submit(k.sign('k', 'SHORT', 'INFO', {}, /*ttl*/ 0.05));
    expect(bridge.getActiveSignalCount()).toBe(1);
    await new Promise(r => setTimeout(r, 100));
    const removed = bridge.cleanup();
    expect(removed).toBe(1);
    expect(bridge.getActiveSignalCount()).toBe(0);
  });

  it('bounded signal log (ring buffer) does not leak memory', () => {
    const bridge = new InterKernelBridge(1000, /*logCap*/ 10, 100_000, 100_000);
    const k = bootstrapKernel(bridge, 'k', 'TEST');
    for (let i = 0; i < 50; i++) {
      bridge.submit(k.sign('k', `T.${i}`, 'INFO', { i }));
    }
    expect(bridge.getSignalLog().length).toBeLessThanOrEqual(10);
  });
});
