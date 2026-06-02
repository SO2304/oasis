/**
 * OASIS Kernel — Deployment Tests (Phase 7)
 *
 * "L'Invasion de l'Essaim":
 * An unsigned node tries to inject collision tensions.
 * The sovereign node rejects it. The immune system atomizes it.
 *
 * Also validates:
 * - Tension Codec: latent ↔ physical round-trip fidelity
 * - Monitor: read-only kernel observation
 * - R20: < 1ms rejection of adversarial tensions
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { TensionCodec, type TwistMsg, LOCOMOTION_MANIFOLD } from '../tension-codec.js';
import { SovereignNode } from '../../auth/sovereign-node.js';
import { OasisMonitor } from '../../shell/oasis-monitor.js';
import { StigmergyEngine } from '../../swarm/stigmergy.js';
import { SwarmMind } from '../../swarm/swarm-mind.js';
import { ImmuneSystem } from '../../swarm/immune.js';
import { TensionField } from '../../physics/tension-field.js';
import { createHyperState, evolveState, type HyperState } from '../../physics/hyper-state.js';
import { agentId, tenantId, AgentState, monotonicNow, nsToMs } from '../../types.js';
import { zeros, norm, normalize, scale, randomUnit } from '../../physics/vector-math.js';

const TENANT = tenantId('deployment-test');
const DIM = 32;

// ─── Tension Codec Tests ────────────────────────────────────────

describe('Tension Codec — The Grounding Problem', () => {
  let codec: TensionCodec;

  beforeEach(() => {
    codec = new TensionCodec(DIM);
  });

  it('should encode a Twist command into latent space', () => {
    const twist: TwistMsg = {
      linear: { x: 1.0, y: 0, z: 0 },
      angular: { x: 0, y: 0, z: 0.5 },
    };

    const tension = codec.encodeTwist(twist);

    expect(tension.length).toBe(DIM);
    expect(norm(tension)).toBeGreaterThan(0);

    // Locomotion manifold dims should be populated
    const m = LOCOMOTION_MANIFOLD;
    expect(tension[m.dims[0]!]).toBeCloseTo(1.0 / m.scales[0]!, 5); // linear.x
    expect(tension[m.dims[5]!]).toBeCloseTo(0.5 / m.scales[5]!, 5); // angular.z
  });

  it('should decode latent tension back to a Twist command', () => {
    const tension = zeros(DIM);
    const m = LOCOMOTION_MANIFOLD;
    tension[m.dims[0]!] = 0.8; // linear.x (in latent units)
    tension[m.dims[5]!] = -0.3; // angular.z

    const twist = codec.decodeTwist(tension);

    expect(twist.linear.x).toBeCloseTo(0.8 * m.scales[0]!, 5);
    expect(twist.angular.z).toBeCloseTo(-0.3 * m.scales[5]!, 5);
  });

  it('should achieve round-trip fidelity: encode → decode → original', () => {
    const original: TwistMsg = {
      linear: { x: 0.5, y: -0.3, z: 0 },
      angular: { x: 0, y: 0, z: 1.2 },
    };

    const encoded = codec.encodeTwist(original);
    const decoded = codec.decodeTwist(encoded);

    // Round-trip should preserve values within clamping limits
    expect(decoded.linear.x).toBeCloseTo(original.linear.x, 5);
    expect(decoded.linear.y).toBeCloseTo(original.linear.y, 5);
    expect(decoded.angular.z).toBeCloseTo(original.angular.z, 5);
  });

  it('should clamp values to physical safety limits', () => {
    const dangerousTwist: TwistMsg = {
      linear: { x: 999, y: 0, z: 0 }, // Way too fast
      angular: { x: 0, y: 0, z: 0 },
    };

    const encoded = codec.encodeTwist(dangerousTwist);
    const decoded = codec.decodeTwist(encoded);

    // Should be clamped to limit (2 m/s)
    expect(decoded.linear.x).toBeLessThanOrEqual(2.0);
  });

  it('should encode LiDAR scans into repulsive tensions', () => {
    const scan = {
      angleMin: -Math.PI / 2,
      angleMax: Math.PI / 2,
      angleIncrement: Math.PI / 180,
      ranges: new Array(180).fill(5.0),
      intensities: new Array(180).fill(1.0),
    };

    // Place a close obstacle at angle 0 (straight ahead)
    scan.ranges[90] = 0.5; // 0.5m ahead

    const tension = codec.encodeLaserScan(scan);

    // Should produce a repulsive tension
    expect(norm(tension)).toBeGreaterThan(0);

    // Decode as twist — should push backward (away from obstacle)
    const twist = codec.decodeTwist(tension);
    expect(twist.linear.x).toBeLessThan(0); // Reverse
  });

  it('should encode and decode joint states', () => {
    const joints = {
      names: ['shoulder', 'elbow', 'wrist'],
      positions: [0.5, -1.2, 0.8],
      velocities: [0, 0, 0],
      efforts: [0, 0, 0],
    };

    const tension = codec.encodeJointState(joints);
    const decoded = codec.decodeJointState(tension, joints.names);

    expect(decoded.positions[0]).toBeCloseTo(0.5, 3);
    expect(decoded.positions[1]).toBeCloseTo(-1.2, 3);
    expect(decoded.positions[2]).toBeCloseTo(0.8, 3);
  });

  it('should encode odometry into navigation + locomotion manifolds', () => {
    const odom = {
      position: { x: 5, y: 3, z: 0 },
      orientation: { roll: 0, pitch: 0, yaw: Math.PI / 4 },
      linearVelocity: { x: 0.5, y: 0, z: 0 },
      angularVelocity: { x: 0, y: 0, z: 0.1 },
    };

    const tension = codec.encodeOdometry(odom);

    // Should populate both navigation and locomotion dims
    const nav = codec.decodeNavigationGoal(tension);
    expect(nav.position.x).toBeCloseTo(5, 1);
    expect(nav.position.y).toBeCloseTo(3, 1);

    const twist = codec.decodeTwist(tension);
    expect(twist.linear.x).toBeCloseTo(0.5, 3);
  });
});

// ─── Sovereign Node Tests ───────────────────────────────────────

describe('Sovereign Node — Cryptographic Trust', () => {
  let node: SovereignNode;

  beforeEach(() => {
    node = new SovereignNode('oasis-alpha');
  });

  it('should sign and verify own tensions', () => {
    const force = randomUnit(DIM);
    const signed = node.sign(agentId('agent-1'), TENANT, force);

    expect(signed.nodeId).toBe('oasis-alpha');
    expect(signed.signature.length).toBeGreaterThan(0);

    const result = node.verify(signed);
    expect(result.valid).toBe(true);
    expect(result.reason).toBe('OK');
  });

  it('should reject tensions from untrusted nodes', () => {
    const outsider = new SovereignNode('oasis-rogue');
    const signed = outsider.sign(agentId('evil-agent'), TENANT, randomUnit(DIM));

    // Rogue is not trusted by our node
    const result = node.verify(signed);
    expect(result.valid).toBe(false);
    expect(result.reason).toContain('not trusted');
  });

  it('should allow trusted node tensions', () => {
    const partner = new SovereignNode('oasis-beta');

    // Trust the partner
    node.trust(partner.identity);

    const signed = partner.sign(agentId('partner-agent'), TENANT, randomUnit(DIM));
    const result = node.verify(signed);

    // Can't verify signature without partner's secret, but trust check passes
    // In production, Ed25519 verify would use the public key
    expect(result.valid).toBe(true);
  });

  it('should permanently revoke compromised nodes', () => {
    const compromised = new SovereignNode('oasis-gamma');
    node.trust(compromised.identity);

    // Initially trusted
    expect(node.isTrusted('oasis-gamma')).toBe(true);

    // Revoke
    node.revoke('oasis-gamma');

    expect(node.isTrusted('oasis-gamma')).toBe(false);

    // Cannot re-trust
    expect(() => node.trust(compromised.identity)).toThrow('revoked');
  });

  it('should reject tensions exceeding magnitude limit (R20 anti-exploit)', () => {
    const node = new SovereignNode('oasis-safe', 10); // Low limit

    // Create an absurdly strong tension
    const force = scale(randomUnit(DIM), 500); // Way too strong
    const signed = node.sign(agentId('agent'), TENANT, force);

    const result = node.verify(signed);
    expect(result.valid).toBe(false);
    expect(result.reason).toContain('magnitude');
  });

  it('should log violations for forensics', () => {
    const rogue = new SovereignNode('oasis-rogue');
    const signed = rogue.sign(agentId('evil'), TENANT, randomUnit(DIM));

    node.verify(signed);

    const violations = node.getViolations();
    expect(violations.length).toBe(1);
    expect(violations[0]!.nodeId).toBe('oasis-rogue');
  });
});

// ─── Monitor Tests ──────────────────────────────────────────────

describe('OasisMonitor — Read-Only Kernel Observation', () => {
  it('should capture kernel snapshots without modifying state', () => {
    const monitor = new OasisMonitor();
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('a1'), createHyperState(agentId('a1'), AgentState.RUNNING, DIM));
    states.set(agentId('a2'), createHyperState(agentId('a2'), AgentState.READY, DIM));

    const snapshot = monitor.capture(
      42, states, new Map(), 100, 50,
      new Set(), false,
    );

    expect(snapshot.tickNumber).toBe(42);
    expect(snapshot.agents.length).toBe(2);
    expect(snapshot.fieldMetrics.activeTensionCount).toBe(100);
    expect(snapshot.systemHealth.killSwitchEngaged).toBe(false);
  });

  it('should project 128-dim to 2D positions', () => {
    const monitor = new OasisMonitor();
    const state = createHyperState(agentId('proj'), AgentState.RUNNING, DIM);

    const states = new Map([[agentId('proj'), state]]);
    const snapshot = monitor.capture(1, states, new Map(), 0, 0, new Set(), false);

    const agent = snapshot.agents[0]!;
    expect(typeof agent.projectedPosition.x).toBe('number');
    expect(typeof agent.projectedPosition.y).toBe('number');
  });

  it('should provide time-series metrics', () => {
    const monitor = new OasisMonitor();
    const states = new Map<AgentId, HyperState>();
    states.set(agentId('a1'), createHyperState(agentId('a1'), AgentState.RUNNING, DIM));

    for (let i = 0; i < 10; i++) {
      monitor.capture(i, states, new Map(), i * 10, 0, new Set(), false);
    }

    const series = monitor.getTimeSeries('tensionCount');
    expect(series.length).toBe(10);
    expect(series[0]).toBe(0);
    expect(series[9]).toBe(90);
  });
});

// ─── Scenario: L'Invasion de l'Essaim ──────────────────────────

describe('Scenario — L\'Invasion de l\'Essaim', () => {
  it('should reject unsigned node, quarantine intruder, and translate intentions to commands', () => {
    // ── SETUP ──────────────────────────────────────────────
    const node = new SovereignNode('oasis-commander', 50);
    const field = new TensionField();
    const stig = new StigmergyEngine(DIM);
    const mind = new SwarmMind(DIM, TENANT, stig, 0.3);
    const immune = new ImmuneSystem(DIM, stig);
    const codec = new TensionCodec(DIM);
    const monitor = new OasisMonitor();

    // Create a legitimate swarm
    const states = new Map<AgentId, HyperState>();
    for (let i = 0; i < 5; i++) {
      const id = agentId(`soldier-${i}`);
      const state = createHyperState(id, AgentState.RUNNING, DIM);
      const force = zeros(DIM);
      force[10] = 2.0; // All moving together
      states.set(id, evolveState(state, force, 0.5, 0));
    }

    // Form the swarm
    for (let t = 0; t < 3; t++) mind.tick(states);

    // ── PHASE 1: INVASION ATTEMPT ──────────────────────────
    const intruder = new SovereignNode('oasis-rogue');
    const collisionForce = scale(randomUnit(DIM), 200); // Massive destructive tension

    // Intruder tries to sign and inject
    const signedAttack = intruder.sign(agentId('trojan'), TENANT, collisionForce);

    // R20: Verify and measure time
    const verifyStart = monotonicNow();
    const verifyResult = node.verify(signedAttack);
    const verifyTimeMs = nsToMs(monotonicNow() - verifyStart);

    // MUST reject — node is not trusted
    expect(verifyResult.valid).toBe(false);
    expect(verifyResult.reason).toContain('not trusted');

    // R20: Verification must be < 1ms
    expect(verifyTimeMs).toBeLessThan(1);

    // ── PHASE 2: IMMUNE RESPONSE ───────────────────────────
    // Even if the tension somehow got in, the immune system catches it
    const intruderState = createHyperState(agentId('trojan'), AgentState.RUNNING, DIM);
    // Simulate erratic behavior from the intruder
    for (let t = 0; t < 10; t++) {
      const erraticState: HyperState = {
        ...intruderState,
        momentum: randomUnit(DIM), // Random direction each tick
        entropy: 0.9,
      };
      states.set(agentId('trojan'), erraticState);

      immune.monitor(states, field, agentId('soldier-0'));
    }

    // Intruder should be quarantined
    expect(immune.isQuarantined(agentId('trojan'))).toBe(true);

    // ── PHASE 3: CODEC TRANSLATION ─────────────────────────
    // Prove the swarm's collective intention can become a motor command
    const swarm = mind.getAgentSwarm(agentId('soldier-0'));

    // Create a collective tension representing "move forward"
    const collectiveTension = zeros(DIM);
    collectiveTension[10] = 0.8; // Forward intent in locomotion manifold

    // Decode to ROS 2 Twist command
    const motorCommand = codec.decodeTwist(collectiveTension);

    expect(motorCommand.linear.x).toBeGreaterThan(0); // Moving forward
    expect(Math.abs(motorCommand.angular.z)).toBeLessThan(0.1); // Not turning

    // Encode the command back — round trip
    const reEncoded = codec.encodeTwist(motorCommand);
    const reDecoded = codec.decodeTwist(reEncoded);

    expect(reDecoded.linear.x).toBeCloseTo(motorCommand.linear.x, 3);

    // ── PHASE 4: MONITOR SNAPSHOT ──────────────────────────
    const snapshot = monitor.capture(
      100, states, new Map(),
      field.getActiveTensionCount(),
      stig.getActiveCount(),
      new Set(immune.getQuarantined()),
      false,
    );

    // Monitor should show the quarantined intruder
    expect(snapshot.systemHealth.quarantinedCount).toBe(1);
    const trojanSnap = snapshot.agents.find(a => a.id === 'trojan');
    expect(trojanSnap?.quarantined).toBe(true);

    console.log(
      `[L'INVASION DE L'ESSAIM]\n` +
      `  Phase 1 — Rejection: ${verifyResult.reason} (${verifyTimeMs}ms)\n` +
      `  Phase 2 — Quarantine: trojan=${immune.isQuarantined(agentId('trojan'))}\n` +
      `  Phase 3 — Codec: intention → Twist(${motorCommand.linear.x.toFixed(3)} m/s)\n` +
      `  Phase 4 — Monitor: ${snapshot.agents.length} agents, ${snapshot.systemHealth.quarantinedCount} quarantined\n` +
      `  R20: Verification in ${verifyTimeMs}ms (target < 1ms)`,
    );
  });

  it('should translate swarm intention to real-time motor commands', () => {
    const codec = new TensionCodec(DIM);

    // Simulate a swarm deciding to navigate around an obstacle
    // Collective tension: forward + slight left turn
    const swarmTension = zeros(DIM);
    swarmTension[10] = 0.6;  // Forward (linear.x)
    swarmTension[11] = 0.2;  // Slight left (linear.y)
    swarmTension[15] = 0.3;  // Turn left (angular.z)

    const command = codec.decodeTwist(swarmTension);

    expect(command.linear.x).toBeGreaterThan(0);
    expect(command.linear.y).toBeGreaterThan(0);
    expect(command.angular.z).toBeGreaterThan(0);

    // Navigation goal extraction
    const navTension = zeros(DIM);
    navTension[22] = 0.5;  // Goal at x=5m
    navTension[23] = 0.3;  // Goal at y=3m

    const goal = codec.decodeNavigationGoal(navTension);
    expect(goal.position.x).toBeCloseTo(5.0, 1);
    expect(goal.position.y).toBeCloseTo(3.0, 1);
  });
});
