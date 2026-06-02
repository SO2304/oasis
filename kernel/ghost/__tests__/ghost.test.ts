/**
 * OASIS — GHOST-to-CORE Protocol Tests
 *
 * Can an AI command a robot to do something dangerous?
 * Can a robot refuse an unsafe instruction?
 * Does the AI get meaningful feedback?
 *
 * Every test proves the MEDIATION works: the AI proposes,
 * OASIS validates, the robot disposes.
 */

import { describe, it, expect } from 'vitest';
import {
  GhostMediator, type GhostIntention,
  RULE_NO_RED_ZONE, RULE_HUMAN_FORCE_LIMIT,
  RULE_YELLOW_SPEED, RULE_NO_WORK_DURING_EVAC,
} from '../ghost-protocol.js';
import { monotonicNow } from '../../types.js';

function makeIntention(
  ghostId: string, goal: string,
  params: Record<string, number | string | boolean> = {},
  priority = 5,
): GhostIntention {
  return {
    id: `intent-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    ghostId, goal, params, priority, deadlineS: 0, timestamp: monotonicNow(),
  };
}

describe('Identity — GHOST Must Be Registered', () => {
  it('registered GHOST: intention accepted', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('planner-ai', 'Production Planner', 0.9);

    const result = mediator.mediate(makeIntention('planner-ai', 'MOVE', { x: 10, y: 20 }));

    expect(result.verdict).toBe('ACCEPTED');
    expect(result.injected).toBe(true);
  });

  it('unregistered GHOST: intention REFUSED', () => {
    const mediator = new GhostMediator();

    const result = mediator.mediate(makeIntention('rogue-ai', 'HACK', {}));

    expect(result.verdict).toBe('REFUSED');
    expect(result.injected).toBe(false);
    expect(result.reason).toContain('not registered');
  });

  it('low trust GHOST: high priority intention downgraded', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('intern-ai', 'Junior Planner', 0.2); // Low trust

    const intention = makeIntention('intern-ai', 'MOVE', {}, 9); // High priority
    const result = mediator.mediate(intention);

    // Accepted but priority was reduced
    expect(result.verdict).not.toBe('REFUSED');
    expect(result.modifications.some(m => m.includes('Priority'))).toBe(true);
  });
});

describe('Safety Rules — OASIS Protects the CORE', () => {
  it('HARD: intention targeting RED zone → REFUSED', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Planner', 0.9);
    mediator.addSafetyRule(RULE_NO_RED_ZONE);

    const result = mediator.mediate(
      makeIntention('ai', 'INSPECT', { zone: 'furnace-RED', target: 'crack-42' }),
    );

    console.log(
      `[RED ZONE REFUSED]\n` +
      `  Goal: INSPECT furnace-RED\n` +
      `  Verdict: ${result.verdict}\n` +
      `  Reason: ${result.reason}`,
    );

    expect(result.verdict).toBe('REFUSED');
    expect(result.reason).toContain('RED');
  });

  it('HARD: high force near human → REFUSED', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Controller', 0.9);
    mediator.addSafetyRule(RULE_HUMAN_FORCE_LIMIT);

    const result = mediator.mediate(
      makeIntention('ai', 'PUSH', { force: 20, humansNearby: true }),
    );

    expect(result.verdict).toBe('REFUSED');
    expect(result.reason).toContain('Force');
  });

  it('HARD: same force WITHOUT human → ACCEPTED', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Controller', 0.9);
    mediator.addSafetyRule(RULE_HUMAN_FORCE_LIMIT);

    const result = mediator.mediate(
      makeIntention('ai', 'PUSH', { force: 20, humansNearby: false }),
    );

    expect(result.verdict).toBe('ACCEPTED');
  });

  it('SOFT: speed in YELLOW zone → MODIFIED (not refused)', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Navigator', 0.9);
    mediator.addSafetyRule(RULE_YELLOW_SPEED);

    const result = mediator.mediate(
      makeIntention('ai', 'MOVE', { zone: 'section-YELLOW', speed: 2.0 }),
    );

    console.log(
      `[YELLOW ZONE MODIFIED]\n` +
      `  Verdict: ${result.verdict}\n` +
      `  Modifications: ${result.modifications.join('; ')}`,
    );

    expect(result.verdict).toBe('MODIFIED');
    expect(result.injected).toBe(true); // Still injected, but modified
  });

  it('HARD: work during evacuation → REFUSED', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Planner', 0.9);
    mediator.addSafetyRule(RULE_NO_WORK_DURING_EVAC);

    const result = mediator.mediate(
      makeIntention('ai', 'INSPECT', { opStatus: 'EVACUATE' }),
    );

    expect(result.verdict).toBe('REFUSED');
    expect(result.reason).toContain('EVACUATING');
  });

  it('evacuation intention DURING evacuation → ACCEPTED', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Emergency', 0.9);
    mediator.addSafetyRule(RULE_NO_WORK_DURING_EVAC);

    const result = mediator.mediate(
      makeIntention('ai', 'EVACUATE', { opStatus: 'EVACUATE' }),
    );

    expect(result.verdict).toBe('ACCEPTED');
  });
});

describe('Feedback — CORE Reports to GHOST', () => {
  it('GHOST receives semantic feedback, not raw sensor data', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Planner', 0.9);

    const intention = makeIntention('ai', 'MOVE', { x: 50, y: 20 });
    mediator.mediate(intention);

    // CORE reports back
    mediator.recordFeedback({
      intentionId: intention.id,
      progress: 0.6,
      status: 'EXECUTING',
      obstacles: ['wall at 30cm'],
      stress: 0.3,
      entropy: 0.2,
      safetyViolations: [],
      timestamp: monotonicNow(),
    });

    const feedback = mediator.getFeedback(intention.id);

    console.log(
      `[FEEDBACK]\n` +
      `  Progress: ${feedback?.progress ? (feedback.progress * 100).toFixed(0) : 'N/A'}%\n` +
      `  Status: ${feedback?.status}\n` +
      `  Obstacles: ${feedback?.obstacles.join(', ')}\n` +
      `  Stress: ${feedback?.stress}`,
    );

    expect(feedback).not.toBeNull();
    expect(feedback!.progress).toBe(0.6);
    expect(feedback!.status).toBe('EXECUTING');
    expect(feedback!.obstacles).toContain('wall at 30cm');
  });

  it('GHOST sees BLOCKED status when robot cannot proceed', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Planner', 0.9);

    const intention = makeIntention('ai', 'ENTER', { room: 'lab-3' });
    mediator.mediate(intention);

    mediator.recordFeedback({
      intentionId: intention.id,
      progress: 0.0,
      status: 'BLOCKED',
      obstacles: ['door locked', 'no access badge'],
      stress: 0.1,
      entropy: 0.8, // High uncertainty
      safetyViolations: [],
      timestamp: monotonicNow(),
    });

    const fb = mediator.getFeedback(intention.id);

    expect(fb!.status).toBe('BLOCKED');
    expect(fb!.entropy).toBeGreaterThan(0.5);
  });
});

describe('Multi-GHOST Coordination', () => {
  it('two GHOSTs send conflicting intentions: both mediated independently', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('planner-A', 'Line A Planner', 0.9);
    mediator.registerGhost('planner-B', 'Line B Planner', 0.9);
    mediator.addSafetyRule(RULE_NO_RED_ZONE);

    // A wants robot in safe zone
    const resultA = mediator.mediate(
      makeIntention('planner-A', 'MOVE', { zone: 'storage-GREEN', speed: 1.0 }),
    );

    // B wants robot in dangerous zone
    const resultB = mediator.mediate(
      makeIntention('planner-B', 'MOVE', { zone: 'furnace-RED', speed: 2.0 }),
    );

    console.log(
      `[CONFLICTING GHOSTS]\n` +
      `  Planner A (safe zone): ${resultA.verdict}\n` +
      `  Planner B (RED zone): ${resultB.verdict}`,
    );

    expect(resultA.verdict).toBe('ACCEPTED');
    expect(resultB.verdict).toBe('REFUSED');

    const stats = mediator.getStats();
    expect(stats.accepted).toBe(1);
    expect(stats.refused).toBe(1);
  });
});

describe('Integrated Scenario — AI Plans, Robot Executes Safely', () => {
  it('full dialogue: intention → mediation → execution → feedback → adaptation', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('master-ai', 'Master Planner', 0.95);
    mediator.addSafetyRule(RULE_NO_RED_ZONE);
    mediator.addSafetyRule(RULE_HUMAN_FORCE_LIMIT);
    mediator.addSafetyRule(RULE_YELLOW_SPEED);

    // Step 1: AI plans inspection route
    const step1 = mediator.mediate(
      makeIntention('master-ai', 'INSPECT', { zone: 'storage-GREEN', speed: 1.5 }),
    );
    expect(step1.verdict).toBe('ACCEPTED');

    // Step 2: Robot reports progress
    mediator.recordFeedback({
      intentionId: step1.intentionId, progress: 0.5, status: 'EXECUTING',
      obstacles: [], stress: 0.1, entropy: 0.1, safetyViolations: [], timestamp: monotonicNow(),
    });

    // Step 3: AI adjusts — wants to enter YELLOW zone
    const step3 = mediator.mediate(
      makeIntention('master-ai', 'INSPECT', { zone: 'approach-YELLOW', speed: 2.0 }),
    );
    expect(step3.verdict).toBe('MODIFIED'); // Speed reduced

    // Step 4: AI tries RED zone (will be refused)
    const step4 = mediator.mediate(
      makeIntention('master-ai', 'INSPECT', { zone: 'furnace-RED' }),
    );
    expect(step4.verdict).toBe('REFUSED');

    // Step 5: AI adapts — requests remote inspection instead
    const step5 = mediator.mediate(
      makeIntention('master-ai', 'REMOTE_SCAN', { zone: 'furnace-approach-GREEN', distance: 15 }),
    );
    expect(step5.verdict).toBe('ACCEPTED');

    const stats = mediator.getStats();

    console.log(
      `[FULL DIALOGUE]\n` +
      `  Step 1: INSPECT storage → ${step1.verdict}\n` +
      `  Step 3: INSPECT yellow → ${step3.verdict} (speed modified)\n` +
      `  Step 4: INSPECT RED → ${step4.verdict} (REFUSED)\n` +
      `  Step 5: REMOTE_SCAN → ${step5.verdict} (AI adapted)\n` +
      `  Stats: ${stats.accepted} accepted, ${stats.modified} modified, ${stats.refused} refused`,
    );

    expect(stats.accepted).toBe(2);
    expect(stats.modified).toBe(1);
    expect(stats.refused).toBe(1);

    // Full audit trail
    const log = mediator.getLog();
    expect(log.length).toBe(4);
  });
});

describe('Benchmark', () => {
  it('mediation throughput with 4 safety rules', () => {
    const mediator = new GhostMediator();
    mediator.registerGhost('ai', 'Bench', 0.9);
    mediator.addSafetyRule(RULE_NO_RED_ZONE);
    mediator.addSafetyRule(RULE_HUMAN_FORCE_LIMIT);
    mediator.addSafetyRule(RULE_YELLOW_SPEED);
    mediator.addSafetyRule(RULE_NO_WORK_DURING_EVAC);

    const N = 10_000;
    const start = process.hrtime.bigint();
    for (let i = 0; i < N; i++) {
      mediator.mediate(makeIntention('ai', 'MOVE', { speed: 1, zone: 'GREEN' }));
    }
    const usPerMediation = Number(process.hrtime.bigint() - start) / N / 1000;

    console.log(
      `[GHOST MEDIATION BENCHMARK]\n` +
      `  ${usPerMediation.toFixed(1)} μs/mediation\n` +
      `  ${(1e6 / usPerMediation).toFixed(0)} mediations/sec`,
    );

    expect(usPerMediation).toBeLessThan(500); // < 0.5ms
  });
});
