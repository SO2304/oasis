"""
OASIS Factory — Webots thin-client, real OASIS Rust kernel via subprocess.

Webots provides only sensors (IMU, GPS, gyro, 4 sonars) and motors.
All cognition — HyperState (128D), WorldModel (Claim 10), EmotionalState
(Claim 5), FederatedMesh (Claim 11), reflexes (Claim 9) — runs in
oasis-rt/target/release/drone_bridge.exe built from the Rust kernel with
125 unit tests passed on PC + 3h33 continuous daemon session on phone.

Per Webots step:
  1. Read sensors, serialize as JSON line, send to bridge stdin
  2. Read command JSON line from bridge stdout (dvx, dvy, s_alt, abort)
  3. PID mixes command → motor velocities (this file is just a body driver)
"""

import sys, os, json, subprocess
from controller import Robot


class PID:
    def __init__(self):
        self.pvxe=0;self.pvye=0;self.pae=0;self.ppe=0;self.pre=0;self.ai=0
    def pid(self,dt,dvx,dvy,dyaw,dalt,roll,pitch,yr,alt,vx,vy):
        g={"kp_att_y":1,"kd_att_y":0.5,"kp_att_rp":0.5,"kd_att_rp":0.1,
           "kp_vel_xy":2,"kd_vel_xy":0.5,"kp_z":10,"ki_z":5,"kd_z":5}
        vxe=dvx-vx;vxd=(vxe-self.pvxe)/dt;vye=dvy-vy;vyd=(vye-self.pvye)/dt
        dp=g["kp_vel_xy"]*max(-1,min(1,vxe))+g["kd_vel_xy"]*vxd
        dr=-g["kp_vel_xy"]*max(-1,min(1,vye))-g["kd_vel_xy"]*vyd
        self.pvxe=vxe;self.pvye=vye
        ae=dalt-alt;ad=(ae-self.pae)/dt;self.ai+=ae*dt
        ac=g["kp_z"]*ae+g["kd_z"]*ad+g["ki_z"]*max(-2,min(2,self.ai))+48;self.pae=ae
        pe=dp-pitch;pd=(pe-self.ppe)/dt;re=dr-roll;rd=(re-self.pre)/dt;ye=dyaw-yr
        rc=g["kp_att_rp"]*max(-1,min(1,re))+g["kd_att_rp"]*rd
        pc=-g["kp_att_rp"]*max(-1,min(1,pe))-g["kd_att_rp"]*pd
        yc=g["kp_att_y"]*max(-1,min(1,ye))
        self.ppe=pe;self.pre=re
        return[max(0,min(600,ac-rc+pc+yc)),max(0,min(600,ac-rc-pc-yc)),
               max(0,min(600,ac+rc-pc+yc)),max(0,min(600,ac+rc+pc-yc))]


BRIDGE = "C:/dev/oasis/oasis-rt/target/release/drone_bridge.exe"

if __name__ == '__main__':
    robot = Robot()
    ts = int(robot.getBasicTimeStep())
    args = sys.argv[1:] if len(sys.argv) > 1 else ["patrol1", "0"]
    name = args[0]; did = args[1] if len(args) > 1 else "0"

    # Fault injection schedule: (tick_start, tick_end, sensor_name)
    # Sensor dies at tick_start, revives at tick_end. Applies to patrol1 only
    # (others run clean for comparison). Set FAULT_INJECT=1 env to enable.
    FAULTS = []
    if os.environ.get("FAULT_INJECT") == "1" and name == "patrol1":
        FAULTS = [
            (500, 1500,  "sonar"),   # Degraded (Important down)
            (2000, 3000, "gps"),     # Critical (2 Vitals down when stacked)
            (2500, 3500, "imu"),     # Critical+ (3 Vitals down → Dead after 100 ticks)
        ]

    log_path = f"C:/dev/oasis/webots/factory_{name}.log"
    log_file = open(log_path, "w", encoding="utf-8", buffering=1)
    def log(m): log_file.write(m + "\n")

    # Per-run metrics (patrol1 only during fault experiments)
    MAX_TICKS = int(os.environ.get("MAX_TICKS", "0"))  # 0 = unlimited
    gate_mode = os.environ.get("OASIS_GATE_MODE", "full")
    run_seed = os.environ.get("OASIS_RUN_SEED", "0")
    metrics_path = f"C:/dev/oasis/webots/metrics_{name}_{gate_mode}_seed{run_seed}.csv"
    metrics_fh = open(metrics_path, "w", encoding="utf-8", buffering=1)
    metrics_fh.write("tick,x,y,alt,dvx,dvy,energy,min_obs,imu_alive,gps_alive,sonar_alive\n")
    first_recovery_tick = 0  # tick of first movement after T3500 recovery

    log(f"=== WEBOTS THIN-CLIENT: {name} id={did} ===")
    log(f"  Bridge binary: {BRIDGE}")
    log(f"  Protocol: JSON lines over stdin/stdout")

    # Spawn the REAL OASIS Rust kernel as subprocess, pipe stderr to our log
    bridge = subprocess.Popen(
        [BRIDGE, name, did],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=log_file,
        bufsize=1,
        text=True,
        creationflags=0x08000000 if os.name == 'nt' else 0,  # CREATE_NO_WINDOW
    )
    log(f"  Bridge PID: {bridge.pid}")

    motors = []
    for mn in ["m1_motor", "m2_motor", "m3_motor", "m4_motor"]:
        m = robot.getDevice(mn); m.setPosition(float('inf'))
        m.setVelocity(-1 if mn in ["m1_motor", "m3_motor"] else 1); motors.append(m)

    imu = robot.getDevice("inertial_unit"); imu.enable(ts)
    gps = robot.getDevice("gps"); gps.enable(ts)
    gyro = robot.getDevice("gyro"); gyro.enable(ts)
    rf_s = robot.getDevice("range_front"); rf_s.enable(ts)
    rl_s = robot.getDevice("range_left"); rl_s.enable(ts)
    rr_s = robot.getDevice("range_right"); rr_s.enable(ts)
    rb_s = robot.getDevice("range_back"); rb_s.enable(ts)

    pid = PID()
    px = 0.0; py = 0.0; tick = 0
    s_alt_cmd = 1.30
    dvx_cmd = 0.0; dvy_cmd = 0.0; aborted_cmd = False

    # --- main loop ---
    while robot.step(ts) != -1:
        tick += 1; dt = ts / 1000.0
        roll, pitch, _yaw = imu.getRollPitchYaw()
        gx, gy, gz = gyro.getValues()
        x, y, alt = gps.getValues()
        vx = (x - px) / dt if tick > 1 else 0.0
        vy = (y - py) / dt if tick > 1 else 0.0
        px = x; py = y
        rf = rf_s.getValue()/1000; rl = rl_s.getValue()/1000
        rr = rr_s.getValue()/1000; rb = rb_s.getValue()/1000

        # Apply fault injection schedule — only alive FLAGS are toggled.
        # The Python PID keeps using the real Webots sensor readings (drone flies
        # safely regardless of OASIS gate decisions). This isolates the R14 effect:
        # if the gate blocks actuation, drone stops; if not, drone flies normally.
        imu_alive = True; gps_alive = True; sonar_alive = True
        for (t_start, t_end, sensor) in FAULTS:
            if t_start <= tick < t_end:
                if sensor == "imu":   imu_alive = False
                if sensor == "gps":   gps_alive = False
                if sensor == "sonar": sonar_alive = False
        if tick == 1 or any(tick == t for (t, _, _) in FAULTS) or any(tick == t for (_, t, _) in FAULTS):
            log(f"  [FAULT] T{tick} imu={imu_alive} gps={gps_alive} sonar={sonar_alive}")

        # Send REAL sensor data to Rust (alive flags indicate fault for vitality calc).
        pkt = (
            f'{{"tick":{tick},"x":{x:.4f},"y":{y:.4f},"alt":{alt:.4f},'
            f'"roll":{roll:.4f},"pitch":{pitch:.4f},"gz":{gz:.4f},'
            f'"rf":{rf:.4f},"rl":{rl:.4f},"rr":{rr:.4f},"rb":{rb:.4f},'
            f'"vx":{vx:.4f},"vy":{vy:.4f},'
            f'"imu_alive":{str(imu_alive).lower()},'
            f'"gps_alive":{str(gps_alive).lower()},'
            f'"sonar_alive":{str(sonar_alive).lower()}}}\n'
        )
        try:
            bridge.stdin.write(pkt); bridge.stdin.flush()
        except Exception as e:
            log(f"  !!! bridge stdin FAILED: {e}")
            break

        # Read command back from Rust kernel (blocking, one line per tick)
        try:
            line = bridge.stdout.readline()
            if not line:
                log(f"  !!! bridge EOF at tick {tick}")
                break
            cmd = json.loads(line.strip())
            dvx_cmd = float(cmd.get("dvx", 0.0))
            dvy_cmd = float(cmd.get("dvy", 0.0))
            s_alt_cmd = float(cmd.get("s_alt", 1.3))
            aborted_cmd = bool(cmd.get("abort", False))
        except Exception as e:
            log(f"  !!! bridge stdout FAILED at tick {tick}: {e}")
            break

        # Abort: motors OFF (reflex from Rust kernel)
        if aborted_cmd:
            for m in motors: m.setVelocity(0)
            continue

        # Safety net: only triggers on true divergence, not on takeoff climb.
        # (3.0m diff leaves room for supervisor cruise=1.70 from ground 0.)
        spd = (vx*vx + vy*vy) ** 0.5
        if spd > 2.5 or abs(alt - s_alt_cmd) > 3.0:
            dvx_cmd = -vx * 0.5; dvy_cmd = -vy * 0.5
            s_alt_cmd = max(0.4, min(2.0, alt)); pid.ai *= 0.5

        # PID → motors
        mp = pid.pid(dt, dvx_cmd, dvy_cmd, 0, s_alt_cmd, roll, pitch, gz, alt, vx, vy)
        motors[0].setVelocity(-mp[0]); motors[1].setVelocity(mp[1])
        motors[2].setVelocity(-mp[2]); motors[3].setVelocity(mp[3])

        # Log per-tick metrics for patrol1 during fault experiments
        if name == "patrol1" and FAULTS:
            energy = abs(dvx_cmd) + abs(dvy_cmd)
            min_obs = min(rf, rl, rr, rb)
            metrics_fh.write(f"{tick},{x:.3f},{y:.3f},{alt:.3f},{dvx_cmd:.4f},{dvy_cmd:.4f},{energy:.4f},{min_obs:.3f},{int(imu_alive)},{int(gps_alive)},{int(sonar_alive)}\n")
            # Track first movement after recovery T3500
            if first_recovery_tick == 0 and tick > 3500 and (abs(vx) + abs(vy)) > 0.05:
                first_recovery_tick = tick

        if MAX_TICKS and tick >= MAX_TICKS:
            log(f"=== MAX_TICKS {MAX_TICKS} reached — terminating ===")
            break

    # Cleanup
    try:
        bridge.stdin.close()
        bridge.wait(timeout=2)
    except Exception:
        bridge.kill()
    log(f"=== END Webots thin-client tick={tick} first_recovery_tick={first_recovery_tick} ===")
    metrics_fh.close()
    log_file.close()
