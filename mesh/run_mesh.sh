#!/bin/bash
# OASIS 3-instance mesh demo.
# Instance A: Webots drone_bridge (real sim)
# Instance B: PC synthetic "pc-node" (stuck/inspect events → digests)
# Instance C: PC synthetic "phone-node" (stuck/inspect events → digests)
# All 3 share C:/dev/oasis/mesh/shared/. Each reads the 2 others via OASIS_MESH_PEERS.

SHARED="c:/dev/oasis/mesh/shared/"
BRIDGE="c:/dev/oasis/oasis-rt/target/release/drone_bridge.exe"
LOG_DIR="c:/dev/oasis/mesh/logs/"

mkdir -p "$SHARED" "$LOG_DIR"
rm -f "$SHARED"*.bin "$SHARED"*.json "$LOG_DIR"*.log

# Synthetic sensor stream generator (triggers STUCK + INSPECT events for digest production)
gen_stream() {
  local name=$1
  local target_x=$2
  local target_y=$3
  local ticks=${4:-6000}
  for i in $(seq 1 $ticks); do
    # Oscillation near target + occasional close obstacle (triggers pain+stuck)
    local r=$((i % 100))
    local x="$target_x"
    local y="$target_y"
    local obs=2.0
    if [ $r -lt 20 ]; then obs=0.08; fi  # Near obstacle → stuck detection fires
    echo "{\"tick\":$i,\"x\":$x,\"y\":$y,\"alt\":1.3,\"roll\":0,\"pitch\":0,\"gz\":0,\"rf\":$obs,\"rl\":2,\"rr\":2,\"rb\":2,\"vx\":0.001,\"vy\":0.001,\"imu_alive\":true,\"gps_alive\":true,\"sonar_alive\":true}"
    # Pace to allow merge cycles (tick 200 merges)
    if [ $((i % 50)) -eq 0 ]; then sleep 0.1; fi
  done
}

# Launch PC synthetic instances (A + B). Each one reads the others via OASIS_MESH_PEERS.
# We use the 'patrol1' targets for simplicity (4 targets, frequent inspects).
echo "=== Launching pc-node ==="
gen_stream "pc-node" -1.5 -0.5 8000 | \
  OASIS_SHARED="$SHARED" \
  OASIS_MESH_PEERS="phone-node,patrol1,patrol2,supervisor" \
  "$BRIDGE" "pc-node" 0 \
  > "$LOG_DIR"pc-node.log 2>&1 &
PC_PID=$!
echo "  pc-node PID=$PC_PID"

sleep 2

echo "=== Launching phone-node (simulated on PC) ==="
gen_stream "phone-node" -1.8 0.2 8000 | \
  OASIS_SHARED="$SHARED" \
  OASIS_MESH_PEERS="pc-node,patrol1,patrol2,supervisor" \
  "$BRIDGE" "phone-node" 1 \
  > "$LOG_DIR"phone-node.log 2>&1 &
PHONE_PID=$!
echo "  phone-node PID=$PHONE_PID"

sleep 2

# Instance C: real Webots factory (patrol1/2/supervisor write to factory_shared,
# which is DIFFERENT from our mesh shared. Need a bridge: copy Webots supervisor_fed → mesh/)
echo "=== Launching Webots factory (3 drones on real sim) ==="
WEBOTS_SHARED="c:/dev/oasis/webots/factory_shared/"
mkdir -p "$WEBOTS_SHARED"
rm -f "$WEBOTS_SHARED"*

# Relay: every 3 seconds, copy webots supervisor_fed.bin to mesh/ AND mesh phone-node_fed.bin → webots factory_shared
(while true; do
  # Webots → mesh: supervisor's digests as a 4th peer
  [ -f "$WEBOTS_SHARED"supervisor_fed.bin ] && cp "$WEBOTS_SHARED"supervisor_fed.bin "$SHARED"webots-supervisor_fed.bin 2>/dev/null
  # Mesh → Webots: phone-node's + pc-node's digests as extra peers for Webots drones
  [ -f "$SHARED"phone-node_fed.bin ] && cp "$SHARED"phone-node_fed.bin "$WEBOTS_SHARED"phone-node_fed.bin 2>/dev/null
  [ -f "$SHARED"pc-node_fed.bin ] && cp "$SHARED"pc-node_fed.bin "$WEBOTS_SHARED"pc-node_fed.bin 2>/dev/null
  sleep 3
done) &
RELAY_PID=$!
echo "  Webots↔mesh relay PID=$RELAY_PID (every 3s)"

# Launch Webots with MESH_PEERS including phone-node and pc-node
OASIS_MESH_PEERS="phone-node,pc-node" \
  "/c/Program Files/Webots/msys64/mingw64/bin/webots.exe" \
  --mode=fast --minimize --no-rendering --batch c:/dev/oasis/webots/worlds/oasis_factory.wbt \
  > "$LOG_DIR"webots.log 2>&1 &
WEBOTS_PID=$!
echo "  Webots PID=$WEBOTS_PID"

echo
echo "=== MESH LIVE. Monitoring every 10 sec for 120 sec ==="
for iter in $(seq 1 12); do
  sleep 10
  echo "--- T=${iter}0s ---"
  for node in pc-node phone-node; do
    if [ -f "$LOG_DIR$node.log" ]; then
      merges=$(grep -c "MESH merged" "$LOG_DIR$node.log")
      dig=$(grep "dig:" "$LOG_DIR$node.log" | tail -1 | grep -oE "dig:[0-9]+" | head -1)
      echo "  $node: mesh_merges=$merges, latest_$dig"
    fi
  done
  # Webots drones
  for drone in patrol1 patrol2 supervisor; do
    f="c:/dev/oasis/webots/factory_${drone}.log"
    if [ -f "$f" ]; then
      merges=$(grep -c "MESH merged" "$f")
      dig=$(grep "dig:" "$f" | tail -1 | grep -oE "dig:[0-9]+" | head -1)
      echo "  webots-$drone: mesh_merges=$merges, $dig"
    fi
  done
done

# Cleanup
echo
echo "=== CLEANUP ==="
kill $PC_PID $PHONE_PID $RELAY_PID 2>/dev/null
taskkill //F //IM webots.exe //IM webots-bin.exe //IM drone_bridge.exe 2>/dev/null | head -2
echo "Done. Logs in $LOG_DIR"
