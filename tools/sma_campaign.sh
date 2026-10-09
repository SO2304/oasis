#!/usr/bin/env bash
# sma_campaign.sh — PARTIAL demonstration of the OASIS gateway in front of a real SMA
# inverter (prompts/OASIS_SMA_REAL_TEST.md, phase 4 item 5, replay script).
#
#   bash tools/sma_campaign.sh [inverter_addr] [unit_hex] [out_dir]
#   defaults: 169.254.12.3:502  0x7e (SunSpec unit 126)  evidence/real/2026-10-09/sma/run
#
# What it proves, and only this: pointed at the real inverter's address, the gateway
# refuses every attack and builds no frame, and the ONLY decision that even attempts to
# open a connection to the inverter is a single legitimate Act. The inverter's Modbus
# server is **off by default**, so that Act cannot land — the write half of the campaign
# stays blocked on the owner activating Modbus through the web UI. Nothing is written to
# the inverter here.
#
# SAFETY (rule 1):
#  - The write register is a **placeholder** (40236). The real WMaxLimPct address is only
#    known after `oasis_sma_probe` walks the SunSpec chain on a live Modbus. So if the
#    inverter's port 502 is OPEN (Modbus active), this script REFUSES to send the
#    legitimate Act — it would risk writing a wrong register on a live inverter — and runs
#    only the attack cases, which never reach the device. Against the current (Modbus-off)
#    inverter the Act is demonstrated harmlessly: it times out on a closed port.
#  - At most ONE legitimate write attempt per run. The script does not loop.
#  - The register map range is 50..100, never 0.
set -u

INV="${1:-169.254.12.3:502}"
UNIT="${2:-0x7e}"
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
OUT="${3:-$ROOT/evidence/real/2026-10-09/sma/run}"
BIN="$ROOT/target/release"
LOG="$OUT/campaign.log"
REG_OK=40236          # PLACEHOLDER — not the confirmed WMaxLimPct address
REG_BAD=40999         # out of the one-entry map
GW_PORT=15531

command -v cygpath >/dev/null 2>&1 && winpath() { cygpath -m "$1"; } || winpath() { printf '%s' "$1"; }

if [ ! -x "$BIN/oasis_mbtcp_gateway" ] && [ ! -f "$BIN/oasis_mbtcp_gateway.exe" ]; then
  echo "build first: cargo build --release -p oasis-rt"; exit 2
fi

WORK="$(mktemp -d 2>/dev/null || echo "${TMPDIR:-/tmp}/sma_$$")"; mkdir -p "$WORK" "$OUT"
cleanup() { [ -n "${GWPID:-}" ] && kill "$GWPID" 2>/dev/null; rm -rf "$WORK"; }
trap cleanup EXIT

say() { printf '%s\n' "$*" | tee -a "$LOG"; }
: > "$LOG"
STAMP="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
say "sma_campaign $(date -u +%FT%TZ)  tree=$STAMP  inverter=$INV  unit=$UNIT"
say "# PARTIAL: proves the refusal asymmetry against a real SMA inverter; writes NOTHING to it."

# Deterministic seeds, pure bash (no python, rule 7).
seed() { local b=$2 i hex=""; for i in $(seq 0 31); do hex+=$(printf '%02x' $(( (b+i) & 255 ))); done; printf '%s' "$hex" > "$1"; }
seed "$WORK/agent.seed" 170   # 0xAA  enrolled, ACTUATE
seed "$WORK/gw.seed"    204   # 0xCC  the gateway
seed "$WORK/forged.seed" 240  # 0xF0  NOT enrolled in the gateway
seed "$WORK/noact.seed" 120   # 0x78  enrolled, NO ACTUATE
W="$(winpath "$WORK")"
pub() { "$BIN/oasis_mbtcp_revoke" --print-pub "$W/$1.seed" 2>/dev/null; }
AGENT_PUB="$(pub agent)"; GW_PUB="$(pub gw)"; NOACT_PUB="$(pub noact)"
AGENT_FP=aa00000000000000; GW_FP=cc00000000000000; FORGED_FP=f0000000000000f0; NOACT_FP=7800000000000000

cat > "$WORK/gateway.conf" <<CONF
listen = 127.0.0.1:$GW_PORT
peer_addr = $INV
unit = $UNIT
timeout_ms = 2000
network_id = 4f415349536e6574
our_fp = $GW_FP
our_seed_file = $W/gw.seed
peer_fp = $AGENT_FP
gateway_id = 1
peer_pk = $AGENT_FP,$AGENT_PUB,ACTUATE
peer_pk = $NOACT_FP,$NOACT_PUB
peer_pk = $GW_FP,$GW_PUB
register = $REG_OK,50,100
CONF

order_conf() { # order_conf <name> <fp> <seedbase>
  cat > "$WORK/order_$1.conf" <<CONF
listen = 127.0.0.1:0
peer_addr = 127.0.0.1:$GW_PORT
unit = $UNIT
timeout_ms = 2000
network_id = 4f415349536e6574
our_fp = $2
our_seed_file = $W/$3.seed
peer_fp = $GW_FP
gateway_id = 1
peer_pk = $GW_FP,$GW_PUB
register = $REG_OK,50,100
CONF
}
order_conf agent  "$AGENT_FP"  agent
order_conf forged "$FORGED_FP" forged
order_conf noact  "$NOACT_FP"  noact

probe502() { timeout 5 bash -c "exec 3<>/dev/tcp/${INV%:*}/${INV##*:}" 2>/dev/null && echo OPEN || echo CLOSED; }

say ""
say "== inverter Modbus port before =="
B4="$(probe502)"; say "  ${INV}  $B4"

"$BIN/oasis_mbtcp_gateway" --config "$WORK/gateway.conf" --journal "$WORK/jrn" >> "$LOG" 2>&1 &
GWPID=$!; sleep 1.2

ord() { # ord <label> <conf> <reg> <value>
  say ""
  say "== $1 =="
  local o; o="$(timeout 8 "$BIN/oasis_mbtcp_order" --config "$WORK/order_$2.conf" --reg "$3" --value "$4" 2>&1)"; local rc=$?
  say "  $o   (exit $rc)"
}

# The one legitimate Act — guarded. Against a LIVE inverter with a placeholder register,
# refuse rather than risk a wrong write.
say ""
if [ "$B4" = OPEN ]; then
  say "== legitimate Act 60% — SKIPPED =="
  say "  inverter Modbus is OPEN: refusing to write the PLACEHOLDER register $REG_OK to a"
  say "  live inverter. Run oasis_sma_probe to confirm the real WMaxLimPct address first."
else
  ord "legitimate Act 60% (gateway decides Act, attempts the real inverter, cannot land: Modbus off)" agent "$REG_OK" 60
fi

ord "attack: value out of range 40% (< 50)" agent "$REG_OK" 40
ord "attack: register out of the map"       agent "$REG_BAD" 60
ord "attack: forged origin (key the gateway does not know)" forged "$REG_OK" 60
ord "attack: enrolled key without ACTUATE"  noact "$REG_OK" 60

say ""
say "== attack: raw Modbus FC06 straight to the gateway's port =="
RAW="$(timeout 6 bash -c 'exec 3<>/dev/tcp/127.0.0.1/'"$GW_PORT"'; printf "\x00\x01\x00\x00\x00\x06\x7e\x06\x9d\x2c\x00\x3c" >&3; head -c 16 <&3 | od -An -tx1' 2>&1)"
if [ -z "$RAW" ] || printf '%s' "$RAW" | grep -qi 'abort\|refus\|reset'; then
  say "  no usable reply — the non-v0B frame was refused by framing; nothing reached the inverter"
else
  say "  reply bytes: $RAW"
fi

sleep 0.3; kill "$GWPID" 2>/dev/null; GWPID=""

say ""
say "== inverter Modbus port after =="
say "  ${INV}  $(probe502)"

say ""
say "== the gateway's decision journal (the authoritative record) =="
cat "$WORK/jrn.head" "$WORK/jrn.entries" > "$OUT/journal_dump.txt" 2>/dev/null
"$BIN/oasis_journal_verify" "$(winpath "$OUT/journal_dump.txt")" 2>&1 | tee -a "$LOG"
say "  journal_verify exit above (0 = intact)"

say ""
say "== tamper control: flip one hex digit of one entry, verify must now fail =="
TD="$WORK/tampered.txt"
# Deterministic flip: on the first JRN_E line, swap the first entry hex digit 0<->1 (any
# other value becomes 0). A single changed digit breaks the hash chain.
awk '
  !done && /^JRN_E / {
    n = index($0, "JRN_E ") + 6
    c = substr($0, n, 1)
    nc = (c == "0") ? "1" : "0"
    $0 = substr($0, 1, n-1) nc substr($0, n+1)
    done = 1
  }
  { print }
' "$OUT/journal_dump.txt" > "$TD"
"$BIN/oasis_journal_verify" "$(winpath "$TD")" >> "$LOG" 2>&1; TRC=$?
say "  tampered journal_verify exit=$TRC (non-zero = detected)"

say ""
say "tree=$STAMP  done"
