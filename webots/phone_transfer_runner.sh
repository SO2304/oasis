#!/bin/bash
# Phone→Webots cross-morphology transfer test.
# Bar A: drone vierge (no phone brain)
# Bar B: drone inherits 128 phone pain memories via OASIS_PHONE_BRAIN
# Same scenario, same MAX_TICKS. Measure: stuck, bad_spots, loops, fear events.

WEBOTS="/c/Program Files/Webots/msys64/mingw64/bin/webots.exe"
WORLD="c:/dev/oasis/webots/worlds/oasis_factory.wbt"
OUT="c:/dev/oasis/webots/transfer_results"
PHONE="c:/dev/oasis/phone_brain"
MAX_TICKS=15000

mkdir -p $OUT
rm -f $OUT/*.log

for bar in vierge phone_brain; do
  for run in 1 2; do
    echo "=== BAR=$bar RUN=$run ==="
    taskkill //F //IM webots.exe //IM webots-bin.exe //IM drone_bridge.exe 2>/dev/null | head -1
    rm -f c:/dev/oasis/webots/factory_shared/*.json
    rm -f c:/dev/oasis/webots/factory_shared/*.bin
    rm -f c:/dev/oasis/webots/factory_*.log

    PHONE_ENV=""
    if [ "$bar" = "phone_brain" ]; then
      PHONE_ENV="OASIS_PHONE_BRAIN=$PHONE"
    fi

    env $PHONE_ENV \
      OASIS_STACK=full \
      OASIS_GATE_MODE=full \
      MAX_TICKS=$MAX_TICKS \
      "$WEBOTS" --mode=fast --minimize --no-rendering --batch "$WORLD" \
      > $OUT/webots_${bar}_run${run}.log 2>&1 &

    START=$(date +%s)
    while true; do
      sup=$(wc -l < c:/dev/oasis/webots/factory_supervisor.log 2>/dev/null || echo 0)
      elapsed=$(($(date +%s) - START))
      if [ "$sup" -gt "$MAX_TICKS" ] || [ $elapsed -gt 400 ]; then break; fi
      sleep 5
    done
    sleep 2
    taskkill //F //IM webots.exe //IM webots-bin.exe //IM drone_bridge.exe 2>/dev/null | head -1

    for drone in patrol1 patrol2 supervisor; do
      cp c:/dev/oasis/webots/factory_${drone}.log $OUT/log_${bar}_run${run}_${drone}.log 2>/dev/null
    done
  done
done

echo ""; echo "=== TRANSFER BATCH COMPLETE ==="
ls $OUT/log_*.log | wc -l
