"""
OASIS → Crazyflie Bridge (cflib)

Hardware-in-the-loop wrapper: connects to a Crazyflie 2.1 (physical drone OR
crazyflie-firmware SITL) via cflib, spawns the OASIS Rust kernel (drone_bridge.exe)
as subprocess, and pipes sensor/motor data between the two.

Protocol:
  Crazyflie log stream (IMU, position, range) → OASIS JSON sensor packet → kernel
  Kernel JSON motor command → Crazyflie high-level commander setpoint

Usage:
  # SITL (software-in-the-loop, no hardware needed):
  #   Build crazyflie-firmware with: make sitl
  #   Run SITL server: ./build/sitl/cf2_sitl
  python oasis_cf_bridge.py --uri udp://127.0.0.1:19850 --name patrol1

  # Physical Crazyflie 2.1 + Crazyradio PA:
  python oasis_cf_bridge.py --uri radio://0/80/2M/E7E7E7E7E7 --name patrol1

Requirements:
  pip install cflib
  cargo build --release --bin drone_bridge  (in ../oasis-rt/)

Status: PROTOTYPE — protocol adapter validated in code, real flight untested.
"""

import argparse
import json
import logging
import os
import subprocess
import sys
import time

try:
    import cflib.crtp
    from cflib.crazyflie import Crazyflie
    from cflib.crazyflie.log import LogConfig
    from cflib.crazyflie.syncCrazyflie import SyncCrazyflie
except ImportError:
    print("ERROR: cflib not installed. Run: pip install cflib", file=sys.stderr)
    sys.exit(1)


BRIDGE_BIN = os.environ.get(
    "OASIS_DRONE_BRIDGE",
    os.path.join(os.path.dirname(__file__), "..", "oasis-rt", "target",
                 "release", "drone_bridge" + (".exe" if os.name == "nt" else ""))
)


class OasisCrazyflieBridge:
    """Adapter between Crazyflie cflib and OASIS Rust kernel via subprocess."""

    def __init__(self, uri: str, name: str, drone_id: int = 0, log_path: str = None):
        self.uri = uri
        self.name = name
        self.drone_id = drone_id
        self.log_path = log_path or f"./factory_{name}.log"
        self.tick = 0
        self.latest_sensors = {
            "x": 0.0, "y": 0.0, "alt": 0.0,
            "roll": 0.0, "pitch": 0.0, "gz": 0.0,
            "rf": 2.0, "rl": 2.0, "rr": 2.0, "rb": 2.0,
            "vx": 0.0, "vy": 0.0,
        }
        self.kernel_cmd = {"dvx": 0.0, "dvy": 0.0, "s_alt": 0.3, "abort": False}
        self._running = False
        self._log_fh = None
        self._bridge_proc = None
        self._cf_sync = None

    def _spawn_kernel(self):
        """Start the OASIS Rust drone_bridge as subprocess."""
        self._log_fh = open(self.log_path, "w", encoding="utf-8", buffering=1)
        self._log_fh.write(f"=== OASIS-CF bridge: {self.name} uri={self.uri} ===\n")
        self._log_fh.write(f"  Kernel binary: {BRIDGE_BIN}\n")
        if not os.path.exists(BRIDGE_BIN):
            raise FileNotFoundError(
                f"drone_bridge binary not found at {BRIDGE_BIN}. "
                f"Build with: cd oasis-rt && cargo build --release --bin drone_bridge"
            )
        self._bridge_proc = subprocess.Popen(
            [BRIDGE_BIN, self.name, str(self.drone_id)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self._log_fh,
            bufsize=1, text=True,
        )
        self._log_fh.write(f"  Kernel PID: {self._bridge_proc.pid}\n")

    def _kernel_tick(self, sensors: dict) -> dict:
        """Send sensors to kernel, receive motor command."""
        pkt = json.dumps({"tick": self.tick, **sensors}) + "\n"
        self._bridge_proc.stdin.write(pkt)
        self._bridge_proc.stdin.flush()
        line = self._bridge_proc.stdout.readline()
        if not line:
            raise RuntimeError("OASIS kernel stdout EOF")
        return json.loads(line.strip())

    def _setup_cf_logging(self, scf):
        """Subscribe to Crazyflie log streams (state estimate + IMU + range)."""
        cf = scf.cf

        # State: x, y, z, vx, vy
        log_state = LogConfig(name="state", period_in_ms=20)
        log_state.add_variable("stateEstimate.x", "float")
        log_state.add_variable("stateEstimate.y", "float")
        log_state.add_variable("stateEstimate.z", "float")
        log_state.add_variable("stateEstimate.vx", "float")
        log_state.add_variable("stateEstimate.vy", "float")

        # Attitude: roll, pitch, yaw, gyro.z
        log_att = LogConfig(name="att", period_in_ms=20)
        log_att.add_variable("stabilizer.roll", "float")
        log_att.add_variable("stabilizer.pitch", "float")
        log_att.add_variable("stabilizer.yaw", "float")
        log_att.add_variable("gyro.z", "float")

        def cb_state(ts, data, cfg):
            self.latest_sensors["x"] = data["stateEstimate.x"]
            self.latest_sensors["y"] = data["stateEstimate.y"]
            self.latest_sensors["alt"] = data["stateEstimate.z"]
            self.latest_sensors["vx"] = data["stateEstimate.vx"]
            self.latest_sensors["vy"] = data["stateEstimate.vy"]

        def cb_att(ts, data, cfg):
            self.latest_sensors["roll"] = data["stabilizer.roll"] * 0.01745  # deg→rad
            self.latest_sensors["pitch"] = data["stabilizer.pitch"] * 0.01745
            self.latest_sensors["yaw"] = data["stabilizer.yaw"] * 0.01745
            self.latest_sensors["gz"] = data["gyro.z"] * 0.01745

        cf.log.add_config(log_state); cf.log.add_config(log_att)
        log_state.data_received_cb.add_callback(cb_state)
        log_att.data_received_cb.add_callback(cb_att)
        log_state.start(); log_att.start()

        # Optional: Multi-ranger deck (front/back/left/right distances)
        try:
            log_range = LogConfig(name="range", period_in_ms=50)
            for side in ["front", "back", "left", "right"]:
                log_range.add_variable(f"range.{side}", "uint16_t")

            def cb_range(ts, data, cfg):
                self.latest_sensors["rf"] = data["range.front"] / 1000.0
                self.latest_sensors["rb"] = data["range.back"] / 1000.0
                self.latest_sensors["rl"] = data["range.left"] / 1000.0
                self.latest_sensors["rr"] = data["range.right"] / 1000.0

            cf.log.add_config(log_range)
            log_range.data_received_cb.add_callback(cb_range)
            log_range.start()
        except Exception as e:
            self._log_fh.write(f"  no multi-ranger deck ({e}) — using default 2.0m\n")

    def _apply_kernel_cmd(self, scf, cmd: dict):
        """Apply OASIS motor command to Crazyflie. Rotates world-frame OASIS
        commands (dvx, dvy) into body-frame for hover setpoint, using the
        drone's current yaw. Without this, drift in yaw causes the drone to
        head in the wrong direction relative to the OASIS world model."""
        import math
        cf = scf.cf
        if cmd.get("abort", False):
            cf.commander.send_stop_setpoint()
            return
        # Current yaw from latest log (deg→rad; cflib reports stabilizer.yaw
        # but we haven't subscribed — read from attitude log if available).
        yaw = self.latest_sensors.get("yaw", 0.0)  # rad
        dvx_w = float(cmd.get("dvx", 0.0))
        dvy_w = float(cmd.get("dvy", 0.0))
        c, s = math.cos(yaw), math.sin(yaw)
        dvx_b =  c * dvx_w + s * dvy_w
        dvy_b = -s * dvx_w + c * dvy_w
        cf.commander.send_hover_setpoint(
            dvx_b, dvy_b,
            0.0,  # yaw rate — keep heading constant relative to takeoff
            float(cmd.get("s_alt", 0.5)),
        )

    def run(self, duration_s: float = 60.0):
        """Main loop: connect to CF, spawn kernel, relay sensors ↔ commands."""
        cflib.crtp.init_drivers()
        self._spawn_kernel()

        with SyncCrazyflie(self.uri, cf=Crazyflie(rw_cache="./cache")) as scf:
            self._cf_sync = scf
            self._setup_cf_logging(scf)
            time.sleep(1.0)  # let logs populate
            self._log_fh.write(f"  CF connected: {self.uri}\n")

            scf.cf.commander.send_setpoint(0, 0, 0, 0)  # unlock flight commander
            self._running = True
            t0 = time.time()
            dt_target = 0.02  # 50 Hz control loop

            while self._running and (time.time() - t0) < duration_s:
                loop_start = time.time()
                self.tick += 1
                try:
                    self.kernel_cmd = self._kernel_tick(self.latest_sensors)
                except RuntimeError as e:
                    self._log_fh.write(f"  !!! kernel error: {e}\n")
                    break
                self._apply_kernel_cmd(scf, self.kernel_cmd)
                # Pace loop
                elapsed = time.time() - loop_start
                if elapsed < dt_target:
                    time.sleep(dt_target - elapsed)

            scf.cf.commander.send_stop_setpoint()
            self._log_fh.write(f"=== END tick={self.tick} elapsed={time.time()-t0:.1f}s ===\n")

        if self._bridge_proc:
            try:
                self._bridge_proc.stdin.close()
                self._bridge_proc.wait(timeout=2)
            except Exception:
                self._bridge_proc.kill()
        if self._log_fh:
            self._log_fh.close()


def main():
    p = argparse.ArgumentParser(description="OASIS → Crazyflie bridge")
    p.add_argument("--uri", required=True, help="Crazyflie URI (radio:// or udp://)")
    p.add_argument("--name", default="patrol1", help="Drone name (patrol1|patrol2|supervisor)")
    p.add_argument("--id", type=int, default=0, help="Drone ID (0..N)")
    p.add_argument("--duration", type=float, default=60.0, help="Flight duration in seconds")
    p.add_argument("--log", default=None, help="Log file path")
    args = p.parse_args()

    logging.basicConfig(level=logging.INFO)
    bridge = OasisCrazyflieBridge(args.uri, args.name, args.id, args.log)
    bridge.run(args.duration)


if __name__ == "__main__":
    main()
