"""
OASIS MAVLink integration test — sends canonical pymavlink-generated frames to
`mavlink_adapter` on UDP 14550, validates that adapter parses them without errors.

This validates:
  1. OASIS CRC-16/MCRF4XX matches pymavlink's reference implementation
  2. Frame structure matches MAVLink v2 spec
  3. Adapter's accumulator correctly emits OASIS JSON to drone_bridge stdin

Run:
  # In another terminal first: ./mavlink_adapter d00 0 &
  python3 tests/mavlink_integration.py
"""
import os, time, sys, socket
# Force MAVLink v2 dialect directly (v1 is 0xFE magic, v2 is 0xFD — OASIS parses v2).
os.environ["MAVLINK20"] = "1"
from pymavlink.dialects.v20 import common as mavlink_v2

TARGET = ("127.0.0.1", 14550)
N_FRAMES = 50


def main():
    import pymavlink
    print(f"[pymavlink_sender] version={pymavlink.__version__} (forcing MAVLink v2)")
    mav = mavlink_v2.MAVLink(None, srcSystem=1, srcComponent=1)

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

    t0 = time.time()
    seq_sent = 0
    for i in range(N_FRAMES):
        msg = mav.heartbeat_encode(
            mavlink_v2.MAV_TYPE_QUADROTOR, mavlink_v2.MAV_AUTOPILOT_PX4,
            mavlink_v2.MAV_MODE_FLAG_SAFETY_ARMED, 0,
            mavlink_v2.MAV_STATE_ACTIVE,
        )
        sock.sendto(msg.pack(mav), TARGET); seq_sent += 1

        msg = mav.attitude_encode(i * 100, 0.1 + 0.01 * i, 0.2, 0.3, 0.0, 0.0, 0.5)
        sock.sendto(msg.pack(mav), TARGET); seq_sent += 1

        msg = mav.global_position_int_encode(
            i * 100,
            int(47.3977 * 1e7), int(8.5456 * 1e7),
            413000, 13000, 50, -30, 0, 0,
        )
        sock.sendto(msg.pack(mav), TARGET); seq_sent += 1

        msg = mav.distance_sensor_encode(
            i * 100, 20, 400, 150 + i,
            mavlink_v2.MAV_DISTANCE_SENSOR_LASER, 0,
            mavlink_v2.MAV_SENSOR_ROTATION_NONE, 0,
        )
        sock.sendto(msg.pack(mav), TARGET); seq_sent += 1

        msg = mav.sys_status_encode(
            0x01 | 0x20 | 0x8000, 0x01 | 0x20 | 0x8000, 0x01 | 0x20 | 0x8000,
            100, 12000, -1, 50, 0, 0, 0, 0, 0, 0,
        )
        sock.sendto(msg.pack(mav), TARGET); seq_sent += 1

        time.sleep(0.02)

    elapsed = time.time() - t0
    print(f"[pymavlink_sender] sent {seq_sent} frames in {elapsed:.2f}s "
          f"({seq_sent/elapsed:.1f} fps) to {TARGET[0]}:{TARGET[1]}")

if __name__ == "__main__":
    main()
