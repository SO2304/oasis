/**
 * OASIS Browser Entry Point
 *
 * Exports ONLY the browser-safe parts of the kernel.
 * No crypto, no fs, no path.
 * The phone IS the robot.
 */

// Core
export * from './kernel/types.js';
export { KillSwitch, getKillSwitch } from './kernel/kill-switch.js';

// Physics
export * from './kernel/physics/vector-math.js';
export { createHyperState, evolveState, calculateEntropy, isActionSafe } from './kernel/physics/hyper-state.js';
export { TensionField } from './kernel/physics/tension-field.js';
export { LatentEngine } from './kernel/physics/latent-engine.js';
export { WorldModel } from './kernel/physics/world-model.js';

// Perception
export { SensorFusion } from './kernel/perception/sensor-fusion.js';

// Swarm
export { StigmergyEngine } from './kernel/swarm/stigmergy.js';
export { SwarmMind } from './kernel/swarm/swarm-mind.js';

// Neuro
export { SynapticNetwork } from './kernel/neuro/synapse.js';
export { AttentionMechanism } from './kernel/neuro/attention.js';
export { ReflexArc, proximityReflex } from './kernel/neuro/reflex.js';

// Emotion
export { EmotionalField } from './kernel/emotion/emotional-field.js';

// Morpho
export { MorphogenesisEngine, AgentRole } from './kernel/morpho/morphogenesis.js';

// Optim
export { VecPool } from './kernel/optim/vec-pool.js';
export { LazyScheduler } from './kernel/optim/lazy-engine.js';
export { BudgetTick } from './kernel/optim/budget-tick.js';

// Codec
export { TensionCodec } from './kernel/bridge/tension-codec.js';
