/**
 * OASIS Kernel — HAL Type Definitions
 *
 * Every hardware driver implements IHALDriver.
 * The HAL Manager handles lifecycle, hot-swap, and health monitoring.
 */

import type { DriverId, DriverState, DriverCommand, DriverTelemetry, PhysicalConstraints } from '../types.js';

// ─── Driver Interface ───────────────────────────────────────────

export interface IHALDriver {
  readonly id: DriverId;
  readonly name: string;
  readonly version: string;
  readonly isSimulated: boolean;

  /** Initialize the driver — load firmware, calibrate, etc. */
  init(): Promise<void>;

  /** Read current telemetry from the hardware */
  read(): DriverTelemetry;

  /** Send a command to the hardware */
  write(command: DriverCommand): DriverWriteResult;

  /** Graceful shutdown */
  shutdown(): Promise<void>;

  /** Health check — must return within 5ms */
  healthcheck(): DriverHealthStatus;

  /** Emergency stop — synchronous, must complete in < 1ms */
  emergencyStop(): void;

  /** Physical constraints for this driver (null if non-physical) */
  getConstraints(): PhysicalConstraints | null;
}

// ─── Results ────────────────────────────────────────────────────

export interface DriverWriteResult {
  readonly accepted: boolean;
  readonly actualValues: Record<string, number>;
  readonly clampedFields: string[];
  readonly timestamp: bigint;
}

export interface DriverHealthStatus {
  readonly healthy: boolean;
  readonly state: DriverState;
  readonly uptimeMs: number;
  readonly errorCount: number;
  readonly lastError: string | null;
}

// ─── HAL Manager Events ────────────────────────────────────────

export const HALEvent = {
  DRIVER_LOADED: 'hal.driver.loaded',
  DRIVER_READY: 'hal.driver.ready',
  DRIVER_ACTIVE: 'hal.driver.active',
  DRIVER_ERROR: 'hal.driver.error',
  DRIVER_UNLOADED: 'hal.driver.unloaded',
  DRIVER_HOTSWAP: 'hal.driver.hotswap',
  COMMAND_CLAMPED: 'hal.command.clamped',
  CONSTRAINT_VIOLATED: 'hal.constraint.violated',
  EMERGENCY_STOP: 'hal.emergency.stop',
} as const;
export type HALEvent = (typeof HALEvent)[keyof typeof HALEvent];
