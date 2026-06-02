"""
PID-ONLY Crazyflie Controller — NO OASIS
Same waypoints, same arena, same PID gains.
No emotions, no synapses, no reflexes, no obstacle avoidance.
The PID blindly follows waypoints. If a pillar is in the way, it crashes.

This is the CONTROL GROUP for the A/B comparison.
"""

from controller import Robot
import math

# Bitcraze PID (exact copy from OASIS controller)
class CerebellumPID:
    def __init__(self):
        self.past_vx_error = 0.0
        self.past_vy_error = 0.0
        self.past_alt_error = 0.0
        self.past_pitch_error = 0.0
        self.past_roll_error = 0.0
        self.altitude_integrator = 0.0
    def pid(self, dt, dvx, dvy, dyaw, dalt, roll, pitch, yaw_rate, alt, vx, vy):
        gains = {"kp_att_y": 1, "kd_att_y": 0.5, "kp_att_rp": 0.5, "kd_att_rp": 0.1,
                 "kp_vel_xy": 2, "kd_vel_xy": 0.5, "kp_z": 10, "ki_z": 5, "kd_z": 5}
        vxe = dvx - vx; vxd = (vxe - self.past_vx_error) / dt
        vye = dvy - vy; vyd = (vye - self.past_vy_error) / dt
        dp = gains["kp_vel_xy"] * max(-1, min(1, vxe)) + gains["kd_vel_xy"] * vxd
        dr = -gains["kp_vel_xy"] * max(-1, min(1, vye)) - gains["kd_vel_xy"] * vyd
        self.past_vx_error = vxe; self.past_vy_error = vye
        ae = dalt - alt; ad = (ae - self.past_alt_error) / dt
        self.altitude_integrator += ae * dt
        ac = gains["kp_z"]*ae + gains["kd_z"]*ad + gains["ki_z"]*max(-2, min(2, self.altitude_integrator)) + 48
        self.past_alt_error = ae
        pe = dp - pitch; pd = (pe - self.past_pitch_error) / dt
        re = dr - roll; rd = (re - self.past_roll_error) / dt
        ye = dyaw - yaw_rate
        rc = gains["kp_att_rp"]*max(-1, min(1, re)) + gains["kd_att_rp"]*rd
        pc = -gains["kp_att_rp"]*max(-1, min(1, pe)) - gains["kd_att_rp"]*pd
        yc = gains["kp_att_y"]*max(-1, min(1, ye))
        self.past_pitch_error = pe; self.past_roll_error = re
        m1 = max(0, min(600, ac - rc + pc + yc))
        m2 = max(0, min(600, ac - rc - pc - yc))
        m3 = max(0, min(600, ac + rc - pc + yc))
        m4 = max(0, min(600, ac + rc + pc - yc))
        return [m1, m2, m3, m4]

if __name__ == '__main__':
    robot = Robot()
    timestep = int(robot.getBasicTimeStep())
    log = open("C:/dev/oasis/webots/pid_only_flight.log", "w", encoding="utf-8")

    motors = []
    for name in ["m1_motor", "m2_motor", "m3_motor", "m4_motor"]:
        m = robot.getDevice(name); m.setPosition(float('inf'))
        m.setVelocity(-1 if name in ["m1_motor", "m3_motor"] else 1)
        motors.append(m)

    imu = robot.getDevice("inertial_unit"); imu.enable(timestep)
    gps = robot.getDevice("gps"); gps.enable(timestep)
    gyro = robot.getDevice("gyro"); gyro.enable(timestep)

    pid = CerebellumPID()

    # EXACT SAME waypoints as OASIS controller
    WAYPOINTS = [
        (0,    1.0,  0.0,  0.0, "TAKEOFF"),
        (400,  1.0,  0.0,  0.0, "STABLE"),
        (800,  1.0,  0.30, 0.0, "TOWARD-P1"),
        (1400, 1.0,  0.0,  0.0, "RETREAT1"),
        (2000, 1.0,  0.0,  0.30, "TOWARD-P3"),
        (2600, 1.0,  0.0,  0.0, "RETREAT2"),
        (3200, 1.0,  0.25, 0.25, "BETWEEN-P1P3"),
        (3800, 1.0, -0.25, 0.25, "BETWEEN-P2P3"),
        (4400, 1.0, -0.25,-0.25, "BETWEEN-P2P4"),
        (5000, 1.0,  0.25,-0.25, "BETWEEN-P1P4"),
        (5600, 1.0,  0.0,  0.0, "CENTER"),
        (6200, 1.0,  0.60, 0.0, "TOWARD-WALL"),
        (6800, 1.0,  0.0,  0.0, "RETREAT3"),
        (7400, 0.5,  0.25, 0.0, "LOW-NEAR-P1"),
        (8000, 1.5, -0.25, 0.0, "HIGH-NEAR-P2"),
        (8600, 1.0,  0.0,  0.0, "HOME"),
        (9200, 1.0,  0.0,  0.0, "FINAL"),
    ]

    # Same smooth interpolation as OASIS
    smooth_alt = 1.0; smooth_x = 0.0; smooth_y = 0.0
    wp_idx = 0; tick = 0
    past_x = 0.0; past_y = 0.0
    crashed = False

    msg = "=== PID-ONLY — NO OASIS — Same waypoints, same arena ==="
    print(msg); log.write(msg + "\n")
    msg = "  No emotions, no synapses, no reflexes, no obstacle avoidance."
    print(msg); log.write(msg + "\n")

    while robot.step(timestep) != -1:
        tick += 1
        dt = timestep / 1000.0

        roll, pitch, yaw = imu.getRollPitchYaw()
        gx, gy, gz = gyro.getValues()
        x, y, alt = gps.getValues()
        vx = (x - past_x) / dt if tick > 1 else 0.0
        vy = (y - past_y) / dt if tick > 1 else 0.0
        past_x = x; past_y = y

        # Waypoint progression (same as OASIS)
        target_alt = WAYPOINTS[wp_idx][1]
        while wp_idx < len(WAYPOINTS) - 1 and tick >= WAYPOINTS[wp_idx + 1][0]:
            wp_idx += 1
            _, target_alt, tx, ty, name = WAYPOINTS[wp_idx]
            msg = f"  >>> WP{wp_idx}: {name} alt={target_alt}m pos=({tx},{ty})"
            print(msg); log.write(msg + "\n")
        _, _, target_x, target_y, wp_name = WAYPOINTS[wp_idx]

        # Same smooth interpolation
        smooth_alt += (target_alt - smooth_alt) * 0.15
        smooth_x += (target_x - smooth_x) * 0.25
        smooth_y += (target_y - smooth_y) * 0.25

        # PID: blindly follow setpoints (NO obstacle avoidance)
        dx = smooth_x - x; dy = smooth_y - y
        dist = math.sqrt(dx*dx + dy*dy)
        if dist > 0.02:
            speed = min(0.25, dist * 0.5)
            desired_vx = (dx / dist) * speed
            desired_vy = (dy / dist) * speed
        else:
            desired_vx = 0.0; desired_vy = 0.0

        # Crash detection
        speed_total = math.sqrt(vx*vx + vy*vy + ((alt - smooth_alt) if tick > 3 else 0)**2)
        if abs(alt) > 5.0 or abs(x) > 3.0 or abs(y) > 3.0 or speed_total > 5.0:
            if not crashed:
                crashed = True
                msg = f"  !!! CRASH at T{tick}: alt={alt:.1f} x={x:.1f} y={y:.1f} spd={speed_total:.1f}"
                print(msg); log.write(msg + "\n")

        alt_error = alt - smooth_alt
        pos_error = math.sqrt((x - smooth_x)**2 + (y - smooth_y)**2)

        motor_power = pid.pid(dt, desired_vx, desired_vy, 0, smooth_alt,
                              roll, pitch, gz, alt, vx, vy)
        motors[0].setVelocity(-motor_power[0])
        motors[1].setVelocity(motor_power[1])
        motors[2].setVelocity(-motor_power[2])
        motors[3].setVelocity(motor_power[3])

        if tick % 10 == 0:
            msg = f"[T{tick:5}] alt:{alt:.2f} err:{alt_error:+.2f} x:{x:.2f} y:{y:.2f} | {wp_name}"
            print(msg); log.write(msg + "\n"); log.flush()
