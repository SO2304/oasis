"""
OASIS Recon-20 — 20 drones, reconnaissance + RTH, alternating extreme/calm wind.

Minimal delta from oasis_swarm_xl (proven at 5 drones):
  - 20 drone home positions (5x4 grid)
  - Mission FSM: TAKEOFF -> SCOUT -> RTH -> LANDED (distance-gated)
  - EnvironmentSequencer (drone d00 master) writes env_phase.json
  - Wind modulated by phase (still gentle: +/-0.05 EXTREME, +/-0.01 CALM)
  - Per-tick CSV log with fear + wind_comp_mag for habituation analysis

Virtual distance scale: 1 actual m = 10 virtual m (documented assumption).
Swarm target: 50 km virtual => 2500 m virtual / drone => 250 m actual / drone.
"""
import sys, os, json, math, time, random
from controller import Robot

DISTANCE_SCALE = 10.0
SWARM_TARGET_VIRT_KM = 50.0
N_DRONES = 20
PER_DRONE_VIRT_M = (SWARM_TARGET_VIRT_KM * 1000.0) / N_DRONES
PER_DRONE_ACTUAL_M = PER_DRONE_VIRT_M / DISTANCE_SCALE

TICKS_PER_SECOND = int(1000 / 32)
PHASE_LEN_TICKS = 25 * TICKS_PER_SECOND
WARMUP_TICKS = 60 * TICKS_PER_SECOND
MAX_TICKS = 60 * 60 * TICKS_PER_SECOND

SHARED = "C:/dev/oasis/webots/recon20_shared/"
os.makedirs(SHARED, exist_ok=True)

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

def broadcast(name, fears, wind_comp):
    try:
        with open(os.path.join(SHARED, f"{name}.json"), "w") as f:
            json.dump({"src": name, "t": time.time(), "fears": fears, "wind": wind_comp}, f)
    except Exception: pass

def receive(my_name):
    fears, winds = [], []
    try:
        for fn in os.listdir(SHARED):
            if fn.endswith(".json") and fn != "env_phase.json" and not fn.startswith(my_name):
                try:
                    with open(os.path.join(SHARED, fn)) as f:
                        d = json.load(f)
                        fears.extend(d.get("fears", [])); winds.append(d.get("wind", [0, 0]))
                except Exception: pass
    except Exception: pass
    return fears, winds

class EnvSequencer:
    def __init__(self, is_master, seed):
        self.is_master = is_master
        self.rng = random.Random(seed)
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
    """Gentle even in EXTREME — 20-drone Webots physics cannot handle stronger."""
    def __init__(self, seed):
        self.rng = random.Random(seed)
        self.wx = 0.0; self.wy = 0.0; self.gust_timer = 0
    def update(self, phase):
        self.gust_timer -= 1
        if self.gust_timer <= 0:
            if phase == "EXTREME":
                self.wx = self.rng.uniform(-0.05, 0.05)
                self.wy = self.rng.uniform(-0.05, 0.05)
                self.gust_timer = self.rng.randint(40, 100)
            else:
                self.wx = self.rng.uniform(-0.01, 0.01)
                self.wy = self.rng.uniform(-0.01, 0.01)
                self.gust_timer = self.rng.randint(200, 400)
        return self.wx, self.wy

class Mission:
    def __init__(self, home_x, home_y, target_actual_m, did):
        self.home_x = home_x; self.home_y = home_y
        self.target = target_actual_m
        self.state = "TAKEOFF"
        self.distance_actual = 0.0
        self.last_x = home_x; self.last_y = home_y
        self.rng = random.Random(did * 17 + 3)
        self.scout_target = self._next_scout(home_x, home_y)
        self.landed_tick = -1
    def _next_scout(self, x, y):
        for _ in range(8):
            ang = self.rng.uniform(0, 2 * math.pi)
            r = self.rng.uniform(0.8, 1.8)
            tx = x + r * math.cos(ang); ty = y + r * math.sin(ang)
            if -4.5 <= tx <= 4.5 and -4.5 <= ty <= 4.5:
                return (tx, ty)
        return (self.home_x, self.home_y)
    def step(self, x, y, alt, tick):
        if tick > 1:
            d = math.sqrt((x - self.last_x) ** 2 + (y - self.last_y) ** 2)
            if d < 1.0:
                self.distance_actual += d
        self.last_x = x; self.last_y = y

        if self.state == "TAKEOFF":
            if alt > 0.85: self.state = "SCOUT"
            return (self.home_x, self.home_y, 1.0)
        if self.state == "SCOUT":
            if self.distance_actual >= self.target:
                self.state = "RTH"
                return (self.home_x, self.home_y, 1.0)
            tx, ty = self.scout_target
            if math.sqrt((x - tx) ** 2 + (y - ty) ** 2) < 0.3:
                self.scout_target = self._next_scout(x, y)
            return (self.scout_target[0], self.scout_target[1], 1.0)
        if self.state == "RTH":
            if math.sqrt((x - self.home_x) ** 2 + (y - self.home_y) ** 2) < 0.3:
                self.state = "LANDING"
            return (self.home_x, self.home_y, 1.0)
        if self.state == "LANDING":
            if alt < 0.15:
                self.state = "LANDED"; self.landed_tick = tick
            return (self.home_x, self.home_y, 0.05)
        return (self.home_x, self.home_y, 0.05)

if __name__ == '__main__':
    robot = Robot()
    ts = int(robot.getBasicTimeStep())
    args = sys.argv[1:] if len(sys.argv) > 1 else ["d00", "0"]
    name = args[0]; did = int(args[1]) if len(args) > 1 else 0

    log_path = f"C:/dev/oasis/webots/recon20_{name}.log"
    csv_path = f"C:/dev/oasis/webots/recon20_metrics_{name}.csv"
    log_file = open(log_path, "w", encoding="utf-8")
    csv_file = open(csv_path, "w", encoding="utf-8")
    csv_file.write("tick,x,y,alt,fear,wind_x,wind_y,wind_comp_mag,dist_actual,dist_virt,mission,env_phase,phase_idx\n")
    def log(m):
        print(f"[{name}] {m}", flush=True)
        log_file.write(m + "\n"); log_file.flush()

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

    col = did % 5; row = did // 5
    home_x = -3.2 + col * 1.6; home_y = -3.6 + row * 2.4

    pid = PID()
    wind = Wind(did * 42 + 7)
    env = EnvSequencer(is_master=(did == 0), seed=12345)
    mission = Mission(home_x, home_y, PER_DRONE_ACTUAL_M, did)

    s_alt = 1.0; s_x = home_x; s_y = home_y
    px = home_x; py = home_y
    fear = 0.0; fear_zones = []
    foreign_fears, foreign_winds = [], []
    wind_comp = [0.0, 0.0]
    tick = 0

    log(f"=== OASIS RECON-20: {name} (id={did}, home=({home_x:.1f},{home_y:.1f})) ===")
    log(f"  per-drone target: {PER_DRONE_ACTUAL_M:.0f} m actual ({PER_DRONE_VIRT_M:.0f} m virt @ x{DISTANCE_SCALE})")
    log(f"  master={'YES' if did == 0 else 'no'} warmup={WARMUP_TICKS}t phase_len={PHASE_LEN_TICKS}t")

    while robot.step(ts) != -1:
        tick += 1; dt = ts / 1000.0
        if tick > MAX_TICKS: log("!!! MAX_TICKS reached"); break

        roll, pitch, yaw = imu.getRollPitchYaw()
        gx, gy, gz = gyro.getValues()
        x, y, alt = gps.getValues()
        vx = (x - px) / dt if tick > 1 else 0.0
        vy = (y - py) / dt if tick > 1 else 0.0
        px = x; py = y
        rf = rf_s.getValue() / 1000; rl = rl_s.getValue() / 1000
        rr = rr_s.getValue() / 1000; rb = rb_s.getValue() / 1000
        obs = min(rf, rl, rr, rb)

        phase, phase_idx = env.step(tick)
        if did == 0 and tick % PHASE_LEN_TICKS == 0 and tick >= WARMUP_TICKS:
            log(f"  >>> ENV phase={phase} idx={phase_idx} tick={tick}")

        wx, wy = wind.update(phase)

        t_x, t_y, t_alt = mission.step(x, y, alt, tick)
        s_alt += (t_alt - s_alt) * 0.2; s_x += (t_x - s_x) * 0.3; s_y += (t_y - s_y) * 0.3
        ae = alt - s_alt; stable = abs(ae) < 0.2

        lf = 0.0
        if obs < 0.25: lf = (0.25 - obs) * 4.0
        wind_mag = math.sqrt(wx * wx + wy * wy)
        if wind_mag > 0.03:
            lf = max(lf, (wind_mag - 0.03) * 12.0)
        if lf > 0.3: fear_zones = [(x, y, lf)] + fear_zones[:19]

        ff = 0.0
        if tick % 30 == 0: foreign_fears, foreign_winds = receive(name)
        for fz in foreign_fears:
            d = math.sqrt((x - fz[0]) ** 2 + (y - fz[1]) ** 2)
            if d < 0.6: ff = max(ff, fz[2] * (0.6 - d) * 2.0)

        if foreign_winds:
            for fw in foreign_winds:
                wind_comp[0] = wind_comp[0] * 0.99 + fw[0] * 0.01
                wind_comp[1] = wind_comp[1] * 0.99 + fw[1] * 0.01
        # Hard cap on wind_comp (prevents runaway from flyaway velocities)
        wcm = math.sqrt(wind_comp[0] ** 2 + wind_comp[1] ** 2)
        if wcm > 0.05:
            wind_comp[0] *= 0.05 / wcm; wind_comp[1] *= 0.05 / wcm

        # Fear: pure decay + new stimulus, no hardcoded habituation
        fear = min(5.0, fear * 0.85 + max(lf, ff * 0.7) * 0.15)

        dvx = 0.0; dvy = 0.0
        if stable:
            dx = s_x - x; dy = s_y - y
            dist = math.sqrt(dx * dx + dy * dy)
            if dist > 0.02:
                spd = min(0.3, dist * 0.6)
                if fear > 1.0: spd *= max(0.3, 1.0 - fear * 0.05)
                dvx = (dx / dist) * spd; dvy = (dy / dist) * spd
        dvx += wx + wind_comp[0] * 0.5
        dvy += wy + wind_comp[1] * 0.5
        # Slow self-learning, bounded vx
        if 0.05 < abs(vx) < 0.8:
            wind_comp[0] = wind_comp[0] * 0.999 - vx * 0.001
        if 0.05 < abs(vy) < 0.8:
            wind_comp[1] = wind_comp[1] * 0.999 - vy * 0.001

        if rf < 0.25: dvx -= (0.25 - rf) * 0.5
        if rb < 0.25: dvx += (0.25 - rb) * 0.5
        if rl < 0.25: dvy -= (0.25 - rl) * 0.5
        if rr < 0.25: dvy += (0.25 - rr) * 0.5
        dvx = max(-0.3, min(0.3, dvx)); dvy = max(-0.3, min(0.3, dvy))
        if obs < 0.12: dvx = 0; dvy = 0

        spd_t = math.sqrt(vx * vx + vy * vy)
        if spd_t > 1.5 or abs(ae) > 2.0:
            dvx = -vx * 0.5; dvy = -vy * 0.5
            s_alt = max(0.3, min(2.0, alt))
            s_x = x; s_y = y; pid.ai *= 0.5

        # Hard emergency: drone gone from physics domain → permanent motor shutdown
        if alt > 4.0 or abs(x) > 7.0 or abs(y) > 7.0:
            if mission.state != "LOST":
                log(f"!!! T{tick} EMERGENCY LOST: alt={alt:.1f} pos=({x:.1f},{y:.1f})")
                mission.state = "LOST"
        if mission.state in ("LANDED", "LOST"):
            for m in motors: m.setVelocity(0)
        else:
            mp = pid.pid(dt, dvx, dvy, 0, s_alt, roll, pitch, gz, alt, vx, vy)
            motors[0].setVelocity(-mp[0]); motors[1].setVelocity(mp[1])
            motors[2].setVelocity(-mp[2]); motors[3].setVelocity(mp[3])

        if tick % 50 == 0 and (len(fear_zones) > 0 or abs(wind_comp[0]) > 0.001):
            broadcast(name, fear_zones, wind_comp)

        dist_virt = mission.distance_actual * DISTANCE_SCALE
        csv_file.write(f"{tick},{x:.3f},{y:.3f},{alt:.3f},{fear:.4f},"
                       f"{wx:.4f},{wy:.4f},{wcm:.4f},"
                       f"{mission.distance_actual:.2f},{dist_virt:.2f},"
                       f"{mission.state},{phase},{phase_idx}\n")
        if tick % 500 == 0: csv_file.flush()

        if tick % 200 == 0:
            log(f"[T{tick:6}] {mission.state:7} alt:{alt:.2f} ({x:+.2f},{y:+.2f}) "
                f"f:{fear * 100:5.1f}% wc:{wcm:.3f} "
                f"d:{mission.distance_actual:6.1f}m | env:{phase}({phase_idx})")

        if mission.state == "LANDED" and tick > mission.landed_tick + 10 * TICKS_PER_SECOND:
            log(f"=== MISSION COMPLETE: landed T{mission.landed_tick} d={mission.distance_actual:.1f}m "
                f"({mission.distance_actual * DISTANCE_SCALE / 1000:.2f}km virt) phases={phase_idx} ===")
            break

    log_file.close(); csv_file.close()
