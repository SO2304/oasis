/**
 * OASIS Kernel — Termux Sensor HAL Driver
 *
 * Bridges Android phone sensors (via Termux API) into the OASIS HAL.
 * This is a REAL hardware driver — not simulated.
 *
 * Supported sensors (Samsung Galaxy):
 * - LSM6DSVTR Accelerometer (IMU — gravity + linear acceleration)
 * - LSM6DSVTR Gyroscope (IMU — angular velocity)
 * - AK09918C Magnetometer (compass heading)
 * - BMP580 Barometer (altitude via atmospheric pressure)
 * - STK31610 Light (ambient luminosity in lux)
 * - Step Counter (pedometer)
 * - Proximity (hover detection)
 * - Battery (temperature, voltage, health)
 *
 * Data flow: Termux API → JSON → TermuxSensorDriver.ingest() → DriverTelemetry
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

// ─── Termux Sensor Data Schema ────────────────────────────────

export interface TermuxSensorData {
  'LSM6DSVTR Accelerometer'?: { values: [number, number, number] };
  'LSM6DSVTR Gyroscope'?: { values: [number, number, number] };
  'AK09918C Magnetometer'?: { values: [number, number, number] };
  'BMP580 Barometer'?: { values: [number] };
  'STK31610 Light'?: { values: [number] };
  'Step Counter'?: { values: [number] };
  'Hover Proximity'?: { values: [number] };
  'Samsung Gravity Sensor'?: { values: [number, number, number] };
  'Samsung Linear Acceleration Sensor'?: { values: [number, number, number] };
  'Samsung Orientation Sensor'?: { values: [number, number, number] };
}

export interface TermuxBatteryData {
  health: string;
  percentage: number;
  plugged: string;
  status: string;
  temperature: number;
  voltage: number;
  current: number;
}

// ─── Derived Metrics ──────────────────────────────────────────

interface SensorState {
  // Raw IMU
  accelX: number; accelY: number; accelZ: number;
  gyroX: number; gyroY: number; gyroZ: number;
  magX: number; magY: number; magZ: number;
  // Environment
  pressure: number;       // hPa
  light: number;          // lux
  proximity: number;      // cm
  // Motion
  steps: number;
  // Orientation (Euler angles from Samsung sensor)
  azimuth: number;        // compass heading [0, 360]
  pitch: number;          // front-back tilt [-180, 180]
  roll: number;           // left-right tilt [-90, 90]
  // Battery as thermal/power sensor
  batteryTemp: number;    // celsius
  batteryVoltage: number; // mV
  batteryCurrent: number; // µA
  batteryPercent: number;
  // Derived
  accelMagnitude: number;     // total acceleration (should be ~9.81 when still)
  gyroMagnitude: number;      // total angular velocity (should be ~0 when still)
  magMagnitude: number;       // total magnetic field strength
  gravityDeviation: number;   // |accel| - 9.81 = how much the phone is being moved
  estimatedAltitude: number;  // from barometric formula
  motionIntensity: number;    // [0, 1] composite motion score
  stability: number;          // [0, 1] inverse of motion (1 = perfectly still)
}

const STANDARD_GRAVITY = 9.80665;
const SEA_LEVEL_PRESSURE = 1013.25; // hPa

export class TermuxSensorDriver implements IHALDriver {
  readonly id: DriverId;
  readonly name = 'Termux Android Sensor Array';
  readonly version = '1.0.0';
  readonly isSimulated = false; // THIS IS REAL HARDWARE

  private state: DriverState = DriverState.UNLOADED;
  private initTime: bigint = 0n;
  private errorCount = 0;
  private lastError: string | null = null;
  private readCount = 0;
  private lastIngestTime: bigint = 0n;
  private emergencyStopped = false;

  // Smoothed sensor state
  private current: SensorState = this.defaultState();
  private previous: SensorState = this.defaultState();

  // Rolling history for jitter/stability calculation
  private readonly accelHistory: number[] = [];
  private readonly gyroHistory: number[] = [];
  private readonly HISTORY_SIZE = 20;

  constructor(driverName: string = 'android-sensors') {
    this.id = driverId(`termux-${driverName}-${Date.now()}`);
  }

  // ─── IHALDriver Implementation ──────────────────────────────

  async init(): Promise<void> {
    this.state = DriverState.LOADING;
    this.initTime = monotonicNow();
    this.current = this.defaultState();
    this.previous = this.defaultState();
    this.accelHistory.length = 0;
    this.gyroHistory.length = 0;
    this.emergencyStopped = false;
    this.state = DriverState.READY;
  }

  read(): DriverTelemetry {
    this.readCount++;
    const s = this.current;
    return {
      driverId: this.id,
      timestamp: monotonicNow(),
      values: {
        // Raw IMU
        accelX: s.accelX, accelY: s.accelY, accelZ: s.accelZ,
        gyroX: s.gyroX, gyroY: s.gyroY, gyroZ: s.gyroZ,
        magX: s.magX, magY: s.magY, magZ: s.magZ,
        // Environment
        pressure: s.pressure, light: s.light, proximity: s.proximity,
        // Orientation
        azimuth: s.azimuth, pitch: s.pitch, roll: s.roll,
        // Motion
        steps: s.steps,
        // Battery-as-sensor
        temperature: s.batteryTemp, voltage: s.batteryVoltage,
        current: s.batteryCurrent, batteryPercent: s.batteryPercent,
        // Derived metrics
        accelMagnitude: s.accelMagnitude,
        gyroMagnitude: s.gyroMagnitude,
        magMagnitude: s.magMagnitude,
        gravityDeviation: s.gravityDeviation,
        estimatedAltitude: s.estimatedAltitude,
        motionIntensity: s.motionIntensity,
        stability: s.stability,
        // Health
        force: s.accelMagnitude * 0.175,   // phone mass ~175g
        stress: s.motionIntensity,
        distance: s.proximity,
      },
      healthy: this.state !== DriverState.ERROR && !this.emergencyStopped,
    };
  }

  write(command: DriverCommand): DriverWriteResult {
    // Sensor driver is read-only — but we accept CONFIGURE commands
    if (command.type === 'CONFIGURE') {
      return {
        accepted: true,
        actualValues: command.params,
        clampedFields: [],
        timestamp: monotonicNow(),
      };
    }
    return {
      accepted: false,
      actualValues: {},
      clampedFields: [],
      timestamp: monotonicNow(),
    };
  }

  async shutdown(): Promise<void> {
    this.state = DriverState.UNLOADED;
  }

  healthcheck(): DriverHealthStatus {
    const staleness = this.lastIngestTime > 0n
      ? nsToMs(monotonicNow() - this.lastIngestTime)
      : 0;
    return {
      healthy: this.state === DriverState.ACTIVE && staleness < 5000,
      state: this.state,
      uptimeMs: this.initTime > 0n ? nsToMs(monotonicNow() - this.initTime) : 0,
      errorCount: this.errorCount,
      lastError: this.lastError,
    };
  }

  emergencyStop(): void {
    this.emergencyStopped = true;
    this.state = DriverState.UNLOADED;
  }

  getConstraints(): PhysicalConstraints | null {
    // Phone is a sensor platform, not an actuator
    // But we define "geofence" as acceptable sensor ranges
    return null;
  }

  // ─── Data Ingestion (called by the bridge) ──────────────────

  /**
   * Ingest raw Termux sensor JSON data.
   * This is the hot path — called every sensor polling cycle.
   */
  ingestSensors(data: TermuxSensorData): void {
    this.previous = { ...this.current };
    this.lastIngestTime = monotonicNow();

    const s = this.current;

    // Accelerometer (m/s²)
    const accel = data['LSM6DSVTR Accelerometer'];
    if (accel) {
      s.accelX = accel.values[0];
      s.accelY = accel.values[1];
      s.accelZ = accel.values[2];
      s.accelMagnitude = Math.sqrt(
        s.accelX * s.accelX + s.accelY * s.accelY + s.accelZ * s.accelZ,
      );
      s.gravityDeviation = Math.abs(s.accelMagnitude - STANDARD_GRAVITY);
    }

    // Gyroscope (rad/s)
    const gyro = data['LSM6DSVTR Gyroscope'];
    if (gyro) {
      s.gyroX = gyro.values[0];
      s.gyroY = gyro.values[1];
      s.gyroZ = gyro.values[2];
      s.gyroMagnitude = Math.sqrt(
        s.gyroX * s.gyroX + s.gyroY * s.gyroY + s.gyroZ * s.gyroZ,
      );
    }

    // Magnetometer (µT)
    const mag = data['AK09918C Magnetometer'];
    if (mag) {
      s.magX = mag.values[0];
      s.magY = mag.values[1];
      s.magZ = mag.values[2];
      s.magMagnitude = Math.sqrt(
        s.magX * s.magX + s.magY * s.magY + s.magZ * s.magZ,
      );
    }

    // Barometer → altitude
    const baro = data['BMP580 Barometer'];
    if (baro) {
      s.pressure = baro.values[0];
      // Hypsometric formula: altitude ≈ 44330 × (1 - (P/P0)^0.1903)
      s.estimatedAltitude = 44330 * (1 - Math.pow(s.pressure / SEA_LEVEL_PRESSURE, 0.1903));
    }

    // Light (lux)
    const light = data['STK31610 Light'];
    if (light) {
      s.light = light.values[0];
    }

    // Step Counter
    const steps = data['Step Counter'];
    if (steps) {
      s.steps = steps.values[0];
    }

    // Proximity
    const prox = data['Hover Proximity'];
    if (prox) {
      s.proximity = prox.values[0];
    }

    // Orientation (azimuth, pitch, roll in degrees)
    const orient = data['Samsung Orientation Sensor'];
    if (orient) {
      s.azimuth = orient.values[0];
      s.pitch = orient.values[1];
      s.roll = orient.values[2];
    }

    // Update rolling history for stability
    this.accelHistory.push(s.gravityDeviation);
    this.gyroHistory.push(s.gyroMagnitude);
    if (this.accelHistory.length > this.HISTORY_SIZE) this.accelHistory.shift();
    if (this.gyroHistory.length > this.HISTORY_SIZE) this.gyroHistory.shift();

    // Compute derived metrics
    this.computeDerived();
  }

  /** Ingest battery data as thermal/power sensor */
  ingestBattery(data: TermuxBatteryData): void {
    this.current.batteryTemp = data.temperature;
    this.current.batteryVoltage = data.voltage;
    this.current.batteryCurrent = data.current;
    this.current.batteryPercent = data.percentage;
  }

  /** Get current state snapshot (for debugging/display) */
  getState(): Readonly<SensorState> {
    return { ...this.current };
  }

  /** Get previous state (for delta computation) */
  getPreviousState(): Readonly<SensorState> {
    return { ...this.previous };
  }

  // ─── Private ────────────────────────────────────────────────

  private computeDerived(): void {
    const s = this.current;

    // Motion intensity: composite of acceleration deviation + gyro activity
    // Normalized to [0, 1] with soft saturation
    const accelMotion = Math.min(1, s.gravityDeviation / 5.0);  // 5 m/s² = very shaky
    const gyroMotion = Math.min(1, s.gyroMagnitude / 3.0);      // 3 rad/s = fast rotation
    s.motionIntensity = Math.min(1, accelMotion * 0.6 + gyroMotion * 0.4);

    // Stability: rolling variance of acceleration deviation
    if (this.accelHistory.length >= 3) {
      const mean = this.accelHistory.reduce((a, b) => a + b, 0) / this.accelHistory.length;
      const variance = this.accelHistory.reduce((sum, v) => sum + (v - mean) ** 2, 0)
        / this.accelHistory.length;
      // Low variance = high stability
      s.stability = Math.max(0, Math.min(1, 1 - Math.sqrt(variance) * 2));
    } else {
      s.stability = 0.5; // Unknown
    }
  }

  private defaultState(): SensorState {
    return {
      accelX: 0, accelY: 0, accelZ: STANDARD_GRAVITY,
      gyroX: 0, gyroY: 0, gyroZ: 0,
      magX: 0, magY: 0, magZ: 0,
      pressure: SEA_LEVEL_PRESSURE,
      light: 0, proximity: 100, steps: 0,
      azimuth: 0, pitch: 0, roll: 0,
      batteryTemp: 25, batteryVoltage: 4200,
      batteryCurrent: 0, batteryPercent: 100,
      accelMagnitude: STANDARD_GRAVITY,
      gyroMagnitude: 0, magMagnitude: 0,
      gravityDeviation: 0, estimatedAltitude: 0,
      motionIntensity: 0, stability: 1,
    };
  }
}
