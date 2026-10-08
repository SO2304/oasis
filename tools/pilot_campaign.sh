#!/usr/bin/env bash
# pilot_campaign.sh <outdir> — this PC as the test PLC, end to end, as real processes.
#
# What this adds over `tests/mbtcp_pilot_sockets.rs`, which already drives the library
# over sockets: it runs the **shipped binaries**, so it exercises config files, seed files
# read from disk, the listener, the thread per connection, the persisted sequence file, and
# the journal on disk — none of which a library test touches. Three processes and a client
# in another language:
#
#   hmi_client.py  --Modbus TCP-->  oasis_mbtcp_agent  --v0B-->  oasis_mbtcp_gateway
#                                                                      |
#                                                            Modbus TCP v
#                                                              oasis_test_plc  (rmodbus)
#
# Neither end is OASIS code: the device is `rmodbus` in its own crate that does not depend
# on oasis-rt, and the HMI is Python with nothing imported. The write counter the device
# prints is the ground truth for "only Act decisions reach it".
#
# Writes only under <outdir>. Nothing outside it is touched, and every path used by a
# destructive command is checked first — an empty variable in an inline WSL command once
# made rsync --delete wipe a working tree.
set -u

OUT="${1:?usage: pilot_campaign.sh <outdir>}"
case "$OUT" in
  ""|/|/c|/c/|C:|C:/) echo "refus: outdir dangereux ($OUT)"; exit 3;;
esac
mkdir -p "$OUT" || exit 3
OUT="$(cd "$OUT" && pwd)"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release"
PY=python
LOG="$OUT/campaign.log"
: > "$LOG"

# The binaries are native Windows programs; a path like /tmp/x means nothing to them, and
# the first run of this script died exactly there — the configs held MSYS paths and both
# OASIS processes exited with "Le chemin d'acces specifie est introuvable". Shell-side
# paths stay MSYS; anything a binary must open goes through here.
winpath() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }
WOUT="$(winpath "$OUT")"

say() { printf '%s\n' "$*" | tee -a "$LOG"; }
note() { printf '\n== %s ==\n' "$*" | tee -a "$LOG"; }

PIDS=""
cleanup() {
  for p in $PIDS; do kill "$p" 2>/dev/null; done
  sleep 0.3
  for p in $PIDS; do kill -9 "$p" 2>/dev/null; done
}
trap cleanup EXIT

# --- identities. Seeds on disk, never compiled: phase 1.2's lesson. ---
AGENT_FP=aa00000000000000
GW_FP=cc00000000000000
seed() { # seed <file> <byte> — 32 bytes starting at <byte>, ascending, as 64 hex chars
  "$PY" -c "import sys; b=int(sys.argv[1]); sys.stdout.write(''.join('%02x'%((b+i)&0xff) for i in range(32)))" "$2" > "$1"
}
seed "$OUT/agent.seed" 170   # 0xAA
seed "$OUT/gw.seed" 204      # 0xCC

PLC_PORT=15020
GW_PORT=15021
AGENT_PORT=15022
UNIT=0x11
REG_OK=10
REG_BAD=17

cat > "$OUT/gateway.conf" <<CONF
# machine side: verifies, runs the unchanged Part F gate, writes to the PLC only on Act
listen = 127.0.0.1:$GW_PORT
peer_addr = 127.0.0.1:$PLC_PORT
unit = $UNIT
timeout_ms = 2000
network_id = 4f415349536e6574
our_fp = $GW_FP
our_seed_file = $WOUT/gw.seed
peer_fp = $AGENT_FP
gateway_id = 1
peer = $AGENT_FP,$WOUT/agent.seed
peer = $GW_FP,$WOUT/gw.seed
register = $REG_OK,0,1000
CONF

cat > "$OUT/agent.conf" <<CONF
# operator side: the HMI is unmodified and writes plain Modbus TCP here
listen = 127.0.0.1:$AGENT_PORT
peer_addr = 127.0.0.1:$GW_PORT
unit = $UNIT
timeout_ms = 2000
network_id = 4f415349536e6574
our_fp = $AGENT_FP
our_seed_file = $WOUT/agent.seed
peer_fp = $GW_FP
gateway_id = 1
peer = $AGENT_FP,$WOUT/agent.seed
peer = $GW_FP,$WOUT/gw.seed
register = $REG_OK,0,1000
CONF

say "pilot_campaign $(date -u +%FT%TZ)  tree=$(git -C "$ROOT" rev-parse --short HEAD)"
say "PLC=127.0.0.1:$PLC_PORT  gateway=127.0.0.1:$GW_PORT  agent=127.0.0.1:$AGENT_PORT"
say "device=oasis_test_plc (rmodbus 0.12.2, no oasis-rt dependency)   HMI=tools/hmi_client.py (python, no imports)"

start_plc() {
  "$BIN/oasis_test_plc" --listen "127.0.0.1:$PLC_PORT" --unit "$UNIT" --log "$WOUT/plc.log" >> "$OUT/plc.stdout" 2>&1 &
  PLC_PID=$!; PIDS="$PIDS $PLC_PID"
}
start_gw() {
  "$BIN/oasis_mbtcp_gateway" --config "$WOUT/gateway.conf" --journal "$WOUT/jrn" >> "$OUT/gateway.log" 2>&1 &
  GW_PID=$!; PIDS="$PIDS $GW_PID"
}
start_agent() {
  "$BIN/oasis_mbtcp_agent" --config "$WOUT/agent.conf" >> "$OUT/agent.log" 2>&1 &
  AGENT_PID=$!; PIDS="$PIDS $AGENT_PID"
}

start_plc; start_gw; start_agent

# Wait for all three to accept a connection, and ABORT if any does not. The first run of
# this script ran eleven cases against three dead processes and reported five passes,
# because a refused connection looks like a refused order to anything that only checks
# "did it fail". Silence is not success; nor is failure.
ready() { "$PY" -c "import socket,sys; s=socket.socket(); s.settimeout(0.4)
try:
    s.connect(('127.0.0.1', int(sys.argv[1]))); s.close(); sys.exit(0)
except Exception: sys.exit(1)" "$1" 2>/dev/null; }
for i in $(seq 1 40); do
  if ready $PLC_PORT && ready $GW_PORT && ready $AGENT_PORT; then break; fi
  sleep 0.25
done
BAD=""
ready $PLC_PORT   || BAD="$BAD plc:$PLC_PORT"
ready $GW_PORT    || BAD="$BAD gateway:$GW_PORT"
ready $AGENT_PORT || BAD="$BAD agent:$AGENT_PORT"
if [ -n "$BAD" ]; then
  say "ABORT: these did not come up:$BAD"
  for f in plc.stdout gateway.log agent.log; do
    say "--- $f"; sed -n '1,6p' "$OUT/$f" 2>/dev/null | tee -a "$LOG"
  done
  say "CAMPAIGN ABORTED (nothing was tested)"
  exit 1
fi
say "all three processes are listening"


plc_writes() { if [ -f "$OUT/plc.log" ]; then awk '/APPLIED_WRITE/{n++} END{print n+0}' "$OUT/plc.log"; else echo 0; fi; }
plc_frames() { if [ -f "$OUT/plc.log" ]; then awk '/PLC rx/{n++} END{print n+0}' "$OUT/plc.log"; else echo 0; fi; }

hmi() { # hmi write|read <reg> <val>  -> prints the client's line, returns its exit code
  "$PY" "$ROOT/tools/hmi_client.py" "127.0.0.1:$AGENT_PORT" "$1" "$UNIT" "$2" "$3" 2>&1
}

PASS=0; FAIL=0
check() { # check <name> <expected substring> <actual>
  if printf '%s' "$3" | grep -qF "$2"; then
    say "  PASS  $1  -> $3"; PASS=$((PASS+1))
  else
    say "  FAIL  $1  expected to contain '$2', got: $3"; FAIL=$((FAIL+1))
  fi
}
check_eq() { # check_eq <name> <expected> <actual>
  if [ "$2" = "$3" ]; then say "  PASS  $1  ($3)"; PASS=$((PASS+1));
  else say "  FAIL  $1  expected $2, got $3"; FAIL=$((FAIL+1)); fi
}

note "C1 a legitimate write reaches the device, and the device holds the value"
W0=$(plc_writes)
OUT1=$(hmi write $REG_OK 500)
check "C1 ack" "OK fc=6" "$OUT1"
sleep 0.4
check_eq "C1 device applied exactly one write" "$((W0+1))" "$(plc_writes)"
check "C1 device holds 500" "reg=$REG_OK value=500" "$(grep 'APPLIED_WRITE' "$OUT/plc.log" | tail -1)"

note "C2 an unlisted register: refused, and the device is never asked"
W1=$(plc_writes)
OUT2=$(hmi write $REG_BAD 1)
check "C2 exception 0x02" "code=0x02" "$OUT2"
sleep 0.4
check_eq "C2 no write reached the device" "$W1" "$(plc_writes)"

note "C3 a value outside the configured range: refused the same way"
OUT3=$(hmi write $REG_OK 5000)
check "C3 exception 0x03" "code=0x03" "$OUT3"
sleep 0.4
check_eq "C3 still no write" "$W1" "$(plc_writes)"

note "C4 a read goes THROUGH the gateway and returns what the device holds"
OUT4=$(hmi read $REG_OK 1)
check "C4 read served" "OK fc=3" "$OUT4"
check "C4 read value" "values=500" "$OUT4"

note "C5 an unlisted read is refused and the device is not asked"
OUT5=$(hmi read $REG_BAD 1)
check "C5 read exception 0x02" "code=0x02" "$OUT5"

note "C6 plain Modbus straight at the GATEWAY port must not reach the device"
W2=$(plc_writes)
OUT6=$("$PY" "$ROOT/tools/hmi_client.py" "127.0.0.1:$GW_PORT" write "$UNIT" $REG_OK 777 2>&1)
check "C6 no usable answer from the gateway port" "NOANSWER" "$OUT6"
sleep 0.4
check_eq "C6 device untouched" "$W2" "$(plc_writes)"

note "C7 the agent's sequence survives a restart and is never reused"
SEQ_BEFORE=$(cat "$OUT/agent.conf.seq" 2>/dev/null || echo "none")
kill "$AGENT_PID" 2>/dev/null; sleep 0.6
start_agent; sleep 1.2
OUT7=$(hmi write $REG_OK 501)
check "C7 write after restart acknowledged" "OK fc=6" "$OUT7"
SEQ_AFTER=$(cat "$OUT/agent.conf.seq" 2>/dev/null || echo "none")
say "  seq file before=$SEQ_BEFORE after=$SEQ_AFTER"
if [ "$SEQ_BEFORE" != "none" ] && [ "$SEQ_AFTER" != "none" ] && [ "$SEQ_AFTER" -gt "$SEQ_BEFORE" ]; then
  say "  PASS  C7 the claimed sequence rose across the restart"; PASS=$((PASS+1))
else
  say "  FAIL  C7 sequence did not rise: $SEQ_BEFORE -> $SEQ_AFTER"; FAIL=$((FAIL+1))
fi

note "C8 the journal is on disk and verifies with the independent verifier"
cat "$OUT/jrn.head" "$OUT/jrn.entries" > "$OUT/jrn.txt"
ENTRIES=$(awk '/^JRN_E /{n++} END{print n+0}' "$OUT/jrn.txt" 2>/dev/null || echo 0)
say "  journal carries $ENTRIES entries"
V=$("$BIN/oasis_journal_verify" "$WOUT/jrn.txt" 2>&1); RC=$?
printf '%s
' "$V" > "$OUT/jrn_dump.txt"
say "  oasis_journal_verify -> rc=$RC"
printf '%s
' "$V" | sed 's/^/    /' | tee -a "$LOG" >/dev/null
check_eq "C8 verifier exit code" "0" "$RC"
check "C8 verdict" "verdict intact" "$(printf '%s' "$V" | tr 'A-Z' 'a-z')"
# Assert what the chain CONTAINS, not a number guessed in advance: the first version of
# this case expected >= 6 entries and failed on a correct journal of 4, because the two
# read cases are deliberately NOT journalled (an HMI polls; a day of reads would evict the
# command history from the ring). The decisions that must be there are one Act and three
# refusals, and no read.
ACTS=$(awk '/decision=Act/{n++} END{print n+0}' "$OUT/jrn_dump.txt" 2>/dev/null || echo 0)
REFUSALS=$(awk '/decision=Reject/{n++} END{print n+0}' "$OUT/jrn_dump.txt" 2>/dev/null || echo 0)
say "  chain carries $ACTS acceptance(s) and $REFUSALS refusal(s)"
if [ "$ACTS" -ge 1 ] && [ "$REFUSALS" -ge 2 ]; then
  say "  PASS  C8 acceptances AND refusals are both recorded (1.1.9 para 5: legitimate or illegitimate)"; PASS=$((PASS+1))
else
  say "  FAIL  C8 expected >=1 Act and >=2 Reject, got $ACTS / $REFUSALS"; FAIL=$((FAIL+1))
fi
check "C8 the out-of-limits refusals are named" "Reject(OutOfLimits)" "$(cat "$OUT/jrn_dump.txt")"
if grep -q 'decision=Read' "$OUT/jrn_dump.txt" 2>/dev/null; then
  say "  FAIL  C8 a read was journalled; reads must not be (they would evict the command history)"; FAIL=$((FAIL+1))
else
  say "  PASS  C8 no read was journalled, by design"; PASS=$((PASS+1))
fi
# A LIMIT this run exposed, recorded rather than smoothed over. C6's plain-Modbus
# injection leaves NO journal entry: its first four bytes are not a valid length prefix,
# so `read_frame` refuses it before it can become a decision. Nothing reaches the device,
# which is the safety property -- but there is no evidence of the attempt either. Not
# journalling it is deliberate (a framing-level flood would evict the command history from
# the ring, the same reason reads are not journalled), so it is a known gap in the
# evidence trail and not an oversight.
if grep -q 'Reject(NotVerified)' "$OUT/jrn_dump.txt" 2>/dev/null; then
  say "  NOTE  C8 a NotVerified refusal IS present (an injection reached the mesh layer)"
else
  say "  NOTE  C8 limit: the plain-Modbus injection left no journal entry -- refused by framing, before it could become a decision"
fi

note "C9 one flipped hex digit in the journal must be detected"
cp "$OUT/jrn.txt" "$OUT/jrn_tampered.txt"
"$PY" - "$OUT/jrn_tampered.txt" <<'PYEOF'
import io, sys
p = sys.argv[1]
lines = io.open(p, encoding='utf-8').read().split('\n')
# Target a REFUSAL, so the mutation is "a refused order recorded as executed". Byte 25 is
# the decision; hex offset 50. An earlier version broke on the FIRST entry, which was an
# Act whose decision byte is already 00 -- so it wrote 00 over 00, changed nothing, and
# reported "tampering NOT detected" about a mutation that never happened. A mutation that
# does not mutate is the quietest way for a negative control to lie.
done = False
for i, l in enumerate(lines):
    # A real entry line, not the JRN_END marker: the predicate must be 'JRN_E ' with the
    # space, because JRN_END also starts with JRN_E (this bit a silicon test twice).
    if l.startswith('JRN_E '):
        h = l[6:]
        before = h[50:52]
        if before == '00':
            continue          # already an Act: rewriting it as Act is not a mutation
        lines[i] = 'JRN_E ' + h[:50] + '00' + h[52:]
        print('entry %d decision byte %s -> 00 (a refusal rewritten as Act)' % (i, before))
        done = True
        break
if not done:
    raise SystemExit('ABORT: no refusal entry to tamper, the control would prove nothing')
io.open(p, 'w', encoding='utf-8', newline='\n').write('\n'.join(lines))
PYEOF
V2=$("$BIN/oasis_journal_verify" "$WOUT/jrn_tampered.txt" 2>&1); RC2=$?
say "  oasis_journal_verify (tampered) -> rc=$RC2  $V2"
if [ "$RC2" != "0" ]; then say "  PASS  C9 tampering detected (rc=$RC2)"; PASS=$((PASS+1));
else say "  FAIL  C9 tampering NOT detected"; FAIL=$((FAIL+1)); fi

note "C10 a gateway restart changes boot_id, so the journal starts a new chain"
BOOT1=$(grep '^JRN_BOOT' "$OUT/jrn.head" | head -1)
kill "$GW_PID" 2>/dev/null; sleep 0.6
start_gw; sleep 1.2
OUT10=$(hmi write $REG_OK 502)
say "  write after gateway restart: $OUT10"
BOOT2=$(grep '^JRN_BOOT' "$OUT/jrn.head" | head -1)
say "  boot before=[$BOOT1] after=[$BOOT2]"
if [ "$BOOT1" != "$BOOT2" ]; then say "  PASS  C10 boot_id changed across the restart"; PASS=$((PASS+1));
else say "  FAIL  C10 boot_id unchanged — a previous-run order would still look fresh"; FAIL=$((FAIL+1)); fi
cat "$OUT/jrn.head" "$OUT/jrn.entries" > "$OUT/jrn2.txt"
V3=$("$BIN/oasis_journal_verify" "$WOUT/jrn2.txt" 2>&1); RC3=$?
say "  new chain verify -> rc=$RC3  $V3"
check_eq "C10 the new chain is intact too" "0" "$RC3"

note "C11 latency through the real binaries, K=10 x 20 writes"
# One python process, one socket held open, so what is measured is the exchange and not
# the cost of starting an interpreter. The first version of this case spawned a process
# per write and reported 151 ms as a latency, which would have been a misleading number in
# the evidence even with a caveat beside it.
MEDOUT=$("$PY" - "127.0.0.1:$AGENT_PORT" "$UNIT" "$REG_OK" <<'PYEOF'
import socket, struct, sys, time, statistics
addr, unit, reg = sys.argv[1], int(sys.argv[2], 0), int(sys.argv[3], 0)
host, _, port = addr.rpartition(':')

def one(s, tid, val):
    pdu = struct.pack('>BBHH', unit, 6, reg, val)
    s.sendall(struct.pack('>HHH', tid, 0, len(pdu)) + pdu)
    head = b''
    while len(head) < 7:
        c = s.recv(7 - len(head))
        if not c:
            raise IOError('closed')
        head += c
    ln = struct.unpack('>H', head[4:6])[0]
    body = b''
    while len(body) < ln - 1:
        c = s.recv(ln - 1 - len(body))
        if not c:
            raise IOError('closed')
        body += c
    return (head[6:7] + body)[1]

s = socket.create_connection((host, int(port)), timeout=5)
s.settimeout(5)
s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
# Warm-up, discarded: the first exchange pays the agent's clock fetch and a lease write.
for i in range(5):
    one(s, 1, 100)
rounds = []
for k in range(10):
    ts = []
    for i in range(20):
        t0 = time.perf_counter()
        fc = one(s, (k << 8) | i, 100 + i)
        ts.append((time.perf_counter() - t0) * 1e6)
        if fc & 0x80:
            print('ROUND_FAIL exception 0x%02x' % fc)
            sys.exit(1)
    rounds.append(statistics.median(ts))
s.close()
rounds.sort()
med = rounds[len(rounds) // 2]
half = (rounds[-1] - rounds[0]) / 2.0
print('%d %.0f' % (round(med), 100.0 * half / med))
PYEOF
)
case "$MEDOUT" in
  ROUND_FAIL*) say "  FAIL  C11 $MEDOUT"; FAIL=$((FAIL+1));;
  *) say "  PASS  C11 HMI -> agent -> gateway -> PLC: median ${MEDOUT%% *} us, spread +/-${MEDOUT##* }% (K=10 x 20, one held socket)"
     say "        in-process equivalent for comparison: ~460 us (evidence/bench/2026-10-08/mbtcp/)"
     PASS=$((PASS+1));;
esac

note "summary"
say "  PLC: $(plc_writes) applied writes, $(plc_frames) frames received"
say "  cases: $PASS passed, $FAIL failed"
if [ "$FAIL" = 0 ]; then say "CAMPAIGN PASS"; else say "CAMPAIGN FAIL"; fi
[ "$FAIL" = 0 ]
