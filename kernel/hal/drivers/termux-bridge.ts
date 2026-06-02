/**
 * OASIS Kernel — Termux ADB Bridge
 *
 * Handles communication with the Android phone via ADB + Termux API.
 * Polls sensors at configurable rate, parses JSON, feeds TermuxSensorDriver.
 *
 * Architecture:
 *   PC (Node.js) --ADB--> Phone (Termux) --termux-sensor--> Hardware
 *   Phone (JSON) --ADB--> PC (parse) --> TermuxSensorDriver.ingest()
 */

import { execSync, exec } from 'child_process';
import type { TermuxSensorData, TermuxBatteryData, TermuxSensorDriver } from './termux-sensor-driver.js';

export interface TermuxBridgeConfig {
  /** Path to adb executable */
  adbPath: string;
  /** Sensor names to subscribe to */
  sensors: string[];
  /** Polling interval in milliseconds */
  pollIntervalMs: number;
  /** Battery polling interval in milliseconds */
  batteryPollIntervalMs: number;
}

const DEFAULT_CONFIG: TermuxBridgeConfig = {
  adbPath: 'adb',
  sensors: [
    'LSM6DSVTR Accelerometer',
    'LSM6DSVTR Gyroscope',
    'AK09918C Magnetometer',
    'BMP580 Barometer',
    'STK31610 Light',
    'Samsung Orientation Sensor',
  ],
  pollIntervalMs: 100,     // 10 Hz sensor polling
  batteryPollIntervalMs: 5000, // Battery every 5s
};

export class TermuxBridge {
  private readonly config: TermuxBridgeConfig;
  private readonly driver: TermuxSensorDriver;
  private sensorTimer: ReturnType<typeof setInterval> | null = null;
  private batteryTimer: ReturnType<typeof setInterval> | null = null;
  private running = false;
  private pollCount = 0;
  private errorCount = 0;
  private lastPollMs = 0;

  constructor(driver: TermuxSensorDriver, config?: Partial<TermuxBridgeConfig>) {
    this.config = { ...DEFAULT_CONFIG, ...config };
    this.driver = driver;
  }

  /** Verify ADB connection and Termux availability */
  async verify(): Promise<{ connected: boolean; device: string; sensors: string[] }> {
    try {
      const devices = this.adbExec('devices');
      const lines = devices.trim().split('\n').slice(1);
      const device = lines.find(l => l.includes('device'));
      if (!device) {
        return { connected: false, device: '', sensors: [] };
      }

      const deviceId = device.split('\t')[0] ?? '';

      // List available sensors
      const sensorListJson = this.termuxExec('termux-sensor -l');
      const sensorList = JSON.parse(sensorListJson);

      return {
        connected: true,
        device: deviceId,
        sensors: sensorList.sensors || [],
      };
    } catch (err) {
      return { connected: false, device: '', sensors: [] };
    }
  }

  /** Start polling sensors */
  start(): void {
    if (this.running) return;
    this.running = true;

    // Sensor polling loop
    this.sensorTimer = setInterval(() => {
      this.pollSensors();
    }, this.config.pollIntervalMs);

    // Battery polling loop (slower)
    this.batteryTimer = setInterval(() => {
      this.pollBattery();
    }, this.config.batteryPollIntervalMs);

    // Initial polls
    this.pollSensors();
    this.pollBattery();
  }

  /** Stop polling */
  stop(): void {
    this.running = false;
    if (this.sensorTimer) {
      clearInterval(this.sensorTimer);
      this.sensorTimer = null;
    }
    if (this.batteryTimer) {
      clearInterval(this.batteryTimer);
      this.batteryTimer = null;
    }
  }

  /** Single sensor poll — used for synchronous demo mode */
  pollOnce(): TermuxSensorData | null {
    return this.pollSensors();
  }

  /** Get bridge stats */
  getStats(): { pollCount: number; errorCount: number; lastPollMs: number } {
    return {
      pollCount: this.pollCount,
      errorCount: this.errorCount,
      lastPollMs: this.lastPollMs,
    };
  }

  // ─── Private ────────────────────────────────────────────────

  private pollSensors(): TermuxSensorData | null {
    const start = performance.now();
    try {
      const sensorList = this.config.sensors.join(',');
      const cmd = `termux-sensor -s "${sensorList}" -n 1`;
      const raw = this.termuxExec(cmd);
      const data = JSON.parse(raw) as TermuxSensorData;
      this.driver.ingestSensors(data);
      this.pollCount++;
      this.lastPollMs = performance.now() - start;
      return data;
    } catch (err) {
      this.errorCount++;
      return null;
    }
  }

  private pollBattery(): TermuxBatteryData | null {
    try {
      const raw = this.termuxExec('termux-battery-status');
      const data = JSON.parse(raw) as TermuxBatteryData;
      this.driver.ingestBattery(data);
      return data;
    } catch {
      return null;
    }
  }

  /** Execute ADB command */
  private adbExec(cmd: string): string {
    return execSync(`"${this.config.adbPath}" ${cmd}`, {
      encoding: 'utf-8',
      timeout: 10000,
    }).trim();
  }

  /** Execute command in Termux via ADB input */
  private termuxExec(cmd: string): string {
    // Write command to file, execute, read output
    const outFile = `/sdcard/oasis-out-${Date.now()}.json`;
    const escaped = cmd.replace(/ /g, '%s');

    // Method 1: Direct shell execution via run-as (if possible)
    try {
      const result = execSync(
        `"${this.config.adbPath}" shell "export PATH=/data/data/com.termux/files/usr/bin:$PATH && export LD_LIBRARY_PATH=/data/data/com.termux/files/usr/lib && ${cmd}"`,
        { encoding: 'utf-8', timeout: 8000 },
      ).trim();
      if (result.startsWith('{') || result.startsWith('[')) {
        return result;
      }
    } catch {
      // Fall through to method 2
    }

    // Method 2: Via input text to Termux
    execSync(
      `"${this.config.adbPath}" shell "input text '${escaped}%s>%s${outFile}%s2>\\&1'" && "${this.config.adbPath}" shell "input keyevent 66"`,
      { encoding: 'utf-8', timeout: 5000 },
    );
    // Wait for output
    execSync('sleep 2', { timeout: 5000 });
    const result = execSync(
      `"${this.config.adbPath}" shell "cat ${outFile}"`,
      { encoding: 'utf-8', timeout: 5000 },
    ).trim();
    // Cleanup
    try {
      execSync(
        `"${this.config.adbPath}" shell "rm ${outFile}"`,
        { encoding: 'utf-8', timeout: 3000 },
      );
    } catch { /* ignore cleanup errors */ }
    return result;
  }
}
