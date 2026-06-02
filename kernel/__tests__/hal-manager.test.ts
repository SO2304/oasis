/**
 * OASIS Kernel — HAL Manager Tests
 *
 * Tests R9 (force capping), geofencing, hot-swap, and kill switch integration.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { HALManager } from '../hal/hal-manager.js';
import { VirtualActuator, type VirtualActuatorConfig } from '../hal/drivers/virtual-actuator.js';
import { __resetKillSwitchForTesting } from '../kill-switch.js';
import { tenantId, driverId } from '../types.js';

const TEST_TENANT = tenantId('test-tenant');

function createActuator(name = 'test-motor'): VirtualActuator {
  const config: VirtualActuatorConfig = {
    name,
    constraints: {
      maxForceN: 100,
      maxTorqueNm: 50,
      maxVelocityMs: 10,
      geofenceBounds: [-5, -5, -1, 5, 5, 10], // 10m x 10m x 11m box
    },
    massKg: 2.0,
    friction: 0.1,
  };
  return new VirtualActuator(name, config);
}

describe('HALManager', () => {
  let ks: ReturnType<typeof __resetKillSwitchForTesting>;
  let hal: HALManager;
  let actuator: VirtualActuator;

  beforeEach(() => {
    ks = __resetKillSwitchForTesting();
    hal = new HALManager(TEST_TENANT, ks);
    actuator = createActuator();
  });

  it('should load and activate a driver', async () => {
    await hal.loadDriver(actuator);
    expect(hal.getDriverState(actuator.id)).toBe('READY');

    hal.activate(actuator.id);
    expect(hal.getDriverState(actuator.id)).toBe('ACTIVE');
  });

  it('should enforce R9 force capping on commands', async () => {
    await hal.loadDriver(actuator);
    hal.activate(actuator.id);

    // Request 200N but max is 100N
    const result = hal.sendCommand(actuator.id, 'MOVE', { force: 200 });

    expect(result.accepted).toBe(true);
    expect(result.clampedFields).toContain('force');
    expect(result.actualValues['force']).toBe(100); // Clamped to max
  });

  it('should enforce torque capping', async () => {
    await hal.loadDriver(actuator);
    hal.activate(actuator.id);

    const result = hal.sendCommand(actuator.id, 'ROTATE', { torque: 999 });

    expect(result.accepted).toBe(true);
    expect(result.clampedFields).toContain('torque');
    expect(result.actualValues['torque']).toBe(50); // Clamped to max
  });

  it('should trigger kill switch on geofence breach', async () => {
    await hal.loadDriver(actuator);
    hal.activate(actuator.id);

    // Send position outside geofence (max X is 5)
    hal.sendCommand(actuator.id, 'MOVE', { force: 10, x: 100 });

    expect(ks.isTriggered()).toBe(true);
    expect(ks.getPanicEvent()?.reason).toBe('GEOFENCE_BREACH');
  });

  it('should emergency stop all drivers on kill switch', async () => {
    await hal.loadDriver(actuator);
    hal.activate(actuator.id);

    // Give it some velocity
    hal.sendCommand(actuator.id, 'MOVE', { force: 50 });
    expect(actuator.getMotorState().force).toBeGreaterThan(0);

    // Trigger panic
    ks.panic('MANUAL', 'KERNEL', 'Test panic');

    // Motor should be zeroed
    expect(actuator.isEmergencyStopped()).toBe(true);
    expect(actuator.getMotorState().force).toBe(0);
    expect(actuator.getMotorState().velocity.x).toBe(0);
  });

  it('should read telemetry from a driver', async () => {
    await hal.loadDriver(actuator);
    hal.activate(actuator.id);

    hal.sendCommand(actuator.id, 'MOVE', { force: 30 });
    const telemetry = hal.readTelemetry(actuator.id);

    expect(telemetry.driverId).toBe(actuator.id);
    expect(telemetry.healthy).toBe(true);
    expect(telemetry.values['force']).toBe(30);
    expect(telemetry.values['commandCount']).toBe(1);
  });

  it('should unload a driver gracefully', async () => {
    await hal.loadDriver(actuator);
    await hal.unloadDriver(actuator.id);

    expect(() => hal.getDriverState(actuator.id)).toThrow('not found');
  });

  it('should reject commands on non-ACTIVE drivers', async () => {
    await hal.loadDriver(actuator);
    // Don't activate

    expect(() => hal.sendCommand(actuator.id, 'MOVE', { force: 10 })).toThrow('not ACTIVE');
  });

  it('should reject commands after kill switch', async () => {
    await hal.loadDriver(actuator);
    hal.activate(actuator.id);

    ks.panic('MANUAL', 'KERNEL', 'Test');

    expect(() => hal.sendCommand(actuator.id, 'MOVE', { force: 10 })).toThrow('Kill switch');
  });
});
