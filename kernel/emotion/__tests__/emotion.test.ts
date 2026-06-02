/**
 * EmotionalField — dedicated unit tests.
 */
import { describe, it, expect } from 'vitest';
import { EmotionalField, EmotionType } from '../emotional-field.js';
import { createHyperState, type HyperState } from '../../physics/hyper-state.js';
import { agentId, AgentState } from '../../types.js';
import { zeros, norm } from '../../physics/vector-math.js';

const DIM = 32;

describe('EmotionalField', () => {
  it('should start CALM', () => {
    const emo = new EmotionalField(DIM);
    const id = agentId('a');
    emo.register(id);
    const state = createHyperState(id, AgentState.RUNNING, DIM);
    const e = emo.update(id, state);
    expect(e.dominant).toBe(EmotionType.CALM);
  });

  it('should build curiosity at moderate entropy', () => {
    const emo = new EmotionalField(DIM);
    const id = agentId('curious');
    emo.register(id);
    const state: HyperState = { ...createHyperState(id, AgentState.RUNNING, DIM), entropy: 0.5 };
    for (let i = 0; i < 15; i++) emo.update(id, state);
    expect(emo.getState(id)!.curiosity).toBeGreaterThan(0.3);
  });

  it('should build fear from pain memory proximity', () => {
    const emo = new EmotionalField(DIM);
    const id = agentId('afraid');
    emo.register(id);
    const painPos = zeros(DIM); painPos[10] = 1;
    emo.recordPain(id, painPos, 0.9);

    const nearPain: HyperState = {
      ...createHyperState(id, AgentState.RUNNING, DIM),
      position: (() => { const p = zeros(DIM); p[10] = 0.5; return p; })(),
    };
    emo.update(id, nearPain);
    expect(emo.getState(id)!.fear).toBeGreaterThan(0);
  });

  it('should build satisfaction from goal progress', () => {
    const emo = new EmotionalField(DIM);
    const id = agentId('happy');
    emo.register(id);
    emo.recordGoalDistance(id, 10);
    emo.recordGoalDistance(id, 7);
    emo.recordGoalDistance(id, 3);
    emo.update(id, createHyperState(id, AgentState.RUNNING, DIM));
    expect(emo.getState(id)!.satisfaction).toBeGreaterThan(0);
  });

  it('should build frustration from prediction errors', () => {
    const emo = new EmotionalField(DIM);
    const id = agentId('stuck');
    emo.register(id);
    for (let i = 0; i < 10; i++) emo.recordPredictionError(id, 0.6);
    for (let i = 0; i < 10; i++) emo.update(id, createHyperState(id, AgentState.RUNNING, DIM));
    expect(emo.getState(id)!.frustration).toBeGreaterThan(0);
  });

  it('should modulate gain based on emotions', () => {
    const emo = new EmotionalField(DIM);
    const id = agentId('modulated');
    emo.register(id);
    emo.recordPain(id, zeros(DIM), 1);
    emo.update(id, createHyperState(id, AgentState.RUNNING, DIM));
    const gain = emo.computeGain(id);
    expect(gain.repulsionGain).toBeGreaterThan(1);
  });

  it('should produce emotional force vector', () => {
    const emo = new EmotionalField(DIM);
    const id = agentId('forceful');
    emo.register(id);
    const painPos = zeros(DIM); painPos[10] = 1;
    emo.recordPain(id, painPos, 1);
    const nearPain: HyperState = {
      ...createHyperState(id, AgentState.RUNNING, DIM),
      position: (() => { const p = zeros(DIM); p[10] = 0.5; return p; })(),
    };
    emo.update(id, nearPain);
    const force = emo.computeEmotionalForce(id, nearPain);
    expect(norm(force)).toBeGreaterThan(0);
  });
});
