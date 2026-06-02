"""
OASIS Swarm Controller — Federated Synaptic Resonance (Claim 11)
3 drones share experience via file-based federation (like spore.rs).

Drone Alpha: flies toward red pillar, LEARNS danger, BROADCASTS fear.
Drone Beta: starts far from red pillar, RECEIVES Alpha's fear, AVOIDS without experiencing.
Drone Gamma: independent path, learns from both Alpha and Beta.

This proves: swarm learning without a central server.
"""

import sys, os, json, math, time
from controller import Robot

# ─── Bitcraze PID (cerebellum) ───
class PID:
    def __init__(self):
        self.pvxe=0; self.pvye=0; self.pae=0; self.ppe=0; self.pre=0; self.ai=0
    def pid(self, dt, dvx, dvy, dyaw, dalt, roll, pitch, yr, alt, vx, vy):
        g = {"kp_att_y":1,"kd_att_y":0.5,"kp_att_rp":0.5,"kd_att_rp":0.1,
             "kp_vel_xy":2,"kd_vel_xy":0.5,"kp_z":10,"ki_z":5,"kd_z":5}
        vxe=dvx-vx; vxd=(vxe-self.pvxe)/dt; vye=dvy-vy; vyd=(vye-self.pvye)/dt
        dp=g["kp_vel_xy"]*max(-1,min(1,vxe))+g["kd_vel_xy"]*vxd
        dr=-g["kp_vel_xy"]*max(-1,min(1,vye))-g["kd_vel_xy"]*vyd
        self.pvxe=vxe; self.pvye=vye
        ae=dalt-alt; ad=(ae-self.pae)/dt; self.ai+=ae*dt
        ac=g["kp_z"]*ae+g["kd_z"]*ad+g["ki_z"]*max(-2,min(2,self.ai))+48; self.pae=ae
        pe=dp-pitch; pd=(pe-self.ppe)/dt; re=dr-roll; rd=(re-self.pre)/dt; ye=dyaw-yr
        rc=g["kp_att_rp"]*max(-1,min(1,re))+g["kd_att_rp"]*rd
        pc=-g["kp_att_rp"]*max(-1,min(1,pe))-g["kd_att_rp"]*pd
        yc=g["kp_att_y"]*max(-1,min(1,ye))
        self.ppe=pe; self.pre=re
        return [max(0,min(600,ac-rc+pc+yc)),max(0,min(600,ac-rc-pc-yc)),
                max(0,min(600,ac+rc-pc+yc)),max(0,min(600,ac+rc+pc-yc))]

# ─── Federation: share experience via files ───
SHARED_DIR = "C:/dev/oasis/webots/swarm_shared/"
os.makedirs(SHARED_DIR, exist_ok=True)

def broadcast_experience(name, fear_zones):
    """Write my learned fear zones to shared file"""
    path = os.path.join(SHARED_DIR, f"{name}_digest.json")
    with open(path, "w") as f:
        json.dump({"source": name, "time": time.time(), "fears": fear_zones}, f)

def receive_experience(my_name):
    """Read other drones' fear zones"""
    fears = []
    for fname in os.listdir(SHARED_DIR):
        if fname.endswith("_digest.json") and not fname.startswith(my_name):
            try:
                with open(os.path.join(SHARED_DIR, fname)) as f:
                    data = json.load(f)
                    fears.extend(data.get("fears", []))
            except: pass
    return fears

# ─── Main ───
if __name__ == '__main__':
    robot = Robot()
    timestep = int(robot.getBasicTimeStep())
    args = sys.argv[1:] if len(sys.argv) > 1 else ["alpha", "0"]
    my_name = args[0]
    my_id = int(args[1]) if len(args) > 1 else 0

    log_path = f"C:/dev/oasis/webots/swarm_{my_name}.log"
    log_file = open(log_path, "w", encoding="utf-8")
    def log(msg):
        print(f"[{my_name}] {msg}"); log_file.write(msg + "\n"); log_file.flush()

    motors = []
    for name in ["m1_motor", "m2_motor", "m3_motor", "m4_motor"]:
        m = robot.getDevice(name); m.setPosition(float('inf'))
        m.setVelocity(-1 if name in ["m1_motor", "m3_motor"] else 1)
        motors.append(m)

    imu = robot.getDevice("inertial_unit"); imu.enable(timestep)
    gps = robot.getDevice("gps"); gps.enable(timestep)
    gyro = robot.getDevice("gyro"); gyro.enable(timestep)
    range_f = robot.getDevice("range_front"); range_f.enable(timestep)
    range_l = robot.getDevice("range_left"); range_l.enable(timestep)
    range_r = robot.getDevice("range_right"); range_r.enable(timestep)
    range_b = robot.getDevice("range_back"); range_b.enable(timestep)

    pid = PID()

    # Different flight plans per drone
    if my_name == "alpha":
        # Alpha: SPEED RUN — fast slalom toward and around red pillar
        WAYPOINTS = [
            (0,    1.0,  0.0,  0.0, "TAKEOFF"),
            (200,  1.0,  0.0,  0.0, "STABLE"),
            (350,  1.0,  0.45, 0.0, "RUSH-RED"),      # fast approach red pillar
            (500,  1.0,  0.0,  0.3, "DODGE-N"),        # dodge north
            (650,  1.0, -0.3,  0.0, "CROSS-W"),        # cross west
            (800,  1.0,  0.0, -0.3, "DODGE-S"),        # dodge south
            (950,  1.0,  0.45, 0.0, "RUSH-RED2"),      # rush red again
            (1100, 1.0,  0.0,  0.0, "CENTER"),
            (1250, 1.5,  0.3,  0.3, "3D-CLIMB-NE"),    # 3D maneuver
            (1400, 0.5, -0.3, -0.3, "3D-DIVE-SW"),
            (1550, 1.0,  0.0,  0.0, "HOME"),
            (1700, 1.0,  0.0,  0.0, "HOVER"),
        ]
    elif my_name == "beta":
        # Beta: SPEED RUN — delayed, receives Alpha's experience first
        WAYPOINTS = [
            (0,    1.0, -0.8,  0.0, "TAKEOFF"),
            (200,  1.0, -0.8,  0.0, "STABLE"),
            (400,  1.0, -0.3,  0.0, "MOVE-IN"),
            (600,  1.0,  0.0,  0.0, "CENTER"),
            (800,  1.0,  0.45, 0.0, "RUSH-RED"),       # same rush — but with Alpha's warning
            (950,  1.0,  0.0,  0.3, "DODGE-N"),
            (1100, 1.0, -0.4,  0.4, "EXPLORE-NW"),
            (1300, 1.0,  0.0,  0.0, "CENTER2"),
            (1500, 1.0,  0.45, 0.0, "RUSH-RED2"),      # second rush with max knowledge
            (1700, 1.0,  0.0,  0.0, "HOME"),
            (1900, 1.0,  0.0,  0.0, "HOVER"),
        ]
    else:
        # Gamma: SPEED RUN — fastest, learns from both Alpha and Beta
        WAYPOINTS = [
            (0,    1.0,  0.0, -0.8, "TAKEOFF"),
            (200,  1.0,  0.0, -0.4, "MOVE-IN"),
            (400,  1.0,  0.0,  0.0, "CENTER"),
            (550,  1.0,  0.3,  0.3, "NE-FAST"),
            (700,  1.0, -0.3,  0.3, "NW-FAST"),
            (850,  1.0, -0.3, -0.3, "SW-FAST"),
            (1000, 1.0,  0.3, -0.3, "SE-FAST"),
            (1150, 1.0,  0.0,  0.0, "CENTER2"),
            (1300, 1.0,  0.45, 0.0, "RUSH-RED"),       # rush with full swarm knowledge
            (1500, 1.0,  0.0,  0.0, "HOME"),
            (1700, 1.0,  0.0,  0.0, "HOVER"),
        ]

    smooth_alt = 1.0; smooth_x = WAYPOINTS[0][2]; smooth_y = WAYPOINTS[0][3]
    wp_idx = 0; tick = 0; past_x = WAYPOINTS[0][2]; past_y = WAYPOINTS[0][3]
    fear = 0.0; fear_zones = []  # [(x, y, intensity)]
    foreign_fears = []
    syn_obs_weight = 0.0

    log(f"=== OASIS SWARM: {my_name} (id={my_id}) ===")
    log(f"  Federation via {SHARED_DIR}")

    while robot.step(timestep) != -1:
        tick += 1
        dt = timestep / 1000.0

        roll, pitch, yaw = imu.getRollPitchYaw()
        gx, gy, gz = gyro.getValues()
        x, y, alt = gps.getValues()
        vx = (x - past_x) / dt if tick > 1 else 0.0
        vy = (y - past_y) / dt if tick > 1 else 0.0
        past_x = x; past_y = y
        rf = range_f.getValue()/1000; rl = range_l.getValue()/1000
        rr = range_r.getValue()/1000; rb = range_b.getValue()/1000
        obs_near = min(rf, rl, rr, rb)

        # Waypoint progression
        target_alt = WAYPOINTS[wp_idx][1]
        while wp_idx < len(WAYPOINTS) - 1 and tick >= WAYPOINTS[wp_idx + 1][0]:
            wp_idx += 1
            _, target_alt, tx, ty, wname = WAYPOINTS[wp_idx]
            log(f"  >>> WP{wp_idx}: {wname} alt={target_alt} pos=({tx},{ty})")
        _, _, target_x, target_y, wp_name = WAYPOINTS[wp_idx]

        smooth_alt += (target_alt - smooth_alt) * 0.2
        smooth_x += (target_x - smooth_x) * 0.35  # faster interp for speed test
        smooth_y += (target_y - smooth_y) * 0.35

        alt_error = alt - smooth_alt
        alt_stable = abs(alt_error) < 0.2

        # ─── LOCAL FEAR: from my own range sensors ───
        local_fear = 0.0
        if obs_near < 0.25:
            local_fear = (0.25 - obs_near) * 4.0
            # Record fear zone
            if local_fear > 0.5:
                fear_zones = [(x, y, local_fear)] + fear_zones[:9]  # keep last 10

        # ─── FOREIGN FEAR: from other drones' experience ───
        foreign_fear = 0.0
        if tick % 50 == 0:
            foreign_fears = receive_experience(my_name)
        for fz in foreign_fears:
            dist = math.sqrt((x - fz[0])**2 + (y - fz[1])**2)
            if dist < 0.5:  # within 50cm of a feared zone
                foreign_fear = max(foreign_fear, fz[2] * (0.5 - dist) * 2.0)

        # Total fear: own experience + swarm knowledge
        fear_input = max(local_fear, foreign_fear * 0.7)  # trust foreign at 70%
        fear = fear * 0.85 + fear_input * 0.15
        fear = min(5.0, fear)

        # ─── NAVIGATION ───
        desired_vx = 0.0; desired_vy = 0.0
        if alt_stable:
            dx = smooth_x - x; dy = smooth_y - y
            dist = math.sqrt(dx*dx + dy*dy)
            if dist > 0.02:
                spd = min(0.35, dist * 0.7)  # faster for speed test
                # Fear slows down (own + foreign)
                if fear > 0.5:
                    spd *= max(0.3, 1.0 - fear * 0.05)
                desired_vx = (dx/dist) * spd
                desired_vy = (dy/dist) * spd

        # Reactive avoidance
        if rf < 0.25: desired_vx -= (0.25-rf)*0.5
        if rb < 0.25: desired_vx += (0.25-rb)*0.5
        if rl < 0.25: desired_vy -= (0.25-rl)*0.5
        if rr < 0.25: desired_vy += (0.25-rr)*0.5
        desired_vx = max(-0.25, min(0.25, desired_vx))
        desired_vy = max(-0.25, min(0.25, desired_vy))

        # Brake if very close
        if obs_near < 0.12:
            desired_vx = 0.0; desired_vy = 0.0

        # Graceful degradation
        speed = math.sqrt(vx*vx + vy*vy)
        if speed > 1.5 or abs(alt - smooth_alt) > 2.0:
            desired_vx = -vx*0.5; desired_vy = -vy*0.5
            smooth_alt = max(0.3, min(2.0, alt))
            smooth_x = x; smooth_y = y
            pid.ai *= 0.5

        # PID
        mp = pid.pid(dt, desired_vx, desired_vy, 0, smooth_alt, roll, pitch, gz, alt, vx, vy)
        motors[0].setVelocity(-mp[0]); motors[1].setVelocity(mp[1])
        motors[2].setVelocity(-mp[2]); motors[3].setVelocity(mp[3])

        # ─── FEDERATION: broadcast every 100 ticks ───
        if tick % 100 == 0 and len(fear_zones) > 0:
            broadcast_experience(my_name, fear_zones)

        # ─── TELEMETRY ───
        if tick % 10 == 0:
            ff = f"FF:{foreign_fear:.1f}" if foreign_fear > 0.01 else ""
            log(f"[T{tick:5}] alt:{alt:.2f} x:{x:.2f} y:{y:.2f} | fear:{fear*100:.0f}% obs:{obs_near:.2f} "
                f"| fz:{len(fear_zones)} {ff} | {wp_name}")
