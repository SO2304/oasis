"""
OASIS — Signed MAVLink + allowlist test.
Sends signed frames from link_id=1 (allowed) and link_id=42 (NOT allowed).
Expects: link_id=1 accepted, link_id=42 rejected.
"""
import os, time, socket, sys
os.environ["MAVLINK20"] = "1"
from pymavlink.dialects.v20 import common as mav2

ADAPTER_ADDR = ("127.0.0.1", 14550)
OUR_BIND = ("127.0.0.1", 14551)
SECRET_HEX = "bb" * 32

def mk_mav(link_id: int):
    mav = mav2.MAVLink(None, srcSystem=1, srcComponent=1)
    mav.signing.secret_key = bytes.fromhex(SECRET_HEX)
    mav.signing.sign_outgoing = True
    mav.signing.link_id = link_id
    mav.signing.timestamp = int(time.time() * 1e5) + link_id * 1000
    return mav

def main():
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(OUR_BIND)
    sock.setblocking(False)

    setpoints = 0
    def drain():
        nonlocal setpoints
        try:
            while True:
                data, _ = sock.recvfrom(2048)
                if len(data) >= 10 and data[0] == 0xFD:
                    msgid = data[7] | (data[8] << 8) | (data[9] << 16)
                    if msgid == 84:
                        setpoints += 1
        except BlockingIOError:
            pass

    # Phase A: link_id=1 (allowed) — should produce setpoints
    mav1 = mk_mav(1)
    for i in range(15):
        att = mav1.attitude_encode(i * 100, 0.1, 0.2, 0.3, 0.0, 0.0, 0.5)
        mav1.signing.timestamp = int(time.time() * 1e5) + i * 10
        sock.sendto(att.pack(mav1), ADAPTER_ADDR)
        gp = mav1.global_position_int_encode(i * 100, int(1.0 * 1e7), int(2.0 * 1e7), 1300, 1300, 0, 0, 0, 0)
        mav1.signing.timestamp = int(time.time() * 1e5) + i * 10 + 5
        sock.sendto(gp.pack(mav1), ADAPTER_ADDR)
        time.sleep(0.03)
        drain()
    time.sleep(0.3); drain()
    phase_a_sp = setpoints

    # Phase B: link_id=42 (NOT allowed) — should produce no additional setpoints
    mav42 = mk_mav(42)
    for i in range(15):
        att = mav42.attitude_encode(i * 100, 0.1, 0.2, 0.3, 0.0, 0.0, 0.5)
        mav42.signing.timestamp = int(time.time() * 1e5) + i * 10 + 100000
        sock.sendto(att.pack(mav42), ADAPTER_ADDR)
        gp = mav42.global_position_int_encode(i * 100, int(1.0 * 1e7), int(2.0 * 1e7), 1300, 1300, 0, 0, 0, 0)
        mav42.signing.timestamp = int(time.time() * 1e5) + i * 10 + 100005
        sock.sendto(gp.pack(mav42), ADAPTER_ADDR)
        time.sleep(0.03)
        drain()
    time.sleep(0.3); drain()
    phase_b_sp = setpoints

    print(f"Phase A (link_id=1 allowed):  {phase_a_sp} setpoints")
    print(f"Phase B (link_id=42 blocked): {phase_b_sp} setpoints (expected == phase A)")

    if phase_b_sp == phase_a_sp and phase_a_sp > 0:
        print("PASS: allowlist blocked link_id=42, accepted link_id=1")
        return 0
    else:
        print(f"FAIL: phase_a={phase_a_sp}, phase_b={phase_b_sp}")
        return 1

if __name__ == "__main__":
    sys.exit(main())
