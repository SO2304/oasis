/**
 * OASIS Kernel — Virtual Actuator Driver
 *
 * A simulated motor/actuator for testing the HAL pipeline.
 * Implements full R9 force capping and geofencing at driver level
 * (defense in depth — HAL Manager also enforces constraints).
 *
 * INNOVATION: Physics simulation with inertia and friction.
 * The virtual actuator doesn't just set values — it simulates
 * realistic motor behavior with acceleration curves, making
 * the simulation bridge (P3) meaningful for agent development.
 */

import {
  type DriverId,
  type DriverCommand,
  type DriverTelemetry,
  type PhysicalConstraints,
  DriverState,
  driverId,
  monotonicNow,
  nsToMs,
} from '../../types.js';
import type { IHALDriver, DriverWriteResult, DriverHealthStatus } from '../hal-types.js';

export interface VirtualActuatorConfig {
  readonly name: string;
  readonly constraints: PhysicalConstraints;
  /** Simulated mass in kg (affects acceleration) */
  readonly massKg: number;
  /** Friction coefficient (0-1) */
  readonly friction: number;
}

interface MotorState {
  position: { x: number; y: number; z: number };
  velocity: { x: number; y: number; z: number };
  force: number;
  torque: number;
  stopped: boolean;
  /** Current drawn by motor (amps) — rises under load */
  currentDraw: number;
  /** Motor temperature (celsius) — rises with sustained current */
  temperature: number;
  /** Mechanical stress indicator [0, 1] */
  stress: number;
}

export class VirtualActuator implements IHALDriver {
  readonly id: DriverId;
  readonly name: string;
  readonly version = '1.0.0';
  readonly isSimulated = true;

  private state: DriverState = DriverState.UNLOADED;
  private readonly config: VirtualActuatorConfig;
  private initTime: bigint = 0n;
  private errorCount = 0;
  private lastError: string | null = null;
  private commandCount = 0;
  private emergencyStopped = false;
  /** Simulate mechanical jam — friction becomes infinite */
  private jammed = false;

  private motor: MotorState = {
    position: { x: 0, y: 0, z: 0 },
    velocity: { x: 0, y: 0, z: 0 },
    force: 0,
    torque: 0,
    stopped: true,
    currentDraw: 0,
    temperature: 25, // Ambient
    stress: 0,
  };

  constructor(name: string, config: VirtualActuatorConfig) {
    this.id = driverId(`vact-${name}-${Date.now()}`);
    this.name = config.name;
    this.config = config;
  }

  async init(): Promise<void> {
    this.state = DriverState.LOADING;
    // Simulate firmware load (real drivers would do calibration here)
    this.initTime = monotonicNow();
    this.motor = {
      position: { x: 0, y: 0, z: 0 },
      velocity: { x: 0, y: 0, z: 0 },
      force: 0,
      torque: 0,
      stopped: true,
      currentDraw: 0,
      temperature: 25,
      stress: 0,
    };
    this.state = DriverState.READY;
    this.emergencyStopped = false;
    this.jammed = false;
  }

  read(): DriverTelemetry {
    return {
      driverId: this.id,
      timestamp: monotonicNow(),
      values: {
        posX: this.motor.position.x,
        posY: this.motor.position.y,
        posZ: this.motor.position.z,
        velX: this.motor.velocity.x,
        velY: this.motor.velocity.y,
        velZ: this.motor.velocity.z,
        force: this.motor.force,
        torque: this.motor.torque,
        currentDraw: this.motor.currentDraw,
        temperature: this.motor.temperature,
        stress: this.motor.stress,
        jammed: this.jammed ? 1 : 0,
        commandCount: this.commandCount,
      },
      healthy: this.state !== DriverState.ERROR && !this.emergencyStopped,
    };
  }

  write(command: DriverCommand): DriverWriteResult {
    if (this.emergencyStopped) {
      return {
        accepted: false,
        actualValues: {},
        clampedFields: [],
        timestamp: monotonicNow(),
      };
    }

    this.commandCount++;
    const clampedFields: string[] = [];
    const actualValues: Record<string, number> = {};
    const c = this.config.constraints;

    // Process command based on type
    switch (command.type) {
      case 'MOVE': {
        // Apply force with capping (defense in depth)
        let force = command.params['force'] ?? 0;
        if (Math.abs(force) > c.maxForceN) {
          force = Math.sign(force) * c.maxForceN;
          clampedFields.push('force');
        }
        this.motor.force = force;
        actualValues['force'] = force;

        // Simulate physics: F = ma -> a = F/m
        const dt = 0.016; // ~60Hz simulation step

        if (this.jammed) {
          // JAM: motor draws current but nothing moves
          // Current rises with applied force (motor stall behavior)
          this.motor.currentDraw = Math.abs(force) * 0.1; // Amps
          this.motor.temperature += this.motor.currentDraw * 0.5; // Heating
          this.motor.stress = Math.min(1, Math.abs(force) / c.maxForceN);
          this.motor.velocity.x = 0; // No movement
          // Position unchanged — this is the key proprioceptive signal
          actualValues['velocity'] = 0;
          actualValues['position'] = this.motor.position.x;
          actualValues['currentDraw'] = this.motor.currentDraw;
          actualValues['temperature'] = this.motor.temperature;
          actualValues['stress'] = this.motor.stress;
        } else {
          const accel = force / this.config.massKg;
          const frictionDecel = this.motor.velocity.x * this.config.friction;

          this.motor.velocity.x += (accel - frictionDecel) * dt;

          // Velocity capping
          if (Math.abs(this.motor.velocity.x) > c.maxVelocityMs) {
            this.motor.velocity.x = Math.sign(this.motor.velocity.x) * c.maxVelocityMs;
            clampedFields.push('velocity');
          }

          // Update position
          this.motor.position.x += this.motor.velocity.x * dt;
          actualValues['velocity'] = this.motor.velocity.x;
          actualValues['position'] = this.motor.position.x;

          // Normal operation: low current, stable temp
          this.motor.currentDraw = Math.abs(force) * 0.02;
          this.motor.temperature = Math.max(25, this.motor.temperature - 0.1); // Cooling
          this.motor.stress = Math.abs(force) / c.maxForceN * 0.3;
        }

        // Geofence check at driver level (defense in depth)
        const [minX, , , maxX] = c.geofenceBounds;
        if (this.motor.position.x < minX || this.motor.position.x > maxX) {
          // Clamp position and zero velocity — driver-level safety
          this.motor.position.x = Math.max(minX, Math.min(maxX, this.motor.position.x));
          this.motor.velocity.x = 0;
          this.motor.force = 0;
          clampedFields.push('geofence');
        }

        this.motor.stopped = false;
        break;
      }

      case 'ROTATE': {
        let torque = command.params['torque'] ?? 0;
        if (Math.abs(torque) > c.maxTorqueNm) {
          torque = Math.sign(torque) * c.maxTorqueNm;
          clampedFields.push('torque');
        }
        this.motor.torque = torque;
        actualValues['torque'] = torque;
        break;
      }

      case 'STOP': {
        this.motor.velocity = { x: 0, y: 0, z: 0 };
        this.motor.force = 0;
        this.motor.torque = 0;
        this.motor.stopped = true;
        actualValues['stopped'] = 1;
        break;
      }

      default: {
        this.errorCount++;
        this.lastError = `Unknown command type: ${command.type}`;
        return {
          accepted: false,
          actualValues: {},
          clampedFields: [],
          timestamp: monotonicNow(),
        };
      }
    }

    return {
      accepted: true,
      actualValues,
      clampedFields,
      timestamp: monotonicNow(),
    };
  }

  async shutdown(): Promise<void> {
    this.motor.velocity = { x: 0, y: 0, z: 0 };
    this.motor.force = 0;
    this.motor.torque = 0;
    this.motor.stopped = true;
    this.state = DriverState.UNLOADED;
  }

  healthcheck(): DriverHealthStatus {
    return {
      healthy: this.state !== DriverState.ERROR && !this.emergencyStopped,
      state: this.emergencyStopped ? DriverState.UNLOADED : this.state,
      uptimeMs: this.initTime > 0n ? nsToMs(monotonicNow() - this.initTime) : 0,
      errorCount: this.errorCount,
      lastError: this.lastError,
    };
  }

  /**
   * EMERGENCY STOP — synchronous, < 1ms
   *
   * Immediately zeros all motion. No gradual deceleration.
   * This is the last line of defense before physical damage.
   */
  emergencyStop(): void {
    this.motor.velocity = { x: 0, y: 0, z: 0 };
    this.motor.force = 0;
    this.motor.torque = 0;
    this.motor.stopped = true;
    this.emergencyStopped = true;
    this.state = DriverState.UNLOADED;
  }

  getConstraints(): PhysicalConstraints {
    return this.config.constraints;
  }

  /** Test helper — check if emergency stop was triggered */
  isEmergencyStopped(): boolean {
    return this.emergencyStopped;
  }

  /** Test helper — get current motor state */
  getMotorState(): Readonly<MotorState> {
    return { ...this.motor };
  }

  /** Simulate mechanical jam (blocked wheel, debris, etc.) */
  simulateJam(): void {
    this.jammed = true;
  }

  /** Clear a simulated jam */
  clearJam(): void {
    this.jammed = false;
    this.motor.stress = 0;
    this.motor.currentDraw = 0;
  }

  /** Is the actuator currently jammed? */
  isJammed(): boolean {
    return this.jammed;
  }
}
