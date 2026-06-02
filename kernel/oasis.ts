/**
 * OASIS Kernel — Unified Nervous System (Facade)
 *
 * Single entry point. Wires all subsystems.
 * Delegates tick execution to tick-pipeline.ts (R10 compliance).
 *
 *   const kernel = new OasisKernel({ dim: 128, tenantId: 'factory-01' });
 *   kernel.addAgent('navigator', 'RUNNING', 'HARD_RT');
 *   kernel.tick();  // 14-phase heartbeat
 */

import type { AgentId, TenantId, DriverId } from './types.js';
import { agentId, tenantId, SchedulePriority, AgentState } from './types.js';
import type { Vec } from './physics/vector-math.js';
import { normalize } from './physics/vector-math.js';
import { KillSwitch } from './kill-switch.js';
import { LatentEngine } from './physics/latent-engine.js';
import { TensionField } from './physics/tension-field.js';
import { WorldModel } from './physics/world-model.js';
import type { HyperState } from './physics/hyper-state.js';
import { isActionSafe } from './physics/hyper-state.js';
import { ReflectionEngine } from './physics/reflection-engine.js';
import { TensionCodec, type TwistMsg } from './bridge/tension-codec.js';
import { SensorFusion } from './perception/sensor-fusion.js';
import { Proprioception } from './perception/proprioception.js';
import { PerceptionBridge } from './perception/perception-bridge.js';
import { Watchdog } from './sovereign/watchdog.js';
import { TrustChain } from './sovereign/trust-chain.js';
import { AuditLog } from './sovereign/audit-log.js';
import { SwarmMind } from './swarm/swarm-mind.js';
import { StigmergyEngine } from './swarm/stigmergy.js';
import { ImmuneSystem } from './swarm/immune.js';
import { HALManager } from './hal/hal-manager.js';
import { OasisMonitor } from './shell/oasis-monitor.js';
import { LazyScheduler } from './optim/lazy-engine.js';
import { BudgetTick } from './optim/budget-tick.js';
import { SynapticNetwork } from './neuro/synapse.js';
import { AttentionMechanism } from './neuro/attention.js';
import { ReflexArc, type ReflexRule } from './neuro/reflex.js';
import { DreamEngine } from './neuro/dreams.js';
import { EmotionalField, type EmotionalState } from './emotion/emotional-field.js';
import { MorphogenesisEngine } from './morpho/morphogenesis.js';
import { TemporalBranchingEngine } from './temporal/branching.js';
import type { SensorConfig, SensorReading } from './perception/sensor-types.js';
import type { IHALDriver } from './hal/hal-types.js';
import { executeTick, type KernelTickResult } from './tick-pipeline.js';
import { DormancyManager } from './sovereign/dormancy.js';

// ─── Configuration ──────────────────────────────────────────────

export interface OasisConfig {
  readonly dim?: number;
  readonly tenantId: string;
  readonly tickBudgetMs?: number;
  readonly maxTensions?: number;
  readonly maxPheromones?: number;
  readonly enableSwarm?: boolean;
  readonly enableImmune?: boolean;
  readonly enableEmotions?: boolean;
  readonly enableMorpho?: boolean;
  readonly enableBranching?: boolean;
  readonly enableDreams?: boolean;
  readonly monitorInterval?: number;
  readonly branchCount?: number;
}

// Re-export for consumers
export type { KernelTickResult } from './tick-pipeline.js';

// ─── The Kernel ─────────────────────────────────────────────────

export class OasisKernel {
  readonly killSwitch: KillSwitch;
  readonly config: Required<OasisConfig>;
  private tickCount = 0;

  // All subsystems — public for pipeline access
  readonly engine: LatentEngine;
  readonly field: TensionField;
  readonly world: WorldModel;
  readonly codec: TensionCodec;
  readonly hal: HALManager;
  readonly monitor: OasisMonitor;
  readonly fusion: SensorFusion;
  readonly proprio: Proprioception;
  readonly bridge: PerceptionBridge;
  readonly reflection: ReflectionEngine;
  readonly stigmergy: StigmergyEngine;
  readonly swarm: SwarmMind;
  readonly immune: ImmuneSystem;
  readonly lazy: LazyScheduler;
  readonly budget: BudgetTick;
  readonly synapses: SynapticNetwork;
  readonly attention: AttentionMechanism;
  readonly reflexes: ReflexArc;
  readonly dreams: DreamEngine;
  readonly emotions: EmotionalField;
  readonly morpho: MorphogenesisEngine;
  readonly branching: TemporalBranchingEngine;

  // ─── Sovereign ────────────────────────────────────
  readonly watchdog: Watchdog;
  readonly trustChain: TrustChain;
  readonly auditLog: AuditLog;
  readonly dormancy: DormancyManager;

  // Internal state (accessible to pipeline via public)
  readonly driverAgentMap = new Map<DriverId, AgentId>();
  readonly goalPositions = new Map<string, Vec>();
  readonly sentinelId: AgentId;
  private readonly agentPriorities = new Map<AgentId, SchedulePriority>();

  constructor(config: OasisConfig) {
    const dim = config.dim ?? 128;
    const tenant = tenantId(config.tenantId);

    this.config = {
      dim,
      tenantId: config.tenantId,
      tickBudgetMs: config.tickBudgetMs ?? 1.0,
      maxTensions: config.maxTensions ?? 10_000,
      maxPheromones: config.maxPheromones ?? 5_000,
      enableSwarm: config.enableSwarm ?? true,
      enableImmune: config.enableImmune ?? true,
      enableEmotions: config.enableEmotions ?? true,
      enableMorpho: config.enableMorpho ?? true,
      enableBranching: config.enableBranching ?? true,
      enableDreams: config.enableDreams ?? true,
      monitorInterval: config.monitorInterval ?? 100,
      branchCount: config.branchCount ?? 5,
    };

    this.killSwitch = new KillSwitch();
    this.field = new TensionField(this.config.maxTensions, dim);
    this.world = new WorldModel(dim);
    this.engine = new LatentEngine(tenant, dim, this.killSwitch);
    this.codec = new TensionCodec(dim);
    this.hal = new HALManager(tenant, this.killSwitch);
    this.monitor = new OasisMonitor();
    this.fusion = new SensorFusion(dim);
    this.proprio = new Proprioception(dim);
    this.bridge = new PerceptionBridge(agentId('__perception__'), tenant, dim, this.field, this.world);
    this.reflection = new ReflectionEngine(dim, this.field, tenant, agentId('__reflection__'), this.killSwitch);
    this.stigmergy = new StigmergyEngine(dim, this.config.maxPheromones);
    this.swarm = new SwarmMind(dim, tenant, this.stigmergy);
    this.immune = new ImmuneSystem(dim, this.stigmergy);
    this.lazy = new LazyScheduler();
    this.budget = new BudgetTick(this.config.tickBudgetMs);
    this.synapses = new SynapticNetwork(dim);
    this.attention = new AttentionMechanism(dim);
    this.reflexes = new ReflexArc();
    this.dreams = new DreamEngine();
    this.emotions = new EmotionalField(dim);
    this.morpho = new MorphogenesisEngine(dim);
    this.branching = new TemporalBranchingEngine(dim, this.config.branchCount, 8);

    // Sovereign subsystems
    this.auditLog = new AuditLog();
    this.trustChain = new TrustChain(this.auditLog);
    this.dormancy = new DormancyManager();
    this.watchdog = new Watchdog(
      { deadlineMs: Math.max(100, this.config.tickBudgetMs * 50), missThreshold: 3 },
      (reason) => this.killSwitch.panic('WATCHDOG_TIMEOUT', 'KERNEL', reason),
    );

    this.sentinelId = agentId('__sentinel__');
    this.engine.registerAgent(this.sentinelId, AgentState.RUNNING);
    this.lazy.register(this.sentinelId, SchedulePriority.SOFT_RT);
    this.emotions.register(this.sentinelId);
  }

  // ─── Agent Management ───────────────────────────────────

  addAgent(
    name: string,
    initialState: AgentState = AgentState.CREATED,
    priority: 'HARD_RT' | 'SOFT_RT' | 'BEST_EFFORT' = 'BEST_EFFORT',
  ): AgentId {
    this.killSwitch.assertAlive();
    const id = agentId(name);
    const prio = SchedulePriority[priority];
    this.engine.registerAgent(id, initialState);
    this.lazy.register(id, prio);
    this.agentPriorities.set(id, prio);
    this.emotions.register(id);
    this.morpho.register(id);
    return id;
  }

  removeAgent(id: AgentId): void { this.agentPriorities.delete(id); }

  getAgentState(id: AgentId): HyperState | undefined { return this.engine.getState(id); }

  // ─── Hardware & Sensors ─────────────────────────────────

  async addDriver(driver: IHALDriver, ownerAgent?: AgentId): Promise<DriverId> {
    await this.hal.loadDriver(driver);
    this.hal.activate(driver.id);
    this.proprio.register(driver.id);
    if (ownerAgent) this.driverAgentMap.set(driver.id, ownerAgent);
    return driver.id;
  }

  addSensor(config: SensorConfig): void {
    this.fusion.registerSensor(config);
    this.bridge.registerSensor(config);
  }

  addReflex(rule: ReflexRule): void { this.reflexes.register(rule); }

  ingestSensorReading(reading: SensorReading): void { this.bridge.ingestReading(reading); }

  // ─── Intentions ─────────────────────────────────────────

  setGoal(id: string, position: Vec, intensity = 3.0): void {
    this.world.addGoal(id, position, intensity, normalize(position));
    this.goalPositions.set(id, Float64Array.from(position));
    this.attention.setGoals([...this.goalPositions.values()]);
  }

  addObstacle(id: string, position: Vec, intensity = 5.0): void {
    this.world.addObstacle(id, position, intensity, normalize(position));
  }

  emitTension(sourceId: AgentId, force: Vec, intensity = 1.0): void {
    this.engine.emitTension(sourceId, force, normalize(force), intensity, 3, 15);
  }

  getMotorCommand(id: AgentId): TwistMsg | null {
    const state = this.engine.getState(id);
    if (!state || !isActionSafe(state)) return null;
    return this.codec.decodeTwist(state.position);
  }

  // ─── THE HEARTBEAT ─────────────────────────────────────

  tick(): KernelTickResult {
    this.killSwitch.assertAlive();
    this.tickCount++;
    const result = executeTick(this, this.tickCount);
    this.watchdog.feed(); // Watchdog: proof of life
    this.auditLog.append('KERNEL', 'TICK', { n: this.tickCount });
    return result;
  }

  // ─── Accessors ──────────────────────────────────────────

  getTickCount(): number { return this.tickCount; }
  isAlive(): boolean { return !this.killSwitch.isTriggered(); }
  getAgentCount(): number { return this.engine.getAgentIds().length; }
  getSwarmCount(): number { return this.swarm.getSwarmCount(); }
  getQuarantinedCount(): number { return this.immune.getQuarantined().length; }
  getAgentRole(id: AgentId): string { return this.morpho.getRole(id); }
  getAgentEmotion(id: AgentId): EmotionalState | undefined { return this.emotions.getState(id); }
  getSynapseCount(): number { return this.synapses.getSynapseCount(); }
  getDreamCount(): number { return this.dreams.getDreamCount(); }

  panic(reason: string): void { this.killSwitch.panic('MANUAL', 'KERNEL', reason); }
}
