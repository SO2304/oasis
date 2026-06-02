/**
 * OASIS Kernel — Core Types
 *
 * These are the atomic primitives of the kernel.
 * Every subsystem imports from here, never the reverse.
 */

// ─── Agent Identity ─────────────────────────────────────────────

export type AgentId = string & { readonly __brand: 'AgentId' };
export type TenantId = string & { readonly __brand: 'TenantId' };
export type DriverId = string & { readonly __brand: 'DriverId' };

export function agentId(raw: string): AgentId {
  return raw as AgentId;
}
export function tenantId(raw: string): TenantId {
  return raw as TenantId;
}
export function driverId(raw: string): DriverId {
  return raw as DriverId;
}

// ─── Trust Levels (R9 — cyber + physical) ───────────────────────

export const CyberTrustLevel = {
  NATIVE: 'NATIVE',
  VERIFIED: 'VERIFIED',
  ENTERPRISE: 'ENTERPRISE',
  EXPERIMENTAL: 'EXPERIMENTAL',
} as const;
export type CyberTrustLevel = (typeof CyberTrustLevel)[keyof typeof CyberTrustLevel];

export const PhysicalTrustLevel = {
  SIM_ONLY: 'SIM_ONLY',
  SUPERVISED: 'SUPERVISED',
  AUTONOMOUS_INDOOR: 'AUTONOMOUS_INDOOR',
  AUTONOMOUS_OUTDOOR: 'AUTONOMOUS_OUTDOOR',
  SAFETY_CRITICAL: 'SAFETY_CRITICAL',
} as const;
export type PhysicalTrustLevel = (typeof PhysicalTrustLevel)[keyof typeof PhysicalTrustLevel];

export const TRUST_MULTIPLIERS: Record<CyberTrustLevel | PhysicalTrustLevel, number> = {
  NATIVE: 1.0,
  VERIFIED: 0.95,
  ENTERPRISE: 0.85,
  EXPERIMENTAL: 0.7,
  SIM_ONLY: 0.5,
  SUPERVISED: 0.7,
  AUTONOMOUS_INDOOR: 0.85,
  AUTONOMOUS_OUTDOOR: 0.95,
  SAFETY_CRITICAL: 1.0,
};

// ─── Scheduling Priority ────────────────────────────────────────

export const SchedulePriority = {
  /** Hard real-time — deadline miss = system failure */
  HARD_RT: 0,
  /** Soft real-time — deadline miss = degraded quality */
  SOFT_RT: 1,
  /** Best-effort — no deadline guarantee */
  BEST_EFFORT: 2,
} as const;
export type SchedulePriority = (typeof SchedulePriority)[keyof typeof SchedulePriority];

// ─── Agent Process ──────────────────────────────────────────────

export const AgentState = {
  CREATED: 'CREATED',
  INITIALIZING: 'INITIALIZING',
  READY: 'READY',
  RUNNING: 'RUNNING',
  PAUSED: 'PAUSED',
  STOPPING: 'STOPPING',
  STOPPED: 'STOPPED',
  FAILED: 'FAILED',
  KILLED: 'KILLED',
} as const;
export type AgentState = (typeof AgentState)[keyof typeof AgentState];

export interface AgentProcess {
  readonly id: AgentId;
  readonly tenantId: TenantId;
  readonly name: string;
  state: AgentState;
  priority: SchedulePriority;
  deadlineMs: number | null;
  trustCyber: CyberTrustLevel;
  trustPhysical: PhysicalTrustLevel | null;
  createdAt: number;
  lastTickAt: number;
}

// ─── Kernel Clock ───────────────────────────────────────────────

/** Monotonic nanosecond clock — never goes backward */
export function monotonicNow(): bigint {
  return process.hrtime.bigint();
}

/** Convert bigint nanoseconds to milliseconds */
export function nsToMs(ns: bigint): number {
  return Number(ns / 1_000_000n);
}

/** Convert milliseconds to bigint nanoseconds */
export function msToNs(ms: number): bigint {
  return BigInt(Math.round(ms * 1_000_000));
}

// ─── IPC Message ────────────────────────────────────────────────

export const MessageType = {
  COMMAND: 'COMMAND',
  EVENT: 'EVENT',
  QUERY: 'QUERY',
  RESPONSE: 'RESPONSE',
  PANIC: 'PANIC',
} as const;
export type MessageType = (typeof MessageType)[keyof typeof MessageType];

export interface KernelMessage<T = unknown> {
  readonly id: string;
  readonly type: MessageType;
  readonly topic: string;
  readonly source: AgentId | 'KERNEL';
  readonly target: AgentId | 'BROADCAST';
  readonly payload: T;
  readonly timestamp: bigint;
  readonly tenantId: TenantId;
}

// ─── HAL Types ──────────────────────────────────────────────────

export const DriverState = {
  UNLOADED: 'UNLOADED',
  LOADING: 'LOADING',
  READY: 'READY',
  ACTIVE: 'ACTIVE',
  ERROR: 'ERROR',
  UNLOADING: 'UNLOADING',
} as const;
export type DriverState = (typeof DriverState)[keyof typeof DriverState];

export interface PhysicalConstraints {
  /** Maximum force in Newtons */
  maxForceN: number;
  /** Maximum torque in Newton-meters */
  maxTorqueNm: number;
  /** Maximum velocity in m/s */
  maxVelocityMs: number;
  /** Geofence boundaries [minX, minY, minZ, maxX, maxY, maxZ] in meters */
  geofenceBounds: readonly [number, number, number, number, number, number];
}

export interface DriverCommand {
  readonly type: string;
  readonly params: Record<string, number>;
  readonly timestamp: bigint;
}

export interface DriverTelemetry {
  readonly driverId: DriverId;
  readonly timestamp: bigint;
  readonly values: Record<string, number>;
  readonly healthy: boolean;
}

// ─── Kill Switch ────────────────────────────────────────────────

export const PanicReason = {
  MANUAL: 'MANUAL',
  FORCE_EXCEEDED: 'FORCE_EXCEEDED',
  GEOFENCE_BREACH: 'GEOFENCE_BREACH',
  DEADLINE_MISS: 'DEADLINE_MISS',
  TRUST_VIOLATION: 'TRUST_VIOLATION',
  HARDWARE_FAULT: 'HARDWARE_FAULT',
  WATCHDOG_TIMEOUT: 'WATCHDOG_TIMEOUT',
} as const;
export type PanicReason = (typeof PanicReason)[keyof typeof PanicReason];

export interface PanicEvent {
  readonly reason: PanicReason;
  readonly source: AgentId | DriverId | 'KERNEL';
  readonly timestamp: bigint;
  readonly detail: string;
}
