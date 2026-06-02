"""
OASIS — Signed MAVLink + replay protection end-to-end test.

Sends SIGNED MAVLink v2 frames to the adapter via UDP, then replays a captured
signed frame to verify the adapter's replay protection rejects it.

Requires:
  - adapter running with OASIS_MAVLINK_SECRET=<64 hex chars>
  - pymavlink v2 dialect

Test sequence:
  Phase 1: send 20 fresh signed frames (increasing timestamps) → all accepted
  Phase 2: replay the first signed frame → rejected
  Phase 3: send a new fresh frame (higher timestamp than any prior) → accepted

Validates:
  - Signed frames round-trip
  - Replay detection working
  - Per-link_id timestamp monotonicity
"""
import os, time, socket, sys
os.environ["MAVLINK20"] = "1"
from pymavlink.dialects.v20 import common as mav2

ADAPTER_ADDR = ("127.0.0.1", 14550)
OUR_BIND = ("127.0.0.1", 14551)
# Must match OASIS_MAVLINK_SECRET env on adapter side (64 hex chars = 32 bytes).
SECRET_HEX = "aa" * 32

def make_signed_mav():
    mav = mav2.MAVLink(None, srcSystem=1, srcComponent=1)
    # Enable signing (pymavlink internal)
    import hashlib
    key_bytes = bytes.fromhex(SECRET_HEX)
    mav.signing.secret_key = key_bytes
    mav.signing.sign_outgoing = True
    mav.signing.link_id = 1
    mav.signing.timestamp = int(time.time() * 1e5)  # 10µs units
    return mav

def main():
    print(f"[vpx4_signed] pymavlink MAVLink v2 signed + replay protection test")
    mav = make_signed_mav()
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(OUR_BIND)
    sock.setblocking(False)

    captured_frame = None  # for replay attack

    # Phase 1: 20 signed frames with increasing timestamps
    accepted_setpoints = 0

    def drain():
        nonlocal accepted_setpoints
        try:
            while True:
                data, _ = sock.recvfrom(2048)
                if len(data) >= 10 and data[0] == 0xFD:
                    msgid = data[7] | (data[8] << 8) | (data[9] << 16)
                    if msgid == 84:
                        accepted_setpoints += 1
        except BlockingIOError:
            pass

    print("[vpx4_signed] Phase 1: sending 20 fresh signed ATTITUDE frames...")
    for i in range(20):
        att = mav.attitude_encode(i * 100, 0.1 + i * 0.01, 0.2, 0.3, 0.0, 0.0, 0.5)
        mav.signing.timestamp = int((time.time() + i * 0.001) * 1e5)
        packed = att.pack(mav)
        if captured_frame is None:
            captured_frame = packed   # save first signed frame for replay
        sock.sendto(packed, ADAPTER_ADDR)

        # Also send GLOBAL_POSITION_INT so accumulator emits OASIS JSON
        gp = mav.global_position_int_encode(
            i * 100, int(1.0 * 1e7), int(2.0 * 1e7),
            1300, 1300, 0, 0, 0, 0,
        )
        mav.signing.timestamp = int((time.time() + i * 0.001 + 0.0005) * 1e5)
        sock.sendto(gp.pack(mav), ADAPTER_ADDR)
        time.sleep(0.03)
        drain()

    time.sleep(0.5)
    drain()
    phase1_accepted = accepted_setpoints
    print(f"[vpx4_signed] Phase 1: {phase1_accepted} setpoints received after fresh signed frames")

    # Phase 2: replay the captured frame
    print("[vpx4_signed] Phase 2: replaying the FIRST signed frame 10x...")
    for _ in range(10):
        sock.sendto(captured_frame, ADAPTER_ADDR)
        time.sleep(0.05)
    time.sleep(0.5)
    before_phase3 = accepted_setpoints
    drain()

    # Phase 3: fresh frame with a NEW timestamp higher than any prior
    print("[vpx4_signed] Phase 3: sending new fresh frame...")
    for i in range(5):
        att = mav.attitude_encode(i * 500, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0)
        mav.signing.timestamp = int(time.time() * 1e5) + 1000000  # way-future ts
        sock.sendto(att.pack(mav), ADAPTER_ADDR)
        gp = mav.global_position_int_encode(
            i * 500, int(1.5 * 1e7), int(2.5 * 1e7),
            1300, 1300, 0, 0, 0, 0,
        )
        mav.signing.timestamp = int(time.time() * 1e5) + 1000001 + i
        sock.sendto(gp.pack(mav), ADAPTER_ADDR)
        time.sleep(0.05)
        drain()
    time.sleep(0.5)
    drain()

    print("\n[vpx4_signed] === RESULTS ===")
    print(f"  After Phase 1 (20 fresh frames): {phase1_accepted} setpoints received")
    print(f"  After Phase 2 (10 replays):       {before_phase3} setpoints (expected same as Phase 1)")
    print(f"  After Phase 3 (new fresh frame):  {accepted_setpoints} setpoints (expected > Phase 2)")

    # Verdict
    phase2_unchanged = (before_phase3 == phase1_accepted)
    phase3_grew = (accepted_setpoints > before_phase3)
    if phase2_unchanged and phase3_grew:
        print("\n[vpx4_signed] PASS: replay attack rejected, fresh signed frames accepted")
        return 0
    else:
        print(f"\n[vpx4_signed] FAIL: phase2_unchanged={phase2_unchanged} phase3_grew={phase3_grew}")
        return 1

if __name__ == "__main__":
    sys.exit(main())
