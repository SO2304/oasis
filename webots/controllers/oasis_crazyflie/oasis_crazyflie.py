"""
OASIS Hybrid Controller: Bitcraze PID (cerebellum) + OASIS (cortex)

Architecture biologique:
  - CEREBELLUM (PID): stabilisation, hover, attitude — PROVEN by Bitcraze
  - CORTEX (OASIS): emotions, learning, reflexes, navigation decisions

The PID flies. OASIS decides WHERE to fly and HOW to react.
OASIS never touches motor commands directly — it modulates the PID's setpoints.
"""

import sys, os
try:
    _dbg = open("C:/dev/oasis/webots/oasis_debug.log", "w")
    _dbg.write("OASIS hybrid controller starting...\n"); _dbg.flush()
except: pass

from controller import Robot
import math, time
def np_clip(v, lo, hi): return max(lo, min(hi, v))

# ═══ BITCRAZE PID — CEREBELLUM (untouched, proven) ═══

class CerebellumPID:
    """Exact copy of Bitcraze pid_controller.py. DO NOT MODIFY."""
    def __init__(self):
        self.past_vx_error = 0.0
        self.past_vy_error = 0.0
        self.past_alt_error = 0.0
        self.past_pitch_error = 0.0
        self.past_roll_error = 0.0
        self.altitude_integrator = 0.0
        self.gains = {"kp_att_y": 1, "kd_att_y": 0.5, "kp_att_rp": 0.5, "kd_att_rp": 0.1,
                      "kp_vel_xy": 2, "kd_vel_xy": 0.5, "kp_z": 10, "ki_z": 5, "kd_z": 5}

    def pid(self, dt, desired_vx, desired_vy, desired_yaw_rate, desired_altitude,
            actual_roll, actual_pitch, actual_yaw_rate, actual_altitude, actual_vx, actual_vy):
        g = self.gains
        vx_error = desired_vx - actual_vx
        vx_deriv = (vx_error - self.past_vx_error) / dt
        vy_error = desired_vy - actual_vy
        vy_deriv = (vy_error - self.past_vy_error) / dt
        desired_pitch = g["kp_vel_xy"] * np_clip(vx_error, -1, 1) + g["kd_vel_xy"] * vx_deriv
        desired_roll = -g["kp_vel_xy"] * np_clip(vy_error, -1, 1) - g["kd_vel_xy"] * vy_deriv
        self.past_vx_error = vx_error
        self.past_vy_error = vy_error
        alt_error = desired_altitude - actual_altitude
        alt_deriv = (alt_error - self.past_alt_error) / dt
        self.altitude_integrator += alt_error * dt
        alt_command = g["kp_z"]*alt_error + g["kd_z"]*alt_deriv + g["ki_z"]*np_clip(self.altitude_integrator,-2,2) + 48
        self.past_alt_error = alt_error
        pitch_error = desired_pitch - actual_pitch
        pitch_deriv = (pitch_error - self.past_pitch_error) / dt
        roll_error = desired_roll - actual_roll
        roll_deriv = (roll_error - self.past_roll_error) / dt
        yaw_rate_error = desired_yaw_rate - actual_yaw_rate
        roll_command = g["kp_att_rp"]*np_clip(roll_error,-1,1) + g["kd_att_rp"]*roll_deriv
        pitch_command = -g["kp_att_rp"]*np_clip(pitch_error,-1,1) - g["kd_att_rp"]*pitch_deriv
        yaw_command = g["kp_att_y"]*np_clip(yaw_rate_error,-1,1)
        self.past_pitch_error = pitch_error
        self.past_roll_error = roll_error
        m1 = np_clip(alt_command - roll_command + pitch_command + yaw_command, 0, 600)
        m2 = np_clip(alt_command - roll_command - pitch_command - yaw_command, 0, 600)
        m3 = np_clip(alt_command + roll_command - pitch_command + yaw_command, 0, 600)
        m4 = np_clip(alt_command + roll_command + pitch_command - yaw_command, 0, 600)
        return [m1, m2, m3, m4]

# ═══ OASIS CORTEX — emotions, synapses, reflexes ═══

class Emotion:
    def __init__(self):
        self.fear = 0.0
        self.curiosity = 1.0
        self.satisfaction = 0.0
        self.frustration = 0.0
        self.urgency = 1.0
        self.prev_pos_err = 1.0  # track progress for satisfaction
        self.prev_alt_err = 1.0
        self.wp_arrival_tick = 0  # when we first reached this waypoint
    def update(self, alt, vz, alt_err, obs_near, pos_err, tick):
        ground_fear = max(0, 0.3 - alt) * max(0, -vz) * 4.0
        descent_fear = max(0, -vz - 0.5) * 2.0
        self.fear = min(5.0, self.fear * 0.85 + max(ground_fear, descent_fear, obs_near) * 0.15)
        self.curiosity = max(0.1, 1.0 - self.fear * 0.2)
        # SATISFACTION: measures OASIS's contribution, not PID convergence
        # Am I making PROGRESS? (errors shrinking = good decisions)
        total_err = abs(alt_err) + pos_err
        prev_total = abs(self.prev_alt_err) + self.prev_pos_err
        improving = prev_total - total_err  # positive = getting better
        self.satisfaction = max(0.0, min(1.0, self.satisfaction * 0.9 + improving * 3.0))
        self.prev_pos_err = pos_err; self.prev_alt_err = alt_err
        # Track arrival
        if total_err < 0.05 and self.wp_arrival_tick == 0:
            self.wp_arrival_tick = tick
        # Frustration: not improving for long
        if total_err > 0.3 and improving < 0.001:
            self.frustration = min(1.0, self.frustration + 0.005)
        else:
            self.frustration *= 0.9
        self.urgency = min(2.0, 1.0 + total_err * 0.5)
    def new_waypoint(self):
        self.wp_arrival_tick = 0
        self.satisfaction *= 0.3  # partial reset on new target
    def dominant(self):
        vals = [("FEAR", self.fear), ("CUR", self.curiosity), ("SAT", self.satisfaction),
                ("FRUS", self.frustration), ("URG", self.urgency)]
        return max(vals, key=lambda x: x[1])[0]

class Synapse:
    def __init__(self, name):
        self.name = name
        self.weight = 0.0
        self.active = False
        self.n = 0
    def update(self, pre, post):
        self.n += 1
        co = pre * post
        if not self.active and co > 0.05 and self.n > 15:  # low threshold: learn fast
            self.weight = co * 0.15
            self.active = True
        elif self.active:
            self.weight += co * 0.01
            self.weight *= 0.999
            self.weight = max(-1.0, min(1.0, self.weight))
            if abs(self.weight) < 0.01 and self.n > 200:
                self.active = False; self.weight = 0.0

class Reflex:
    def __init__(self, sigma=3.0):
        self.sigma = sigma; self.mean = 0.0; self.std = 0.001
        self.calibrated = False; self.buf = []
    def feed(self, val):
        if len(self.buf) < 64: self.buf.append(val)
    def calibrate(self):
        if len(self.buf) < 3: return
        self.mean = sum(self.buf)/len(self.buf)
        self.std = max(0.001, (sum((x-self.mean)**2 for x in self.buf)/len(self.buf))**0.5)
        self.calibrated = True
    def check(self, val):
        return self.calibrated and val > self.mean + self.sigma * self.std

class Efference:
    def __init__(self):
        self.pred_alt = 0.0; self.pain = 0.0
    def predict(self, t): self.pred_alt = t
    def reflect(self, actual):
        err = abs(actual - self.pred_alt)
        sev = "NOM" if err < 0.05 else "RES" if err < 0.15 else "ANO" if err < 0.4 else "DYS"
        self.pain = self.pain * 0.9 + err
        return sev

# ═══ MAIN ═══

if __name__ == '__main__':
    robot = Robot()
    timestep = int(robot.getBasicTimeStep())
    log_file = open("C:/dev/oasis/webots/oasis_flight.log", "w", encoding="utf-8")
    def log(msg):
        print(msg); log_file.write(msg + "\n"); log_file.flush()

    # Motors
    motors = []
    for name in ["m1_motor", "m2_motor", "m3_motor", "m4_motor"]:
        m = robot.getDevice(name); m.setPosition(float('inf'))
        m.setVelocity(-1 if name in ["m1_motor", "m3_motor"] else 1)
        motors.append(m)

    # Sensors
    imu = robot.getDevice("inertial_unit"); imu.enable(timestep)
    gps = robot.getDevice("gps"); gps.enable(timestep)
    gyro = robot.getDevice("gyro"); gyro.enable(timestep)
    range_f = robot.getDevice("range_front"); range_f.enable(timestep)
    range_l = robot.getDevice("range_left"); range_l.enable(timestep)
    range_r = robot.getDevice("range_right"); range_r.enable(timestep)
    range_b = robot.getDevice("range_back"); range_b.enable(timestep)

    # CEREBELLUM — Bitcraze PID (stable, proven, untouched)
    pid = CerebellumPID()

    # CORTEX — OASIS nervous system
    emo = Emotion()
    eff = Efference()
    reflex_vz = Reflex(3.0)
    reflex_obs = Reflex(3.0)

    # Synapses: OASIS learns HIGH-LEVEL behavior, not motor control
    syn_obstacle = Synapse("obs_avoid")    # obstacle proximity → slow down
    syn_altitude = Synapse("alt_fear")     # low altitude + descent → caution
    syn_wind = Synapse("wind_comp")        # lateral drift → compensate
    syn_explore = Synapse("explore")       # low fear + time → increase speed

    ALL_SYN = [syn_obstacle, syn_altitude, syn_wind, syn_explore]

    # Waypoints: OASIS decides WHERE to go, PID decides HOW
    # REAL obstacle course: 4 pillars (2m tall, 8cm radius) at ±0.4m
    # Walls at ±0.8m. Drone MUST fly between obstacles. Range sensors will detect them.
    # Waypoints deliberately aim NEAR obstacles — OASIS must detect and dodge.
    WAYPOINTS = [
        (0,    1.0,  0.0,  0.0, "TAKEOFF"),
        (400,  1.0,  0.0,  0.0, "STABLE"),
        # Phase 1: fly TOWARD pillar1 (x=0.4) — OASIS must detect and stop
        (800,  1.0,  0.30, 0.0, "TOWARD-P1"),       # 10cm from pillar1!
        (1400, 1.0,  0.0,  0.0, "RETREAT1"),
        # Phase 2: fly TOWARD pillar3 (y=0.4)
        (2000, 1.0,  0.0,  0.30, "TOWARD-P3"),      # 10cm from pillar3!
        (2600, 1.0,  0.0,  0.0, "RETREAT2"),
        # Phase 3: slalom — pass BETWEEN pillar1 and pillar3 (gap = 0.57m)
        (3200, 1.0,  0.25, 0.25, "BETWEEN-P1P3"),   # in the gap
        (3800, 1.0, -0.25, 0.25, "BETWEEN-P2P3"),   # between P2 and P3
        (4400, 1.0, -0.25,-0.25, "BETWEEN-P2P4"),   # between P2 and P4
        (5000, 1.0,  0.25,-0.25, "BETWEEN-P1P4"),   # between P1 and P4
        (5600, 1.0,  0.0,  0.0, "CENTER"),
        # Phase 4: fly toward east wall (x=0.8) — must detect and stop
        (6200, 1.0,  0.60, 0.0, "TOWARD-WALL"),     # 20cm from wall!
        (6800, 1.0,  0.0,  0.0, "RETREAT3"),
        # Phase 5: altitude changes between pillars
        (7400, 0.5,  0.25, 0.0, "LOW-NEAR-P1"),     # low near pillar
        (8000, 1.5, -0.25, 0.0, "HIGH-NEAR-P2"),    # high near pillar
        (8600, 1.0,  0.0,  0.0, "HOME"),
        (9200, 1.0,  0.0,  0.0, "FINAL"),
    ]
    # Smooth interpolation state (phone-inspired: tension field decay, no snap)
    smooth_alt = 1.0; smooth_x = 0.0; smooth_y = 0.0
    wp_idx = 0
    target_alt = 1.0; target_vx = 0.0; target_vy = 0.0
    wp_name = "INIT"

    tick = 0; CALIB = 50
    past_x = 0.0; past_y = 0.0; past_time = 0.0; first = True

    log("=== OASIS v0.6 Hybrid: Bitcraze PID + OASIS Cortex ===")
    log("  PID = cerebellum (motor control, proven)")
    log("  OASIS = cortex (emotions, learning, navigation)")
    log("  OASIS modulates SETPOINTS, never touches motors.")

    while robot.step(timestep) != -1:
        tick += 1
        dt = timestep / 1000.0
        t0 = time.time()

        # ─── SENSE ───
        roll, pitch, yaw = imu.getRollPitchYaw()
        gx, gy, gz = gyro.getValues()
        x, y, alt = gps.getValues()
        rf = range_f.getValue()/1000; rl = range_l.getValue()/1000
        rr = range_r.getValue()/1000; rb = range_b.getValue()/1000
        obs_near = min(rf, rl, rr, rb)

        if first:
            past_x = x; past_y = y; past_time = robot.getTime(); first = False; continue
        curr_time = robot.getTime()
        dt_sim = curr_time - past_time
        if dt_sim < 0.001: dt_sim = 0.001
        vx = (x - past_x) / dt_sim
        vy = (y - past_y) / dt_sim
        vz = (alt - (past_y if tick < 3 else gps.getValues()[2])) / dt_sim if tick > 2 else 0
        # Fix: compute vz properly
        past_x_old = past_x; past_y_old = past_y
        past_x = x; past_y = y; past_time = curr_time

        # ─── WAYPOINT MANAGER (OASIS cortex decides WHERE) ───
        while wp_idx < len(WAYPOINTS)-1 and tick >= WAYPOINTS[wp_idx+1][0]:
            wp_idx += 1
            _, target_alt, tx, ty, wp_name = WAYPOINTS[wp_idx]
            emo.new_waypoint()
            log(f"\n  >>> WP{wp_idx}: {wp_name} alt={target_alt}m pos=({tx},{ty})")
        _, _, target_x, target_y, _ = WAYPOINTS[wp_idx]

        # Smooth interpolation — fast enough to reach targets, smooth enough to not snap
        smooth_alt += (target_alt - smooth_alt) * 0.15
        smooth_x += (target_x - smooth_x) * 0.25
        smooth_y += (target_y - smooth_y) * 0.25

        alt_error = alt - smooth_alt
        pos_error = math.sqrt((x - smooth_x)**2 + (y - smooth_y)**2)

        # ─── OASIS REFLEX (before cortex) ───
        reflex_fired = False; reflex_type = "NONE"
        if tick <= CALIB:
            reflex_vz.feed(abs(alt - 0.01))
            reflex_obs.feed(1.0/max(obs_near, 0.01))
            if tick == CALIB:
                reflex_vz.calibrate(); reflex_obs.calibrate()
                log(f"  Calibrated. Reflexes ready.")
        else:
            rv = reflex_vz.check(abs(alt - target_alt)) and abs(alt - target_alt) > 2.0
            ro = obs_near < 0.12  # reflex: hard brake only at 12cm (collision imminent)
            if rv or ro:
                reflex_fired = True
                reflex_type = "MULTI" if rv and ro else ("ALT" if rv else "OBS")

        # ─── OASIS EMOTIONS ───
        emo.update(alt, vz if tick > 3 else 0, alt_error, max(0, 0.5-obs_near), pos_error, tick)

        # ─── OASIS EFFERENCE ───
        eff.predict(target_alt)
        sev = eff.reflect(alt)

        # ─── OASIS HEBBIAN (learns high-level behaviors) ───
        # Obstacle → slow down correlation (strong signals near obstacles)
        spd = math.sqrt(vx*vx + vy*vy)
        obs_signal = max(0, min(1, (0.3-obs_near)*5.0))  # 0 at 0.3m, 1 at 0.1m
        syn_obstacle.update(obs_signal, max(0, 1.0 - spd*2))
        # Low altitude fear
        syn_altitude.update(max(0, 0.5-alt)*max(0,-vz if tick>3 else 0), min(1, emo.fear))
        # Lateral drift compensation
        drift = math.sqrt(vx*vx + vy*vy)
        syn_wind.update(min(1, drift*2), max(0, 1.0 - drift*2))
        # Exploration: moving safely → increase confidence → faster next time
        moving_safely = min(1, drift * 3) * max(0, 1.0 - emo.fear)  # moving + no fear
        making_progress = max(0, min(1, emo.satisfaction))  # progress toward target
        syn_explore.update(moving_safely, making_progress)

        # ═══ OASIS CORTEX → PID SETPOINTS ═══
        # OASIS modulates WHAT the PID should do, not HOW

        # Desired altitude: smooth interpolated (no snap)
        desired_alt = smooth_alt

        # Desired velocity: bio-inspired approach — slow near target, freeze if unstable
        # Rule 1: ONLY navigate laterally if altitude is stable (err < 0.2m)
        alt_stable = abs(alt_error) < 0.2
        desired_yaw = 0.0

        if alt_stable:
            dx = smooth_x - x; dy = smooth_y - y
            dist = math.sqrt(dx*dx + dy*dy)
            # OASIS ADVANTAGE: adaptive speed from exploration synapse
            # PID alone: fixed speed. OASIS: faster when confident, slower when scared.
            explore_boost = 1.0 + (syn_explore.weight if syn_explore.active else 0) * 0.5
            base_speed = min(0.25 * explore_boost, dist * 0.5) * emo.curiosity
            # Smooth trajectory: cosine profile (bio: minimum-jerk movement)
            # Acceleration phase → cruise → deceleration phase
            if dist > 0.5:
                phase = min(1.0, (dist - 0.5) / 0.3)  # ramp up over 30cm
                speed = base_speed * (0.5 - 0.5 * math.cos(phase * math.pi))
            elif dist > 0.05:
                phase = dist / 0.5  # smooth brake
                speed = base_speed * (0.5 - 0.5 * math.cos(phase * math.pi))
            else:
                speed = 0.0
            if dist > 0.02 and speed > 0.001:
                desired_vx = (dx / dist) * speed
                desired_vy = (dy / dist) * speed
            else:
                desired_vx = 0.0; desired_vy = 0.0
        else:
            # Altitude not stable → hover in place, don't move laterally
            desired_vx = 0.0; desired_vy = 0.0

        # OASIS MODULATIONS:
        # 1. Fear reduces speed (bio: caution, not paralysis)
        if emo.fear > 1.0:
            fear_slow = max(0.3, 1.0 - emo.fear * 0.05)  # max 35% slowdown at fear=500%
            desired_vx *= fear_slow; desired_vy *= fear_slow

        # 2. REACTIVE OBSTACLE AVOIDANCE (OASIS advantage: PID can't do this)
        # Only react to CLOSE obstacles (< 0.25m = danger zone, not 0.4m)
        avoid_dist = 0.25  # 25cm = real danger (8cm pillar + 17cm clearance)
        if rf < avoid_dist:
            repulse = (avoid_dist - rf) * 0.5
            desired_vx -= repulse
            emo.fear = min(5.0, emo.fear + repulse * 1.0)
        if rb < avoid_dist:
            desired_vx += (avoid_dist - rb) * 0.5
            emo.fear = min(5.0, emo.fear + (avoid_dist - rb) * 1.0)
        if rl < avoid_dist:
            desired_vy -= (avoid_dist - rl) * 0.5
            emo.fear = min(5.0, emo.fear + (avoid_dist - rl) * 1.0)
        if rr < avoid_dist:
            desired_vy += (avoid_dist - rr) * 0.5
            emo.fear = min(5.0, emo.fear + (avoid_dist - rr) * 1.0)
        desired_vx = max(-0.15, min(0.15, desired_vx))
        desired_vy = max(-0.15, min(0.15, desired_vy))

        # 3. Obstacle reflex: FULL BRAKE only when VERY close (< 0.15m)
        if obs_near < 0.15:
            desired_vx = 0.0; desired_vy = 0.0
            emo.fear = min(5.0, emo.fear + 1.0)

        # 4. Obstacle synapse: learned caution (slow down when approaching obstacles)
        if syn_obstacle.active and obs_near < 0.5:
            slow = max(0.2, 1.0 - syn_obstacle.weight * 0.5)
            desired_vx *= slow; desired_vy *= slow

        # 4. ALT reflex: hold altitude target
        if reflex_fired and reflex_type in ("ALT", "MULTI"):
            desired_alt = target_alt

        # 5. Wind compensation (learned lateral damping)
        if syn_wind.active:
            desired_vx -= vx * syn_wind.weight * 0.2
            desired_vy -= vy * syn_wind.weight * 0.2

        # ═══ SAFETY: graceful degradation (phone vitality-inspired) ═══
        # NEVER cut motors in real flight. Degrade: cancel lateral, hold altitude, brake.
        speed = math.sqrt(vx*vx + vy*vy + (vz if tick > 3 else 0)**2)
        danger = speed > 1.5 or abs(alt - smooth_alt) > 2.0 or pos_error > 1.0
        if danger:
            # DEGRADE: stop all lateral movement, hold current altitude, reset PID
            desired_vx = -vx * 0.5  # brake laterally (oppose velocity, don't cut)
            desired_vy = -vy * 0.5
            desired_alt = max(0.3, min(2.0, alt))  # hold current alt, clamped safe
            pid.altitude_integrator *= 0.5  # bleed integrator, don't zero
            pid.past_vx_error *= 0.5; pid.past_vy_error *= 0.5
            eff.pain *= 0.8
            emo.fear = min(5.0, emo.fear + 0.3)
            # Reset smooth targets to current safe position
            smooth_alt = desired_alt; smooth_x = x; smooth_y = y
            if tick % 50 == 0:
                log(f"  !!! DEGRADE spd={speed:.1f} alt={alt:.1f} — brake+hold")

        # ═══ CEREBELLUM: PID flies with OASIS setpoints ═══
        motor_power = pid.pid(dt_sim, desired_vx, desired_vy, desired_yaw, desired_alt,
                              roll, pitch, gz, alt, vx, vy)

        motors[0].setVelocity(-motor_power[0])
        motors[1].setVelocity(motor_power[1])
        motors[2].setVelocity(-motor_power[2])
        motors[3].setVelocity(motor_power[3])

        # ─── TELEMETRY ───
        us = int((time.time() - t0) * 1_000_000)
        n_syn = sum(1 for s in ALL_SYN if s.active)
        syn_str = " ".join(f"{s.name[:3]}:{s.weight:.2f}" for s in ALL_SYN if s.active) or "---"
        if tick % 10 == 0:
            log(f"[T{tick:5}] {emo.dominant():4} f:{emo.fear*100:.0f}% s:{emo.satisfaction*100:.0f}% "
                f"| alt:{alt:.2f} err:{alt_error:+.2f} x:{x:.2f} y:{y:.2f} "
                f"| {n_syn}/4syn [{syn_str}] "
                f"| rf:{reflex_type:5} ef:{sev} p:{eff.pain:.1f} | {wp_name} {us}us")

        if tick == 1: log(f"  Calibrating reflexes ({CALIB} ticks)...")
        if tick == CALIB: log(f"  Takeoff!\n")
