#!/bin/bash
# OASIS A/B: simple_nav vs full_oasis — both with bad_spots spatial memory.
# Pre-registered metrics: full_loops per drone, crashes, inspections, stuck, bad_spots.
# N=3 per variant, MAX_TICKS=25000 (enough to see multiple loops develop).

WEBOTS="/c/Program Files/Webots/msys64/mingw64/bin/webots.exe"
WORLD="c:/dev/oasis/webots/worlds/oasis_factory.wbt"
OUT="c:/dev/oasis/webots/ab_results"
MAX_TICKS=25000

mkdir -p $OUT
rm -f $OUT/*.log $OUT/*.csv

for stack in simple_nav full; do
  for run in 1 2 3; do
    echo "=== STACK=$stack RUN=$run ==="
    taskkill //F //IM webots.exe //IM webots-bin.exe //IM drone_bridge.exe 2>/dev/null | head -1
    rm -f c:/dev/oasis/webots/factory_shared/*.json
    rm -f c:/dev/oasis/webots/factory_shared/*.bin
    rm -f c:/dev/oasis/webots/factory_*.log
    rm -f c:/dev/oasis/webots/metrics_*.csv

    OASIS_STACK=$stack \
      OASIS_GATE_MODE=full \
      OASIS_RUN_SEED=$run \
      MAX_TICKS=$MAX_TICKS \
      "$WEBOTS" --mode=fast --minimize --no-rendering --batch "$WORLD" \
      > $OUT/webots_${stack}_run${run}.log 2>&1 &

    START=$(date +%s)
    while true; do
      sup_lines=$(wc -l < c:/dev/oasis/webots/factory_supervisor.log 2>/dev/null || echo 0)
      elapsed=$(($(date +%s) - START))
      if [ "$sup_lines" -gt "$MAX_TICKS" ] || [ $elapsed -gt 600 ]; then break; fi
      sleep 5
    done
    sleep 2
    taskkill //F //IM webots.exe //IM webots-bin.exe //IM drone_bridge.exe 2>/dev/null | head -1

    # Archive logs
    for drone in patrol1 patrol2 supervisor; do
      cp c:/dev/oasis/webots/factory_${drone}.log $OUT/log_${stack}_run${run}_${drone}.log 2>/dev/null
    done
    echo "  done"
  done
done

echo
echo "=== BATCH COMPLETE ==="
ls $OUT/log_*.log | wc -l
