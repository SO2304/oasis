/**
 * HAL subsystem — dedicated unit tests.
 */
import { describe, it, expect } from 'vitest';
import { GPIODriver, type GPIODriverConfig } from '../drivers/gpio-driver.js';
import { VirtualActuator } from '../drivers/virtual-actuator.js';
import { HALManager } from '../hal-manager.js';
import { __resetKillSwitchForTesting } from '../../kill-switch.js';
import { tenantId } from '../../types.js';

describe('GPIODriver', () => {
  it('should init in simulated mode and accept commands', async () => {
    const config: GPIODriverConfig = {
      name: 'test-gpio',
      pins: [
        { pin: 18, mode: 'PWM', label: 'motor', pwmFrequency: 1000 },
        { pin: 23, mode: 'OUTPUT', label: 'dir' },
      ],
      constraints: { maxForceN: 10, maxTorqueNm: 5, maxVelocityMs: 2, geofenceBounds: [-5, -5, -1, 5, 5, 5] },
    };
    const gpio = new GPIODriver(config);
    expect(gpio.isSimulated).toBe(true);

    await gpio.init();
    expect(gpio.healthcheck().healthy).toBe(true);

    const r1 = gpio.write({ type: 'GPIO_SET', params: { pin: 23, value: 1 }, timestamp: 0n });
    expect(r1.accepted).toBe(true);

    const r2 = gpio.write({ type: 'PWM_SET', params: { pin: 18, duty: 75 }, timestamp: 0n });
    expect(r2.accepted).toBe(true);
    expect(r2.actualValues['duty']).toBe(75);

    const r3 = gpio.write({ type: 'PWM_SET', params: { pin: 18, duty: 150 }, timestamp: 0n });
    expect(r3.clampedFields).toContain('duty');

    gpio.emergencyStop();
    expect(gpio.healthcheck().healthy).toBe(false);

    expect(gpio.getWriteLog().length).toBeGreaterThan(0);
  });
});

describe('VirtualActuator — Jam Physics', () => {
  it('should simulate current rise and temperature during jam', async () => {
    const ks = __resetKillSwitchForTesting();
    const hal = new HALManager(tenantId('t'), ks);
    const act = new VirtualActuator('jam-test', {
      name: 'jam-test',
      constraints: { maxForceN: 100, maxTorqueNm: 50, maxVelocityMs: 10, geofenceBounds: [-10, -10, -1, 10, 10, 10] },
      massKg: 2, friction: 0.1,
    });
    await hal.loadDriver(act);
    hal.activate(act.id);

    act.simulateJam();
    for (let i = 0; i < 5; i++) hal.sendCommand(act.id, 'MOVE', { force: 80 });

    const state = act.getMotorState();
    expect(state.velocity.x).toBe(0);
    expect(state.temperature).toBeGreaterThan(25);
    expect(state.stress).toBeGreaterThan(0);
    expect(state.currentDraw).toBeGreaterThan(0);

    act.clearJam();
    expect(act.isJammed()).toBe(false);
  });
});
