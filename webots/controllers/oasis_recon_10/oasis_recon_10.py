"""
OASIS Recon-10 — 10 drones, REAL OASIS Rust kernel via drone_bridge.exe subprocess.

Pattern borrowed from oasis_factory.py (Webots thin-client → drone_bridge subprocess).
Addition: wind disturbance + EnvSequencer (alternating EXTREME/CALM) for habituation test.

Per tick:
  1. Read sensors from Webots.
  2. Send JSON to bridge stdin.
  3. Read (dvx, dvy, s_alt, abort, fear, entropy) from bridge stdout.
  4. INJECT wind (±0.08 EXTREME, ±0.01 CALM) into dvx/dvy.
  5. PID → motor velocities.
  6. Log fear (from bridge), wind, env phase to CSV.

Master drone d00 writes env_phase.json. All others read it.
"""
import sys, os, json, math, random, subprocess, time
from controller import Robot

BRIDGE = "C:/dev/oasis/oasis-rt/target/release/drone_bridge.exe"
SHARED = "C:/dev/oasis/webots/recon10_shared/"
os.makedirs(SHARED, exist_ok=True)

TICKS_PER_SECOND = int(1000 / 32)
WARMUP_TICKS = 60 * TICKS_PER_SECOND   # let all 10 drones take off cleanly
PHASE_LEN_TICKS = 25 * TICKS_PER_SECOND
MAX_TICKS = int(os.environ.get("MAX_TICKS", str(40 * 60 * TICKS_PER_SECOND)))

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

class EnvSequencer:
    def __init__(self, is_master):
        self.is_master = is_master
        self.phase = "CALM"; self.phase_idx = 0; self.phase_start_tick = 0
    def step(self, tick):
        if self.is_master:
            if tick < WARMUP_TICKS:
                self.phase = "CALM"; self.phase_idx = 0; self.phase_start_tick = tick
            elif tick - self.phase_start_tick >= PHASE_LEN_TICKS:
                self.phase = "EXTREME" if self.phase == "CALM" else "CALM"
                self.phase_idx += 1; self.phase_start_tick = tick
            try:
                with open(os.path.join(SHARED, "env_phase.json"), "w") as f:
                    json.dump({"phase": self.phase, "idx": self.phase_idx,
                               "tick": tick, "t": time.time()}, f)
            except Exception: pass
        else:
            try:
                with open(os.path.join(SHARED, "env_phase.json")) as f:
                    d = json.load(f)
                    self.phase = d.get("phase", "CALM"); self.phase_idx = d.get("idx", 0)
            except Exception: pass
        return self.phase, self.phase_idx

class Wind:
    def __init__(self, seed):
        self.rng = random.Random(seed)
        self.wx = 0.0; self.wy = 0.0; self.gust_timer = 0
    def update(self, phase):
        self.gust_timer -= 1
        if self.gust_timer <= 0:
            if phase == "EXTREME":
                self.wx = self.rng.uniform(-0.08, 0.08)
                self.wy = self.rng.uniform(-0.08, 0.08)
                self.gust_timer = self.rng.randint(30, 80)
            else:
                self.wx = self.rng.uniform(-0.01, 0.01)
                self.wy = self.rng.uniform(-0.01, 0.01)
                self.gust_timer = self.rng.randint(150, 300)
        return self.wx, self.wy

if __name__ == '__main__':
    robot = Robot()
    ts = int(robot.getBasicTimeStep())
    args = sys.argv[1:] if len(sys.argv) > 1 else ["d00", "0"]
    name = args[0]; did_str = args[1] if len(args) > 1 else "0"
    did = int(did_str)

    log_path = f"C:/dev/oasis/webots/recon10_{name}.log"
    csv_path = f"C:/dev/oasis/webots/recon10_metrics_{name}.csv"
    log_file = open(log_path, "w", encoding="utf-8", buffering=1)
    csv_file = open(csv_path, "w", encoding="utf-8", buffering=1)
    csv_file.write("tick,x,y,alt,fear,entropy,dvx,dvy,wind_x,wind_y,min_obs,inspected,loops,abort,env_phase,phase_idx\n")
    def log(m): log_file.write(m + "\n")

    log(f"=== OASIS RECON-10: {name} (id={did}) ===")
    log(f"  Bridge: {BRIDGE}")
    log(f"  Master: {'YES' if did == 0 else 'no'} warmup={WARMUP_TICKS}t phase_len={PHASE_LEN_TICKS}t")

    # Spawn REAL OASIS Rust kernel as subprocess
    bridge = subprocess.Popen(
        [BRIDGE, name, did_str],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log_file,
        bufsize=1, text=True,
        creationflags=0x08000000 if os.name == 'nt' else 0,
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
    env = EnvSequencer(is_master=(did == 0))
    wind = Wind(did * 42 + 7)

    px = 0.0; py = 0.0; tick = 0
    s_alt_cmd = 1.30; dvx_cmd = 0.0; dvy_cmd = 0.0; aborted_cmd = False
    last_fear = 0.0; last_entropy = 0.0; last_ins = 0; last_loops = 0

    while robot.step(ts) != -1:
        tick += 1; dt = ts / 1000.0
        if tick > MAX_TICKS: log(f"=== MAX_TICKS {MAX_TICKS} reached ==="); break

        roll, pitch, _yaw = imu.getRollPitchYaw()
        gx, gy, gz = gyro.getValues()
        x, y, alt = gps.getValues()
        vx = (x - px) / dt if tick > 1 else 0.0
        vy = (y - py) / dt if tick > 1 else 0.0
        px = x; py = y
        rf = rf_s.getValue() / 1000; rl = rl_s.getValue() / 1000
        rr = rr_s.getValue() / 1000; rb = rb_s.getValue() / 1000
        min_obs = min(rf, rl, rr, rb)

        phase, phase_idx = env.step(tick)
        if did == 0 and tick % PHASE_LEN_TICKS == 0 and tick >= WARMUP_TICKS:
            log(f"  >>> ENV phase={phase} idx={phase_idx} tick={tick}")

        wx, wy = wind.update(phase)

        # Send sensors to bridge
        pkt = (
            f'{{"tick":{tick},"x":{x:.4f},"y":{y:.4f},"alt":{alt:.4f},'
            f'"roll":{roll:.4f},"pitch":{pitch:.4f},"gz":{gz:.4f},'
            f'"rf":{rf:.4f},"rl":{rl:.4f},"rr":{rr:.4f},"rb":{rb:.4f},'
            f'"vx":{vx:.4f},"vy":{vy:.4f},'
            f'"imu_alive":true,"gps_alive":true,"sonar_alive":true}}\n'
        )
        try:
            bridge.stdin.write(pkt); bridge.stdin.flush()
        except Exception as e:
            log(f"!!! bridge stdin FAILED: {e}"); break

        try:
            line = bridge.stdout.readline()
            if not line: log(f"!!! bridge EOF at T{tick}"); break
            cmd = json.loads(line.strip())
            dvx_cmd = float(cmd.get("dvx", 0.0))
            dvy_cmd = float(cmd.get("dvy", 0.0))
            s_alt_cmd = float(cmd.get("s_alt", 1.30))
            aborted_cmd = bool(cmd.get("abort", False))
            last_fear = float(cmd.get("fear", 0.0))
            last_entropy = float(cmd.get("entropy", 0.0))
            last_ins = int(cmd.get("inspected", 0))
            last_loops = int(cmd.get("loops", 0))
        except Exception as e:
            log(f"!!! bridge stdout FAILED at T{tick}: {e}"); break

        # Inject wind disturbance (this is the habituation stimulus)
        dvx_final = dvx_cmd + wx
        dvy_final = dvy_cmd + wy

        # Abort path
        if aborted_cmd:
            for m in motors: m.setVelocity(0)
        else:
            # Safety net
            spd = (vx * vx + vy * vy) ** 0.5
            if spd > 2.5 or abs(alt - s_alt_cmd) > 3.0:
                dvx_final = -vx * 0.5; dvy_final = -vy * 0.5
                s_alt_cmd = max(0.4, min(2.0, alt)); pid.ai *= 0.5

            mp = pid.pid(dt, dvx_final, dvy_final, 0, s_alt_cmd, roll, pitch, gz, alt, vx, vy)
            motors[0].setVelocity(-mp[0]); motors[1].setVelocity(mp[1])
            motors[2].setVelocity(-mp[2]); motors[3].setVelocity(mp[3])

        # Per-tick metrics
        csv_file.write(f"{tick},{x:.3f},{y:.3f},{alt:.3f},{last_fear:.4f},{last_entropy:.4f},"
                       f"{dvx_cmd:.4f},{dvy_cmd:.4f},{wx:.4f},{wy:.4f},{min_obs:.3f},"
                       f"{last_ins},{last_loops},{int(aborted_cmd)},{phase},{phase_idx}\n")
        if tick % 200 == 0:
            log(f"[T{tick:6}] alt:{alt:.2f} ({x:+.2f},{y:+.2f}) f:{last_fear * 100:5.1f}% "
                f"ent:{last_entropy:.3f} ins:{last_ins} loops:{last_loops} "
                f"abort:{aborted_cmd} env:{phase}({phase_idx})")

    try:
        bridge.stdin.close()
        bridge.wait(timeout=2)
    except Exception:
        bridge.kill()
    log(f"=== END tick={tick} ===")
    csv_file.close(); log_file.close()
