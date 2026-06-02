/**
 * OASIS Kernel — Public API
 */

// Core types
export * from './types.js';

// Kill Switch (R12)
export { KillSwitch, KillSwitchEngagedException, getKillSwitch } from './kill-switch.js';

// IPC
export { MessageBus } from './ipc/message-bus.js';

// HAL
export type { IHALDriver, DriverWriteResult, DriverHealthStatus } from './hal/hal-types.js';
export { HALEvent } from './hal/hal-types.js';
export { HALManager } from './hal/hal-manager.js';
export { VirtualActuator } from './hal/drivers/virtual-actuator.js';

// Scheduler
export { RTScheduler } from './scheduler/rt-scheduler.js';
export type { ScheduledTask, SchedulerTickResult } from './scheduler/rt-scheduler.js';

// Trust
export { TrustEngine } from './trust/trust-engine.js';

// Physics — Tensorial Brain
export * from './physics/vector-math.js';
export {
  type HyperState,
  createHyperState,
  evolveState,
  calculateEntropy,
  collapseState,
  isActionSafe,
  steerToward,
  transitionProbability,
  R14_ENTROPY_CRITICAL,
  DEFAULT_DIM,
} from './physics/hyper-state.js';
export { TensionField } from './physics/tension-field.js';
export type { TensionVector, InterferenceResult, ResonanceEvent } from './physics/tension-field.js';
export { LatentEngine } from './physics/latent-engine.js';
export type { LatentCorrelation, EngineTickResult } from './physics/latent-engine.js';
export { WorldModel, ZoneType } from './physics/world-model.js';
export type { PressureZone, WorldSample, LeastResistancePath } from './physics/world-model.js';

// Perception
export { SensorFusion } from './perception/sensor-fusion.js';
export { PerceptionBridge } from './perception/perception-bridge.js';
export type * from './perception/sensor-types.js';
export { Proprioception } from './perception/proprioception.js';
export { ReflectionEngine, DeviationSeverity } from './physics/reflection-engine.js';
export type { EfferenceCopy, DeviationVector, PainSignal, ReflectionTickResult } from './physics/deviation.js';
export type { LeastResistancePath } from './physics/pathfinding.js';
export type { LatentCorrelation } from './physics/correlation.js';
export { computeFlockingForces } from './swarm/flocking.js';

// Optimization
export { VecPool, addInto, subInto, scaleInto, accumScaled, dotFast, normSq } from './optim/vec-pool.js';
export { RingBuffer } from './optim/ring-buffer.js';
export { BudgetTick } from './optim/budget-tick.js';
export type { WorkItem, BudgetResult, JitterStats } from './optim/budget-tick.js';
export { LazyScheduler } from './optim/lazy-engine.js';

// Swarm
export { StigmergyEngine, PheromoneType } from './swarm/stigmergy.js';
export { SwarmMind, SwarmEvent } from './swarm/swarm-mind.js';
export { ImmuneSystem } from './swarm/immune.js';

// Bridge & Deployment
export { TensionCodec } from './bridge/tension-codec.js';
export { SovereignNode } from './auth/sovereign-node.js';
export { OasisMonitor } from './shell/oasis-monitor.js';
export { CheckpointManager } from './persistence/checkpoint.js';
export { GPIODriver } from './hal/drivers/gpio-driver.js';
export { estimateMutualInformation, detectNonLinearCorrelations } from './physics/mutual-info.js';
export { TemporalPredictor } from './physics/prediction.js';
export { OasisKernel, type OasisConfig } from './oasis.js';
export type { KernelTickResult } from './tick-pipeline.js';

// Innovation
export { TemporalBranchingEngine } from './temporal/branching.js';
export { EmotionalField, EmotionType } from './emotion/emotional-field.js';
export { MorphogenesisEngine, AgentRole } from './morpho/morphogenesis.js';

// Neuroscience
export { SynapticNetwork } from './neuro/synapse.js';
export { AttentionMechanism } from './neuro/attention.js';
export { ReflexArc, proximityReflex, temperatureReflex, forceOverloadReflex, stressPanicReflex } from './neuro/reflex.js';
export { DreamEngine } from './neuro/dreams.js';
