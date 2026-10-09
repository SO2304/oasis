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
# `python` exists in Git Bash on this machine; an Ubuntu runner has only `python3`. Pick
# whichever is there rather than assuming, and say so if neither is.
if command -v python3 >/dev/null 2>&1; then PY=python3
elif command -v python >/dev/null 2>&1; then PY=python
else echo "refus: ni python3 ni python"; exit 3; fi
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
seed "$OUT/op.seed" 17       # 0x11, the operator who signs revocation lists
seed "$OUT/op_bad.seed" 99   # 0x63, an operator the gateway does not trust
seed "$OUT/opnode.seed" 187  # 0xBB, the operator TOOL's own mesh identity
OPNODE_FP=bb00000000000000
seed "$OUT/op2.seed" 33      # 0x21, second operator of the quorum
seed "$OUT/op3.seed" 49      # 0x31, third operator of the quorum
OP2_PUB=$("$BIN/oasis_mbtcp_revoke" --print-pub "$(winpath "$OUT/op2.seed")" 2>/dev/null || true)
OP3_PUB=$("$BIN/oasis_mbtcp_revoke" --print-pub "$(winpath "$OUT/op3.seed")" 2>/dev/null || true)
seed "$OUT/hmi2.seed" 238    # 0xEE, a SECOND commanding HMI, with its own registers
HMI2_FP=ee00000000000000
# The operator's PUBLIC key goes in the gateway config. Derived with the same crate the
# gateway verifies with, so the campaign cannot pass by agreeing with its own arithmetic.
OP_PUB=$("$BIN/oasis_mbtcp_revoke" --print-pub "$OUT/op.seed" 2>/dev/null || true)

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
peer = $AGENT_FP,$WOUT/agent.seed,ACTUATE
peer = $GW_FP,$WOUT/gw.seed
# the operator node publishes revocation lists; it is deliberately NOT granted ACTUATE,
# which case C18 relies on
peer = $OPNODE_FP,$WOUT/opnode.seed
register = $REG_OK,0,1000
operator = $OP_PUB
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

cat > "$OUT/revoke.conf" <<CONF
# the operator tool: its OWN mesh identity, so it neither shares the agent's send counter
# nor stops working when the agent's fingerprint is revoked later in the campaign
listen = 127.0.0.1:0
peer_addr = 127.0.0.1:$GW_PORT
unit = $UNIT
timeout_ms = 2000
network_id = 4f415349536e6574
our_fp = $OPNODE_FP
our_seed_file = $WOUT/opnode.seed
peer_fp = $GW_FP
gateway_id = 1
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

note "C15 a signed ORV1 over the link is applied, and the CHANGE is journalled"
# The change journal of Annex III 1.1.9 para 5 (triggers 2 and 3) was implemented and
# Kani-verified 6/6 but demonstrated NOWHERE: its firmware wiring compiles and no board has
# been reflashed. A revocation applied over this link is a configuration change, so this is
# that journal exercised end to end by the shipped binaries, with no board.
#
# A third fingerprint is revoked, not the agent's, so the agent keeps working and the case
# measures the change record rather than a refusal.
VICTIM=dd00000000000000
R15=$("$BIN/oasis_mbtcp_revoke" --config "$(winpath "$OUT/revoke.conf")" --op-seed "$(winpath "$OUT/op.seed")" --epoch 1 --revoke "$VICTIM" 2>&1); RC15=$?
say "  tool: $R15 (rc=$RC15)"
check_eq "C15 the gateway applied the list" "0" "$RC15"
check "C15 the gateway logged it" "REVOCATION Applied epoch=1" "$(cat "$OUT/gateway.log")"
cat "$OUT/jrn.head" "$OUT/jrn.entries" > "$OUT/jrn15.txt"
J15=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn15.txt")" 2>&1)
LAST15=$(printf '%s' "$J15" | grep -E '^  \[' | tail -1)
check "C15 journalled as a Revocation change" "Change(Revocation)" "$LAST15"
check "C15 recorded as APPLIED (flag bit 4)" "flags=0x10" "$LAST15"
check "C15 the chain is still intact" "VERDICT intact" "$J15"

note "C16 a lower epoch is refused as a rollback, and the refusal is journalled too"
# "legitime OU illegitime": an attempt that failed is the evidence an investigator wants,
# so a refused change must leave a record with the applied bit CLEAR.
R16=$("$BIN/oasis_mbtcp_revoke" --config "$(winpath "$OUT/revoke.conf")" --op-seed "$(winpath "$OUT/op.seed")" --epoch 1 --revoke "$VICTIM" 2>&1); RC16=$?
say "  tool: $R16 (rc=$RC16)"
if [ "$RC16" = "0" ]; then say "  FAIL  C16 a replayed epoch was accepted"; FAIL=$((FAIL+1));
else say "  PASS  C16 the same epoch is refused (rc=$RC16)"; PASS=$((PASS+1)); fi
cat "$OUT/jrn.head" "$OUT/jrn.entries" > "$OUT/jrn16.txt"
LAST16=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn16.txt")" 2>&1 | grep -E '^  \[' | tail -1)
check "C16 the refused change is journalled" "Change(Revocation)" "$LAST16"
check "C16 recorded as NOT applied" "flags=0x00" "$LAST16"

note "C17 a list signed by an operator the gateway does not trust is refused"
R17=$("$BIN/oasis_mbtcp_revoke" --config "$(winpath "$OUT/revoke.conf")" --op-seed "$(winpath "$OUT/op_bad.seed")" --epoch 9 --revoke "$VICTIM" 2>&1); RC17=$?
say "  tool: $R17 (rc=$RC17)"
if [ "$RC17" = "0" ]; then say "  FAIL  C17 a list from an untrusted operator was applied"; FAIL=$((FAIL+1));
else say "  PASS  C17 refused (rc=$RC17)"; PASS=$((PASS+1)); fi
cat "$OUT/jrn.head" "$OUT/jrn.entries" > "$OUT/jrn17.txt"
LAST17=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn17.txt")" 2>&1 | grep -E '^  \[' | tail -1)
check "C17 the attempt is journalled, not applied" "flags=0x00" "$LAST17"

note "C12 an order addressed to ANOTHER gateway_id is refused"
# Until 2026-10-09 the gate was fed `authorized: true`, so `gateway_id` was never compared
# and gateway 1 executed orders addressed to gateway 7. The firmware folds that comparison
# into `authorized`; the TCP path hard-coded it. A second agent config, same keys, different
# gateway_id, is the whole test.
sed 's/^gateway_id = 1$/gateway_id = 7/' "$OUT/agent.conf" > "$OUT/agent7.conf"
# The sequence and counter stores are named after the CONFIG file, but they belong to the
# IDENTITY -- and this config reuses the same keys. Starting it with fresh stores made its
# v0B counter restart at 1, the gateway's window refused the envelope as stale, and the
# order never reached the gate: the case passed on 0x0B while claiming to test gateway_id.
# Copying them is the fix for the test; the design note is that these paths should derive
# from the fingerprint, not the filename.
cp "$OUT/agent.conf.seq" "$OUT/agent7.conf.seq" 2>/dev/null || true
cp "$OUT/agent.conf.txc" "$OUT/agent7.conf.txc" 2>/dev/null || true
kill "$AGENT_PID" 2>/dev/null; sleep 0.6
"$BIN/oasis_mbtcp_agent" --config "$(winpath "$OUT/agent7.conf")" >> "$OUT/agent7.log" 2>&1 &
AGENT_PID=$!; PIDS="$PIDS $AGENT_PID"
for i in $(seq 1 20); do ready $AGENT_PORT && break; sleep 0.25; done
W12=$(plc_writes)
OUT12=$(hmi write $REG_OK 601)
check "C12 refused" "EXC" "$OUT12"
sleep 0.4
check_eq "C12 the device was not touched" "$W12" "$(plc_writes)"
cat "$OUT/jrn.head" "$OUT/jrn.entries" > "$OUT/jrn12.txt"
J12=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn12.txt")" 2>&1 | grep -E '^  \[' | tail -1)
check "C12 journalled as NotAuthorized" "Reject(NotAuthorized)" "$J12"

note "C13 a REVOKED origin is refused, and revocation now has an effect at all"
# `revoked: false` was hard-coded, so revoking a node did nothing on this path while
# revocation is a headline feature proved on silicon. The list is config-time here; a
# signed ORV1 over this link is still not handled, and that is the remaining gap.
# Append the key rather than inject it with sed: the first version put a real newline
# inside the sed expression, which split the command across two lines and produced a
# config the gateway could not parse ("missing network_id"). Order does not matter in a
# key=value file.
{ cat "$OUT/gateway.conf"; echo "revoked = $AGENT_FP"; } > "$OUT/gateway_rev.conf"
kill "$GW_PID" 2>/dev/null; sleep 0.6
"$BIN/oasis_mbtcp_gateway" --config "$(winpath "$OUT/gateway_rev.conf")" --journal "$(winpath "$OUT/jrnrev")" >> "$OUT/gateway_rev.log" 2>&1 &
GW_PID=$!; PIDS="$PIDS $GW_PID"
for i in $(seq 1 20); do ready $GW_PORT && break; sleep 0.25; done
check "C13 the gateway says it revoked the fingerprint" "revoked fp=$AGENT_FP" "$(cat "$OUT/gateway_rev.log")"
# Back to a correctly addressed agent so the refusal can only be the revocation.
kill "$AGENT_PID" 2>/dev/null; sleep 0.6
start_agent
for i in $(seq 1 20); do ready $AGENT_PORT && break; sleep 0.25; done
W13=$(plc_writes)
OUT13=$(hmi write $REG_OK 602)
check "C13 refused" "EXC" "$OUT13"
sleep 0.4
check_eq "C13 the device was not touched" "$W13" "$(plc_writes)"
cat "$OUT/jrnrev.head" "$OUT/jrnrev.entries" > "$OUT/jrn13.txt"
J13=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn13.txt")" 2>&1 | grep -E '^  \[' | tail -1)
# What actually happens, which is stronger than what the case first asserted: the MESH
# layer drops a revoked origin before the gate and before verifying its signature, so the
# entry is `Reject(NotVerified)` and never `Reject(Revoked)`. Wiring `revoked` into the
# gate is therefore defence in depth on this path, and the case says so rather than
# pretending the gate is what stopped it.
check "C13 refused at the mesh layer, before the gate" "Reject(NotVerified)" "$J13"
say "  NOTE  C13 revocation bites one layer ABOVE the gate (v0B, first hop); ctx.revoked is defence in depth"
check "C13 the gateway logged the mesh drop" "MESH_DROP" "$(cat "$OUT/gateway_rev.log")"

note "C14 the journal attributes the decision to an origin, not to eight zero bytes"
# Every entry recorded origin=0000 until 2026-10-09, so the record said a decision
# happened and not who caused it -- and para 5 is about a legitimate OR illegitimate
# intervention, which is an attribution claim.
# Read a VERIFIED decision (C12's refusal by the gate), not C13's mesh drop: a dropped
# envelope keeps origin 0000 on purpose, because its origin field is attacker-controlled
# until the signature is checked. The first version of this case read C13's entry and so
# tested the one place the property deliberately does not hold.
ORIG=$(printf '%s' "$J12" | grep -oE 'origin=[0-9a-f]+' | head -1)
say "  the gate's refusal carries $ORIG (agent fp starts ${AGENT_FP:0:4})"
say "  and C13's mesh drop carries $(printf '%s' "$J13" | grep -oE 'origin=[0-9a-f]+' | head -1) -- unverified, so not recorded as fact"
# An empty value is not an attribution. The first version only tested for "origin=0000"
# and so reported PASS on the empty string left behind by a failing C13 — a case that
# passes when its input is missing is worse than no case.
case "$ORIG" in
  origin=0000) say "  FAIL  C14 the origin is still eight zero bytes"; FAIL=$((FAIL+1));;
  origin=[0-9a-f]*) say "  PASS  C14 the decision is attributed to $ORIG"; PASS=$((PASS+1));;
  *) say "  FAIL  C14 no origin field found at all (got '$ORIG')"; FAIL=$((FAIL+1));;
esac

note "C18 a peer WITHOUT the ACTUATE permission cannot command, though its key is known"
# Until 2026-10-09 the gate was fed `authorized: true`, so holding any key in the registry
# was enough to command anything in the register map, where the firmware requires
# `registry.allows(origin, ACTUATE)`. The operator node's key IS in the gateway's registry
# — it has to be, or its revocation lists would be dropped as an unknown sender — and it is
# deliberately not granted ACTUATE. So it is the honest test of the permission, not of the
# key.
check "C18 the gateway printed the operator node with no permission" "fp=$OPNODE_FP permissions=none" "$(cat "$OUT/gateway.log" "$OUT/gateway_rev.log" 2>/dev/null)"
W18=$(plc_writes)
# Send an ORDER (not a revocation) from the operator node's identity.
OUT18=$("$BIN/oasis_mbtcp_order" --config "$(winpath "$OUT/revoke.conf")" --reg $REG_OK --value 700 2>&1); RC18=$?
say "  tool: $OUT18 (rc=$RC18)"
if [ "$RC18" = "0" ]; then say "  FAIL  C18 an order from a peer without ACTUATE was executed"; FAIL=$((FAIL+1));
else say "  PASS  C18 refused (rc=$RC18)"; PASS=$((PASS+1)); fi
sleep 0.4
check_eq "C18 the device was not touched" "$W18" "$(plc_writes)"
# The gateway was restarted in C13, so the live journal is jrnrev — reading jrn would
# assert against the previous process's chain, which is how this case first failed.
cat "$OUT/jrnrev.head" "$OUT/jrnrev.entries" > "$OUT/jrn18.txt"
LAST18=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn18.txt")" 2>&1 | grep -E '^  \[' | tail -1)
check "C18 journalled as NotAuthorized, attributed to the operator node" "origin=bb00" "$LAST18"
check "C18 and named as a permission refusal" "Reject(NotAuthorized)" "$LAST18"

note "C19-C22 quorum k parmi n sur le lien"
# Until 2026-10-09 the gateway refused EVERY multi-signed list by name, because
# `OperatorAuthority` sat behind a dev-dependency cycle and a `[[bin]]` cannot use a
# dev-dependency. The cycle was an artefact -- `oasis-operator-key/src/` mentions `oasis_rt`
# nowhere -- so it moved and the quorum rule is now the crate's own.
#
# A third gateway, on its own port, configured k=2 of n=3. Separate from the main one so
# the earlier cases keep their single-operator authority and nothing is re-interpreted.
QGW_PORT=15031
{ sed "s|^listen = .*|listen = 127.0.0.1:$QGW_PORT|" "$OUT/gateway.conf" | grep -v '^operator = '
  echo "operator = $OP_PUB"
  echo "operator = $OP2_PUB"
  echo "operator = $OP3_PUB"
  echo "quorum = 2"
} > "$OUT/gateway_q.conf"
"$BIN/oasis_mbtcp_gateway" --config "$(winpath "$OUT/gateway_q.conf")" --journal "$(winpath "$OUT/jrnq")" >> "$OUT/gateway_q.log" 2>&1 &
QGW_PID=$!; PIDS="$PIDS $QGW_PID"
for i in $(seq 1 20); do ready $QGW_PORT && break; sleep 0.25; done
check "C19 the gateway printed the quorum it will enforce" "authority=quorum k=2 of n=3" "$(cat "$OUT/gateway_q.log")"

# The tool points at this gateway, with its own identity and its own stores.
sed "s|^peer_addr = .*|peer_addr = 127.0.0.1:$QGW_PORT|" "$OUT/revoke.conf" > "$OUT/revoke_q.conf"

Q19=$("$BIN/oasis_mbtcp_revoke" --config "$(winpath "$OUT/revoke_q.conf")" --op-seed "$(winpath "$OUT/op.seed")" --op-seed "$(winpath "$OUT/op2.seed")" --epoch 1 --revoke "$VICTIM" 2>&1); RC19=$?
say "  tool: $Q19 (rc=$RC19)"
check_eq "C19 two distinct operators meet k=2" "0" "$RC19"
check "C19 the gateway applied it" "REVOCATION Applied epoch=1" "$(cat "$OUT/gateway_q.log")"

Q20=$("$BIN/oasis_mbtcp_revoke" --config "$(winpath "$OUT/revoke_q.conf")" --op-seed "$(winpath "$OUT/op.seed")" --epoch 2 --revoke "$VICTIM" 2>&1); RC20=$?
say "  tool: $Q20 (rc=$RC20)"
if [ "$RC20" = "0" ]; then say "  FAIL  C20 one signature satisfied k=2"; FAIL=$((FAIL+1));
else say "  PASS  C20 one signature does not meet k=2 (rc=$RC20)"; PASS=$((PASS+1)); fi

# The property the hand-rolled single-key check never had: k DISTINCT keys.
Q21=$("$BIN/oasis_mbtcp_revoke" --config "$(winpath "$OUT/revoke_q.conf")" --op-seed "$(winpath "$OUT/op.seed")" --op-seed "$(winpath "$OUT/op.seed")" --epoch 3 --revoke "$VICTIM" 2>&1); RC21=$?
say "  tool: $Q21 (rc=$RC21)"
if [ "$RC21" = "0" ]; then say "  FAIL  C21 the same operator signing twice was counted as two votes"; FAIL=$((FAIL+1));
else say "  PASS  C21 the same operator twice is one vote, not two (rc=$RC21)"; PASS=$((PASS+1)); fi

cat "$OUT/jrnq.head" "$OUT/jrnq.entries" > "$OUT/jrn19.txt"
JQ=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn19.txt")" 2>&1)
APPLIED=$(printf '%s' "$JQ" | awk '/Change\(Revocation\)/ && /flags=0x10/{n++} END{print n+0}')
REFUSED=$(printf '%s' "$JQ" | awk '/Change\(Revocation\)/ && /flags=0x00/{n++} END{print n+0}')
say "  journal: $APPLIED changement(s) appliqué(s), $REFUSED refusé(s)"
if [ "$APPLIED" -ge 1 ] && [ "$REFUSED" -ge 2 ]; then
  say "  PASS  C19-C21 les trois décisions de quorum sont au journal, appliquée et refusées"; PASS=$((PASS+1))
else
  say "  FAIL  C19-C21 journal attendu >=1 appliqué et >=2 refusés, obtenu $APPLIED / $REFUSED"; FAIL=$((FAIL+1))
fi
check "C19-C21 la chaîne reste intacte" "VERDICT intact" "$JQ"

note "C22 le coût d'un quorum contre une signature unique, K=10"
# The prompt asks for the cost. Measured on the pure verifier rather than over the link, so
# the number is the cryptography and not three loopback hops: n Ed25519 verifications plus
# the distinctness bookkeeping.
# The whole output. An earlier version took `tail -3`, which caught only the closing
# commentary and reported "pas de mesure" about a bench that had measured fine — the case
# was testing where the text happened to end, not whether anything was measured.
C22=$("$BIN/bench_quorum_cost" 2>&1)
printf '%s\n' "$C22" | grep 'ns  ' | sed 's/^/  /' | tee -a "$LOG"
if printf '%s' "$C22" | grep -q 'k=2/n=3'; then
  say "  PASS  C22 coût mesuré, K=10"; PASS=$((PASS+1))
else
  say "  FAIL  C22 pas de mesure: $C22"; FAIL=$((FAIL+1))
fi

note "C23-C26 une carte de registres par origine"
# Until 2026-10-09 the register map was shared: any key the config authorised could write
# every register in it. The firmware requires `registry.allows(origin, ACTUATE)` AND its own
# map; the TCP path had the permission since C18 and the map only now. A pilot's actual
# request is "this HMI may move axis 1 between 0 and 100, that one may only reset the
# counter", which is an authorisation question and not a range check.
#
# A fourth gateway, on its own port, with per-origin rules:
#   agent (aa00) -> register REG_OK, 0..1000
#   hmi2  (ee00) -> register REG_OK2, 0..5      and REG_OK with a ceiling of 9
PGW_PORT=15041
REG_OK2=12
{ sed "s|^listen = .*|listen = 127.0.0.1:$PGW_PORT|" "$OUT/gateway.conf"
  echo "peer = $HMI2_FP,$WOUT/hmi2.seed,ACTUATE"
  echo "register = $REG_OK2,0,1000"
  echo "origin_register = $AGENT_FP,$REG_OK,0,1000"
  echo "origin_register = $HMI2_FP,$REG_OK2,0,5"
  echo "origin_register = $HMI2_FP,$REG_OK,0,9"
} > "$OUT/gateway_p.conf"
"$BIN/oasis_mbtcp_gateway" --config "$(winpath "$OUT/gateway_p.conf")" --journal "$(winpath "$OUT/jrnp")" >> "$OUT/gateway_p.log" 2>&1 &
PGW_PID=$!; PIDS="$PIDS $PGW_PID"
for i in $(seq 1 20); do ready $PGW_PORT && break; sleep 0.25; done

# Two order tools, one per identity, each with its own stores.
sed -e "s|^peer_addr = .*|peer_addr = 127.0.0.1:$PGW_PORT|" "$OUT/revoke.conf" \
    | sed -e "s|^our_fp = .*|our_fp = $AGENT_FP|" -e "s|^our_seed_file = .*|our_seed_file = $WOUT/agent.seed|" > "$OUT/ord_a.conf"
sed -e "s|^peer_addr = .*|peer_addr = 127.0.0.1:$PGW_PORT|" "$OUT/revoke.conf" \
    | sed -e "s|^our_fp = .*|our_fp = $HMI2_FP|" -e "s|^our_seed_file = .*|our_seed_file = $WOUT/hmi2.seed|" > "$OUT/ord_e.conf"

W23=$(plc_writes)
O23=$("$BIN/oasis_mbtcp_order" --config "$(winpath "$OUT/ord_a.conf")" --reg $REG_OK --value 500 2>&1); RC23=$?
say "  agent sur son registre: $O23 (rc=$RC23)"
check_eq "C23 l'agent ecrit son propre registre" "0" "$RC23"
sleep 0.4
check_eq "C23 l'appareil a bien recu une ecriture" "$((W23+1))" "$(plc_writes)"

W24=$(plc_writes)
O24=$("$BIN/oasis_mbtcp_order" --config "$(winpath "$OUT/ord_a.conf")" --reg $REG_OK2 --value 3 2>&1); RC24=$?
say "  agent sur le registre de l'autre: $O24 (rc=$RC24)"
if [ "$RC24" = "0" ]; then say "  FAIL  C24 l'agent a ecrit dans un registre qui n'est pas dans SA carte"; FAIL=$((FAIL+1));
else say "  PASS  C24 refuse: le registre est dans la carte de la passerelle mais pas dans celle de l'agent (rc=$RC24)"; PASS=$((PASS+1)); fi
sleep 0.4
check_eq "C24 l'appareil n'a rien recu" "$W24" "$(plc_writes)"

# The other identity on the same register the agent may write to 1000, but capped at 9.
W25=$(plc_writes)
O25a=$("$BIN/oasis_mbtcp_order" --config "$(winpath "$OUT/ord_e.conf")" --reg $REG_OK --value 5 2>&1); RC25a=$?
O25b=$("$BIN/oasis_mbtcp_order" --config "$(winpath "$OUT/ord_e.conf")" --reg $REG_OK --value 500 2>&1); RC25b=$?
say "  hmi2 a 5 (plafond 9): $O25a (rc=$RC25a)"
say "  hmi2 a 500 (plafond 9): $O25b (rc=$RC25b)"
check_eq "C25 hmi2 sous son plafond est accepte" "0" "$RC25a"
if [ "$RC25b" = "0" ]; then say "  FAIL  C25 hmi2 a depasse son plafond de 9 sur un registre que l'agent peut porter a 1000"; FAIL=$((FAIL+1));
else say "  PASS  C25 la plage est par origine: 500 refuse a hmi2, autorise a l'agent (rc=$RC25b)"; PASS=$((PASS+1)); fi
sleep 0.4
check_eq "C25 une seule ecriture a atteint l'appareil" "$((W25+1))" "$(plc_writes)"

cat "$OUT/jrnp.head" "$OUT/jrnp.entries" > "$OUT/jrn23.txt"
JP=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn23.txt")" 2>&1)
check "C26 la chaine reste intacte" "VERDICT intact" "$JP"
check "C26 un refus hors carte est attribue a l'agent" "origin=aa00" "$(printf '%s' "$JP" | grep 'Reject(OutOfLimits)' | head -1)"
check "C26 un refus de plage est attribue a hmi2" "origin=ee00" "$(printf '%s' "$JP" | grep 'Reject(OutOfLimits)' | tail -1)"

note "C27 compter par origine n'affaiblit pas l'anti-rejeu de cette origine"
# Per-origin counters were added because one global counter made a second commander's
# cmd_seq=1 look like a replay. The property that must survive: a replay of an origin's
# OWN number is still refused. Resetting only the order-sequence file reuses cmd_seq while
# the v0B send counter keeps advancing, so the envelope is fresh and the GATE's cmd_seq
# condition is the only thing that can refuse it.
W27=$(plc_writes)
echo 0 > "$OUT/ord_e.conf.seq"
O27=$("$BIN/oasis_mbtcp_order" --config "$(winpath "$OUT/ord_e.conf")" --reg $REG_OK --value 4 2>&1); RC27=$?
say "  hmi2 rejoue son propre cmd_seq: $O27 (rc=$RC27)"
if [ "$RC27" = "0" ]; then say "  FAIL  C27 une origine a pu rejouer son propre numero"; FAIL=$((FAIL+1));
else say "  PASS  C27 le rejeu de son propre numero est refuse (rc=$RC27)"; PASS=$((PASS+1)); fi
sleep 0.4
check_eq "C27 l'appareil n'a rien recu" "$W27" "$(plc_writes)"
cat "$OUT/jrnp.head" "$OUT/jrnp.entries" > "$OUT/jrn27.txt"
L27=$("$BIN/oasis_journal_verify" "$(winpath "$OUT/jrn27.txt")" 2>&1 | grep -E '^  \[' | tail -1)
check "C27 journalise comme rejeu, attribue a hmi2" "Reject(StaleOrReplayed)" "$L27"
check "C27 et bien a hmi2" "origin=ee00" "$L27"

note "summary"
say "  PLC: $(plc_writes) applied writes, $(plc_frames) frames received"
say "  cases: $PASS passed, $FAIL failed"
if [ "$FAIL" = 0 ]; then say "CAMPAIGN PASS"; else say "CAMPAIGN FAIL"; fi
[ "$FAIL" = 0 ]
