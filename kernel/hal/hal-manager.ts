/**
 * OASIS Kernel — HAL Manager
 *
 * Manages the lifecycle of hardware drivers with hot-swap capability.
 *
 * INNOVATION: State-machine driven lifecycle with automatic fallback.
 * When a driver enters ERROR state, the HAL Manager:
 * 1. Fires emergency stop on the failed driver
 * 2. Attempts hot-swap to a fallback driver (if registered)
 * 3. If no fallback, triggers kernel panic via kill switch
 *
 * State machine: UNLOADED -> LOADING -> READY -> ACTIVE -> (ERROR | UNLOADING -> UNLOADED)
 */

import {
  type DriverId,
  type TenantId,
  DriverState,
  driverId,
  monotonicNow,
  nsToMs,
} from '../types.js';
import type { IHALDriver, DriverWriteResult } from './hal-types.js';
import { HALEvent } from './hal-types.js';
import { type KillSwitch, getKillSwitch } from '../kill-switch.js';
import { MessageBus } from '../ipc/message-bus.js';

interface ManagedDriver {
  driver: IHALDriver;
  state: DriverState;
  fallbackId: DriverId | null;
  loadedAt: bigint;
  errorCount: number;
}

export class HALManager {
  private readonly drivers = new Map<DriverId, ManagedDriver>();
  private readonly tenantId: TenantId;

  constructor(
    tenantId: TenantId,
    private readonly killSwitch: KillSwitch = getKillSwitch(),
    private readonly bus: MessageBus = new MessageBus(killSwitch),
  ) {
    this.tenantId = tenantId;

    // Kill switch -> emergency stop ALL drivers
    this.killSwitch.onPanic(() => {
      this.emergencyStopAll();
    });
  }

  /**
   * Load a driver into the HAL.
   * Optionally specify a fallback driver ID for hot-swap on failure.
   */
  async loadDriver(driver: IHALDriver, fallbackId?: DriverId): Promise<void> {
    this.killSwitch.assertAlive();

    if (this.drivers.has(driver.id)) {
      throw new Error(`Driver ${driver.id} already loaded`);
    }

    const managed: ManagedDriver = {
      driver,
      state: DriverState.LOADING,
      fallbackId: fallbackId ?? null,
      loadedAt: monotonicNow(),
      errorCount: 0,
    };

    this.drivers.set(driver.id, managed);
    this.emitEvent(HALEvent.DRIVER_LOADED, driver.id);

    try {
      await driver.init();
      managed.state = DriverState.READY;
      this.emitEvent(HALEvent.DRIVER_READY, driver.id);
    } catch (err) {
      managed.state = DriverState.ERROR;
      managed.errorCount++;
      this.emitEvent(HALEvent.DRIVER_ERROR, driver.id, String(err));
      throw err;
    }
  }

  /** Activate a driver — it starts accepting commands */
  activate(id: DriverId): void {
    this.killSwitch.assertAlive();
    const managed = this.getManaged(id);

    if (managed.state !== DriverState.READY) {
      throw new Error(`Cannot activate driver ${id}: state is ${managed.state}, expected READY`);
    }

    managed.state = DriverState.ACTIVE;
    this.emitEvent(HALEvent.DRIVER_ACTIVE, id);
  }

  /**
   * Send a command to a driver with R9 force capping.
   *
   * INNOVATION: The HAL Manager enforces physical constraints
   * BEFORE the command reaches the driver. The driver never sees
   * values that exceed its safety limits.
   */
  sendCommand(
    id: DriverId,
    type: string,
    params: Record<string, number>,
  ): DriverWriteResult {
    this.killSwitch.assertAlive();
    const managed = this.getManaged(id);

    if (managed.state !== DriverState.ACTIVE) {
      throw new Error(`Driver ${id} is not ACTIVE (state: ${managed.state})`);
    }

    // R9: Enforce physical constraints at HAL level
    const constraints = managed.driver.getConstraints();
    const clampedFields: string[] = [];
    const safeParams = { ...params };

    if (constraints) {
      // Force capping
      if (safeParams['force'] !== undefined && Math.abs(safeParams['force']!) > constraints.maxForceN) {
        safeParams['force'] = Math.sign(safeParams['force']!) * constraints.maxForceN;
        clampedFields.push('force');
      }

      // Torque capping
      if (safeParams['torque'] !== undefined && Math.abs(safeParams['torque']!) > constraints.maxTorqueNm) {
        safeParams['torque'] = Math.sign(safeParams['torque']!) * constraints.maxTorqueNm;
        clampedFields.push('torque');
      }

      // Velocity capping
      if (safeParams['velocity'] !== undefined && Math.abs(safeParams['velocity']!) > constraints.maxVelocityMs) {
        safeParams['velocity'] = Math.sign(safeParams['velocity']!) * constraints.maxVelocityMs;
        clampedFields.push('velocity');
      }

      // Geofence check
      if (safeParams['x'] !== undefined || safeParams['y'] !== undefined || safeParams['z'] !== undefined) {
        const [minX, minY, minZ, maxX, maxY, maxZ] = constraints.geofenceBounds;
        if (safeParams['x'] !== undefined) {
          if (safeParams['x']! < minX || safeParams['x']! > maxX) {
            this.killSwitch.panic('GEOFENCE_BREACH', id, `X=${safeParams['x']} outside [${minX},${maxX}]`);
          }
        }
        if (safeParams['y'] !== undefined) {
          if (safeParams['y']! < minY || safeParams['y']! > maxY) {
            this.killSwitch.panic('GEOFENCE_BREACH', id, `Y=${safeParams['y']} outside [${minY},${maxY}]`);
          }
        }
        if (safeParams['z'] !== undefined) {
          if (safeParams['z']! < minZ || safeParams['z']! > maxZ) {
            this.killSwitch.panic('GEOFENCE_BREACH', id, `Z=${safeParams['z']} outside [${minZ},${maxZ}]`);
          }
        }
      }

      if (clampedFields.length > 0) {
        this.emitEvent(HALEvent.COMMAND_CLAMPED, id, `Clamped: ${clampedFields.join(', ')}`);
      }
    }

    // Send the safe command to the driver
    const command = { type, params: safeParams, timestamp: monotonicNow() };

    try {
      const result = managed.driver.write(command);
      // Merge HAL-level clamping info into driver result
      return {
        ...result,
        clampedFields: [...clampedFields, ...result.clampedFields],
      };
    } catch (err) {
      managed.state = DriverState.ERROR;
      managed.errorCount++;
      this.emitEvent(HALEvent.DRIVER_ERROR, id, String(err));
      this.attemptHotSwap(id, managed);
      throw err;
    }
  }

  /** Read telemetry from a driver */
  readTelemetry(id: DriverId): ReturnType<IHALDriver['read']> {
    const managed = this.getManaged(id);
    return managed.driver.read();
  }

  /** Unload a driver gracefully */
  async unloadDriver(id: DriverId): Promise<void> {
    const managed = this.getManaged(id);
    managed.state = DriverState.UNLOADING;

    try {
      await managed.driver.shutdown();
    } finally {
      this.drivers.delete(id);
      this.emitEvent(HALEvent.DRIVER_UNLOADED, id);
    }
  }

  /** Emergency stop ALL drivers — synchronous, called on panic */
  emergencyStopAll(): void {
    for (const [id, managed] of this.drivers) {
      try {
        managed.driver.emergencyStop();
        managed.state = DriverState.UNLOADED;
      } catch {
        // Must not throw during emergency
      }
      this.emitEvent(HALEvent.EMERGENCY_STOP, id);
    }
  }

  /** Get all loaded driver IDs */
  getDriverIds(): DriverId[] {
    return [...this.drivers.keys()];
  }

  /** Get driver state */
  getDriverState(id: DriverId): DriverState {
    return this.getManaged(id).state;
  }

  /** Get the IPC bus for external subscription */
  getBus(): MessageBus {
    return this.bus;
  }

  // ─── Private ────────────────────────────────────────────────

  private getManaged(id: DriverId): ManagedDriver {
    const managed = this.drivers.get(id);
    if (!managed) throw new Error(`Driver ${id} not found`);
    return managed;
  }

  /**
   * INNOVATION: Hot-swap on failure.
   * If a fallback driver is registered, swap transparently.
   */
  private attemptHotSwap(failedId: DriverId, managed: ManagedDriver): void {
    if (!managed.fallbackId) return;

    const fallback = this.drivers.get(managed.fallbackId);
    if (!fallback || fallback.state !== DriverState.READY) return;

    // Promote fallback to active
    fallback.state = DriverState.ACTIVE;
    this.emitEvent(HALEvent.DRIVER_HOTSWAP, managed.fallbackId, `Replacing failed ${failedId}`);
  }

  private emitEvent(event: HALEvent, driverId: DriverId, detail?: string): void {
    this.bus.publish(
      this.bus.createMessage(
        'EVENT',
        event,
        'KERNEL',
        'BROADCAST',
        { driverId, detail: detail ?? null },
        this.tenantId,
      ),
    );
  }
}
