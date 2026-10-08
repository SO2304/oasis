#!/usr/bin/env python3
"""A Modbus TCP client standing in for an HMI. **No OASIS code**, no dependencies.

The point of writing the client in another language with nothing imported is the same as
the point of using `rmodbus` for the device: when this says "the write was acknowledged",
OASIS is not agreeing with itself. The HMI in a real deployment is not ours either, and
the pilot's whole claim is that it does not have to be modified.

    hmi_client.py <host:port> write <unit> <reg> <value>
    hmi_client.py <host:port> read  <unit> <reg> <count>

Prints one line of the form `OK fc=6 reg=10 value=500` or
`EXC fc=0x86 code=0x02` or `NOANSWER <reason>`, and exits 0 on an acknowledged
exchange, 1 on a Modbus exception, 2 on no usable answer. A campaign reads the exit code.
"""
import socket
import struct
import sys

TIMEOUT = 5.0


def exchange(addr, pdu):
    host, _, port = addr.rpartition(":")
    s = socket.create_connection((host, int(port)), timeout=TIMEOUT)
    s.settimeout(TIMEOUT)
    # One small request, one small reply, alternating: disable Nagle or the measurement
    # reports the delayed ACK and not the exchange.
    s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    try:
        # MBAP: transaction id, protocol 0, length, then the PDU (unit + function + data).
        tid = 0x4242
        frame = struct.pack(">HHH", tid, 0, len(pdu)) + pdu
        s.sendall(frame)
        head = recv_exact(s, 7)
        if head is None:
            return None, "closed before the header"
        rtid, proto, length = struct.unpack(">HHH", head[:6])
        if rtid != tid:
            return None, "transaction id %d, expected %d" % (rtid, tid)
        if proto != 0:
            return None, "protocol %d" % proto
        body = recv_exact(s, length - 1)
        if body is None:
            return None, "closed mid-body"
        return head[6:7] + body, None
    finally:
        s.close()


def recv_exact(s, n):
    buf = b""
    while len(buf) < n:
        try:
            chunk = s.recv(n - len(buf))
        except socket.timeout:
            return None
        if not chunk:
            return None
        buf += chunk
    return buf


def main():
    if len(sys.argv) < 6:
        print(__doc__)
        return 2
    addr, op, unit, a, b = sys.argv[1], sys.argv[2], int(sys.argv[3], 0), int(sys.argv[4], 0), int(sys.argv[5], 0)

    if op == "write":
        pdu = struct.pack(">BBHH", unit, 6, a, b)
    elif op == "read":
        pdu = struct.pack(">BBHH", unit, 3, a, b)
    else:
        print("NOANSWER unknown op %s" % op)
        return 2

    try:
        resp, err = exchange(addr, pdu)
    except OSError as e:
        print("NOANSWER %s" % e.__class__.__name__)
        return 2
    if resp is None:
        print("NOANSWER %s" % err)
        return 2

    fc = resp[1]
    if fc & 0x80:
        print("EXC fc=0x%02x code=0x%02x" % (fc, resp[2]))
        return 1
    if op == "write":
        reg, val = struct.unpack(">HH", resp[2:6])
        print("OK fc=%d reg=%d value=%d" % (fc, reg, val))
    else:
        n = resp[2] // 2
        vals = struct.unpack(">" + "H" * n, resp[3:3 + 2 * n])
        print("OK fc=%d count=%d values=%s" % (fc, n, ",".join(str(v) for v in vals)))
    return 0


sys.exit(main())
