"""
OASIS — Virtual PX4 proxy for closed-loop integration test.

Substitutes for a full PX4 SITL install:
  - Emits canonical MAVLink v2 sensor frames (HEARTBEAT/ATTITUDE/GLOBAL_POSITION_INT/
    DISTANCE_SENSOR/SYS_STATUS) at 50 Hz on UDP 14550
  - Listens for OASIS's SET_POSITION_TARGET_LOCAL_NED replies
  - Measures closed-loop latency: from sensor-frame-sent to command-received

Expected usage:
  # Terminal 1 (WSL): start adapter with drone_bridge subprocess
  OASIS_BRIDGE_PATH=/root/oasis_linux_target/release/drone_bridge \\
    OASIS_LOG_LATENCY=1 \\
    OASIS_MAV_PEER=127.0.0.1:14551 \\
    /root/oasis_linux_target/release/mavlink_adapter d00 0 > /tmp/adapter.out 2>&1 &

  # Terminal 2: run this virtual PX4
  /opt/mavtest/bin/python3 tests/virtual_px4.py
"""
import os, time, socket, struct, sys
os.environ["MAVLINK20"] = "1"
from pymavlink.dialects.v20 import common as mav2

ADAPTER_ADDR = ("127.0.0.1", 14550)
OUR_BIND = ("127.0.0.1", 14551)  # adapter sends setpoints here
N_SECONDS = 10
TX_HZ = 50


def main():
    print(f"[vpx4] virtual PX4 proxy — {N_SECONDS}s at {TX_HZ} Hz")

    mav = mav2.MAVLink(None, srcSystem=1, srcComponent=1)
    # Use ONE socket for both tx and rx, bound to OUR_BIND. This way the adapter's
    # auto-peer-learning on recv_from picks up 14551 as source port → setpoints come back here.
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(OUR_BIND)
    sock.setblocking(False)
    tx = sock
    rx = sock

    tick_interval = 1.0 / TX_HZ
    t0 = time.time()
    next_tick = t0

    # Track closed-loop: map (sent_timestamp) → measure RTT when matching setpoint received
    sensor_timestamps = []  # list of (seq, send_time)
    setpoint_times = []     # list of receive times
    latencies_ms = []
    setpoint_count = 0
    tick = 0

    while time.time() - t0 < N_SECONDS:
        now = time.time()
        # --- emit one sensor "tick" per period ---
        if now >= next_tick:
            send_t = time.time()

            # HEARTBEAT
            hb = mav.heartbeat_encode(mav2.MAV_TYPE_QUADROTOR, mav2.MAV_AUTOPILOT_PX4,
                                       mav2.MAV_MODE_FLAG_SAFETY_ARMED, 0, mav2.MAV_STATE_ACTIVE)
            tx.sendto(hb.pack(mav), ADAPTER_ADDR)

            # ATTITUDE
            att = mav.attitude_encode(tick, 0.1, 0.2, 0.3, 0.0, 0.0, 0.5)
            tx.sendto(att.pack(mav), ADAPTER_ADDR)

            # GLOBAL_POSITION_INT — values chosen so adapter's lat/1e7 → local x in [-5, 5] meters
            # (OASIS drone_bridge rejects |x|>5.5 as DIVERGENCE). Same for y. alt in mm → 1.3 m.
            gp = mav.global_position_int_encode(
                tick,
                int(1.0 * 1e7),   # lat→x = 1.0 m
                int(2.0 * 1e7),   # lon→y = 2.0 m
                1300,             # alt_mm = 1300 → alt = 1.3 m
                1300,             # relative_alt
                50, -30, 0, 0,    # vx, vy, vz, hdg
            )
            tx.sendto(gp.pack(mav), ADAPTER_ADDR)
            sensor_timestamps.append((tick, send_t))

            tick += 1
            next_tick += tick_interval

        # --- drain setpoint replies ---
        try:
            while True:
                data, _ = rx.recvfrom(2048)
                rcv_t = time.time()
                setpoint_count += 1
                setpoint_times.append(rcv_t)
                # Quick header-only check — adapter sends msgid 84 (SET_POSITION_TARGET_LOCAL_NED)
                if len(data) >= 10 and data[0] == 0xFD:
                    msgid = data[7] | (data[8] << 8) | (data[9] << 16)
                    if msgid == 84 and sensor_timestamps:
                        # Latency: from most recent sensor bundle emission → this setpoint arrival
                        last_send = sensor_timestamps[-1][1]
                        lat_ms = (rcv_t - last_send) * 1000.0
                        latencies_ms.append(lat_ms)
        except BlockingIOError:
            pass

        time.sleep(0.001)

    elapsed = time.time() - t0
    print(f"[vpx4] finished {elapsed:.2f}s")
    print(f"[vpx4] sensor bundles sent:     {len(sensor_timestamps)}")
    print(f"[vpx4] setpoints received:      {setpoint_count}")
    if latencies_ms:
        latencies_ms.sort()
        n = len(latencies_ms)
        mean = sum(latencies_ms) / n
        med = latencies_ms[n // 2]
        p95 = latencies_ms[int(n * 0.95)]
        print(f"[vpx4] closed-loop latency:     n={n} mean={mean:.2f}ms median={med:.2f}ms p95={p95:.2f}ms")
        print(f"[vpx4]                           min={latencies_ms[0]:.2f}ms max={latencies_ms[-1]:.2f}ms")
    else:
        print(f"[vpx4] NO SETPOINTS RECEIVED — adapter not replying (bridge may not be spawning commands)")
        sys.exit(1)


if __name__ == "__main__":
    main()
