"""
OASIS Recon v3 — Hazard drone.
Simple PID-controlled Crazyflie that orbits the arena during EXTREME phases.
Reads env_phase.json (written by recon d00 master).
Orbits provoke real sonar proximity events on recon drones → real M5 fear signal.
"""
import sys, os, json, math, time
from controller import Robot

SHARED = "C:/dev/oasis/webots/recon10v3_shared/"
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

def read_phase():
    try:
        with open(os.path.join(SHARED, "env_phase.json")) as f:
            d = json.load(f)
            return d.get("phase", "CALM"), d.get("idx", 0)
    except Exception:
        return "CALM", 0

if __name__ == '__main__':
    robot = Robot()
    ts = int(robot.getBasicTimeStep())
    args = sys.argv[1:] if len(sys.argv) > 1 else ["hz0", "0"]
    name = args[0]; did = int(args[1]) if len(args) > 1 else 0

    log_file = open(f"C:/dev/oasis/webots/recon10v3_{name}.log", "w", encoding="utf-8", buffering=1)
    def log(m): log_file.write(m + "\n")

    motors = []
    for mn in ["m1_motor", "m2_motor", "m3_motor", "m4_motor"]:
        m = robot.getDevice(mn); m.setPosition(float('inf'))
        m.setVelocity(-1 if mn in ["m1_motor", "m3_motor"] else 1); motors.append(m)
    imu = robot.getDevice("inertial_unit"); imu.enable(ts)
    gps = robot.getDevice("gps"); gps.enable(ts)
    gyro = robot.getDevice("gyro"); gyro.enable(ts)

    pid = PID()

    # Orbit parameters: 2 hazards, offset 180 deg, radius 2.5m, altitude 1.5m
    ORBIT_RADIUS = 2.5
    ORBIT_ALT = 1.5
    ORBIT_SPEED = 0.6   # m/s tangential
    ORBIT_W = ORBIT_SPEED / ORBIT_RADIUS  # rad/s
    phase_offset = did * math.pi  # hz0 at 0°, hz1 at 180°

    # Hover parameters during CALM
    HOVER_X = 0.0 if did == 0 else 0.0
    HOVER_Y = 4.2 if did == 0 else -4.2   # park at poles during CALM

    px = 0.0; py = 0.0; tick = 0
    s_alt = ORBIT_ALT
    log(f"=== HAZARD {name} orbit r={ORBIT_RADIUS} alt={ORBIT_ALT} start_phase={phase_offset:.2f}rad ===")

    while robot.step(ts) != -1:
        tick += 1; dt = ts / 1000.0
        roll, pitch, _yaw = imu.getRollPitchYaw()
        gx, gy, gz = gyro.getValues()
        x, y, alt = gps.getValues()
        vx = (x - px) / dt if tick > 1 else 0.0
        vy = (y - py) / dt if tick > 1 else 0.0
        px = x; py = y

        phase, phase_idx = read_phase()

        # EXTREME: orbit around origin; CALM: park at pole
        if phase == "EXTREME":
            theta = ORBIT_W * tick * dt + phase_offset
            t_x = ORBIT_RADIUS * math.cos(theta)
            t_y = ORBIT_RADIUS * math.sin(theta)
            t_alt = ORBIT_ALT
            spd_cap = 0.8
        else:
            t_x = HOVER_X; t_y = HOVER_Y; t_alt = ORBIT_ALT
            spd_cap = 0.4

        s_alt += (t_alt - s_alt) * 0.2
        ae = alt - s_alt

        # Proportional navigation toward target
        dx = t_x - x; dy = t_y - y
        dist = math.sqrt(dx*dx + dy*dy)
        if dist > 0.05:
            sp = min(spd_cap, dist * 0.8)
            dvx = dx / dist * sp; dvy = dy / dist * sp
        else:
            dvx = 0; dvy = 0

        # Safety net
        spd_t = math.sqrt(vx*vx + vy*vy)
        if spd_t > 2.0 or abs(ae) > 2.0:
            dvx = -vx * 0.5; dvy = -vy * 0.5
            s_alt = max(0.5, min(2.0, alt)); pid.ai *= 0.5

        if alt > 4.5 or abs(x) > 5.5 or abs(y) > 4.5:
            log(f"!!! T{tick} HAZARD LOST alt={alt:.1f} pos=({x:.1f},{y:.1f})")
            for m in motors: m.setVelocity(0)
            continue

        mp = pid.pid(dt, dvx, dvy, 0, s_alt, roll, pitch, gz, alt, vx, vy)
        motors[0].setVelocity(-mp[0]); motors[1].setVelocity(mp[1])
        motors[2].setVelocity(-mp[2]); motors[3].setVelocity(mp[3])

        if tick % 500 == 0:
            log(f"[T{tick:6}] {name} alt={alt:.2f} pos=({x:+.2f},{y:+.2f}) phase={phase}({phase_idx})")

    log_file.close()
