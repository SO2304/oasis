#!/bin/bash
# OASIS 3-PLATFORM MESH — Phone (Android Termux) + PC synthetic + Webots 3 drones.
# Requires phone daemon started manually: open Termux, run `bash /sdcard/launch-oasis.sh`

ADB="/c/Users/pc/AppData/Local/Temp/platform-tools/adb.exe"
SHARED="c:/dev/oasis/mesh/shared/"
BRIDGE="c:/dev/oasis/oasis-rt/target/release/drone_bridge.exe"
LOG_DIR="c:/dev/oasis/mesh/logs/"
WEBOTS_SHARED="c:/dev/oasis/webots/factory_shared/"

mkdir -p "$SHARED" "$LOG_DIR" "$WEBOTS_SHARED"
rm -f "$SHARED"*.bin "$SHARED"*.json "$LOG_DIR"*.log

echo "=== 1) Verify phone daemon is running ==="
if ! "$ADB" shell "pgrep -f oasis-rt" 2>/dev/null | grep -q "^[0-9]"; then
  echo "!!! PHONE DAEMON NOT RUNNING"
  echo "Please open Termux on phone and run: bash /sdcard/launch-oasis.sh"
  exit 1
fi
echo "  phone oasis-rt PIDs: $("$ADB" shell 'pgrep -f oasis-rt' 2>&1 | tr -d '\r' | head -1)"

gen_stream() {
  local x=$1 y=$2 ticks=${3:-10000}
  for i in $(seq 1 $ticks); do
    local r=$((i % 100))
    local obs=2.0; [ $r -lt 20 ] && obs=0.08
    echo "{\"tick\":$i,\"x\":$x,\"y\":$y,\"alt\":1.3,\"roll\":0,\"pitch\":0,\"gz\":0,\"rf\":$obs,\"rl\":2,\"rr\":2,\"rb\":2,\"vx\":0.01,\"vy\":0.01,\"imu_alive\":true,\"gps_alive\":true,\"sonar_alive\":true}"
    [ $((i % 50)) -eq 0 ] && sleep 0.1
  done
}

# Start PC synthetic instance (will peer with phone + Webots)
echo "=== 2) Launch PC synthetic node ==="
gen_stream -1.8 0.2 12000 | \
  OASIS_SHARED="$SHARED" \
  OASIS_MESH_PEERS="phone,patrol1,patrol2,supervisor" \
  "$BRIDGE" "pc-node" 0 \
  > "$LOG_DIR"pc-node.log 2>&1 &
PC_PID=$!
echo "  pc-node PID=$PC_PID"

# Phone ↔ mesh sync loop
echo "=== 3) Launch phone↔mesh sync (ADB pull/push every 5s) ==="
(
  while true; do
    # PULL phone digests → mesh (phone writes oasis-memory.bin every ~30s)
    "$ADB" pull //sdcard/oasis-memory.bin "$SHARED"phone_fed.bin 2>/dev/null | head -1 > /dev/null
    # PUSH mesh peer digests → phone inbox (phone reads every 10 ticks)
    # Combine pc-node + webots supervisor digests into one file (phone just reads first valid OASISMEM)
    if [ -f "$SHARED"pc-node_fed.bin ]; then
      "$ADB" push "$SHARED"pc-node_fed.bin //sdcard/oasis-spore-inbox.bin 2>&1 | tail -1 > /dev/null
    elif [ -f "$WEBOTS_SHARED"supervisor_fed.bin ]; then
      "$ADB" push "$WEBOTS_SHARED"supervisor_fed.bin //sdcard/oasis-spore-inbox.bin 2>&1 | tail -1 > /dev/null
    fi
    sleep 5
  done
) &
SYNC_PID=$!
echo "  phone sync PID=$SYNC_PID"

# Webots ↔ mesh relay (copy fed.bin files between factory_shared and mesh/shared)
echo "=== 4) Launch Webots↔mesh relay (every 3s) ==="
(
  while true; do
    [ -f "$WEBOTS_SHARED"supervisor_fed.bin ] && cp "$WEBOTS_SHARED"supervisor_fed.bin "$SHARED"webots-supervisor_fed.bin 2>/dev/null
    [ -f "$SHARED"phone_fed.bin ] && cp "$SHARED"phone_fed.bin "$WEBOTS_SHARED"phone_fed.bin 2>/dev/null
    [ -f "$SHARED"pc-node_fed.bin ] && cp "$SHARED"pc-node_fed.bin "$WEBOTS_SHARED"pc-node_fed.bin 2>/dev/null
    sleep 3
  done
) &
RELAY_PID=$!
echo "  relay PID=$RELAY_PID"

echo "=== 5) Launch Webots factory ==="
rm -f c:/dev/oasis/webots/factory_*.log
OASIS_MESH_PEERS="phone,pc-node" \
  "/c/Program Files/Webots/msys64/mingw64/bin/webots.exe" \
  --mode=fast --minimize --no-rendering --batch c:/dev/oasis/webots/worlds/oasis_factory.wbt \
  > "$LOG_DIR"webots.log 2>&1 &
WEBOTS_PID=$!
echo "  Webots PID=$WEBOTS_PID"

echo ""; echo "=== MESH LIVE — monitoring 120s ==="
for iter in $(seq 1 12); do
  sleep 10
  echo "--- T=${iter}0s ---"
  # pc-node
  if [ -f "$LOG_DIR"pc-node.log ]; then
    m=$(grep -c "MESH merged" "$LOG_DIR"pc-node.log)
    d=$(grep -oE "dig:[0-9]+" "$LOG_DIR"pc-node.log | tail -1)
    echo "  pc-node: merges=$m $d"
  fi
  # Phone (via log on /sdcard)
  phone_log=$("$ADB" shell "tail -5 //sdcard/oasis-mesh.log 2>&1 | grep -iE 'SPORE|FED|Memory' | tail -3" 2>&1 | tr -d '\r')
  echo "  phone: $(echo "$phone_log" | tr '\n' ' | ')"
  # Webots drones
  for drone in patrol1 patrol2 supervisor; do
    f="c:/dev/oasis/webots/factory_${drone}.log"
    if [ -f "$f" ]; then
      m=$(grep -c "MESH merged" "$f")
      d=$(grep -oE "dig:[0-9]+" "$f" | tail -1)
      echo "  webots-$drone: merges=$m $d"
    fi
  done
done

echo; echo "=== CLEANUP ==="
kill $PC_PID $SYNC_PID $RELAY_PID 2>/dev/null
taskkill //F //IM webots.exe //IM webots-bin.exe //IM drone_bridge.exe 2>/dev/null | head -2

echo; echo "=== FINAL CROSS-PLATFORM MERGE EVENTS ==="
grep "MESH merged\|SPORE received" "$LOG_DIR"*.log c:/dev/oasis/webots/factory_*.log 2>/dev/null | head -15
echo
echo "Phone daemon still running on phone. Don't forget to kill it via Termux if done."
