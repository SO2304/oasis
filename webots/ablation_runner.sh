#!/bin/bash
# OASIS Ablation Study — 4 gate modes × 3 seeds = 12 runs.
# Each run: patrol1 with fault injection schedule, capped at MAX_TICKS.
# Output: metrics_patrol1_<mode>_seed<N>.csv per run.

WEBOTS="/c/Program Files/Webots/msys64/mingw64/bin/webots.exe"
WORLD="c:/dev/oasis/webots/worlds/oasis_factory.wbt"
OUT="c:/dev/oasis/webots/ablation_results"
MAX_TICKS=4500   # covers sonar fault (500-1500), gps/imu (2000-3500), post-recovery

mkdir -p $OUT
rm -f $OUT/*.csv $OUT/*.log

for mode in none static adaptive_nocouple full; do
  for seed in 1; do  # N=1 for now (Webots deterministic; replicates would be identical)
    echo "=== MODE=$mode SEED=$seed ==="
    # Kill any prior instance
    taskkill //F //IM webots.exe //IM webots-bin.exe //IM drone_bridge.exe 2>/dev/null | head -1

    rm -f c:/dev/oasis/webots/factory_shared/*.json
    rm -f c:/dev/oasis/webots/factory_shared/*.bin
    rm -f c:/dev/oasis/webots/factory_*.log
    rm -f c:/dev/oasis/webots/metrics_*.csv

    # Launch Webots with this configuration
    FAULT_INJECT=1 \
      OASIS_GATE_MODE=$mode \
      OASIS_RUN_SEED=$seed \
      MAX_TICKS=$MAX_TICKS \
      "$WEBOTS" --mode=fast --minimize --no-rendering --batch "$WORLD" \
      > $OUT/webots_${mode}_seed${seed}.log 2>&1 &
    WPID=$!

    # Wait for metrics CSV to fill to MAX_TICKS worth of rows, then kill
    START=$(date +%s)
    while true; do
      rows=$(wc -l < c:/dev/oasis/webots/metrics_patrol1_${mode}_seed${seed}.csv 2>/dev/null || echo 0)
      if [ "$rows" -gt "$MAX_TICKS" ]; then break; fi
      elapsed=$(($(date +%s) - START))
      if [ $elapsed -gt 180 ]; then echo "  TIMEOUT"; break; fi
      sleep 3
    done

    # Grace period, then kill
    sleep 2
    taskkill //F //IM webots.exe //IM webots-bin.exe //IM drone_bridge.exe 2>/dev/null | head -1

    # Copy outputs
    cp c:/dev/oasis/webots/metrics_patrol1_${mode}_seed${seed}.csv $OUT/ 2>/dev/null
    cp c:/dev/oasis/webots/factory_patrol1.log $OUT/log_${mode}_seed${seed}.log 2>/dev/null
    echo "  done: $(wc -l < c:/dev/oasis/webots/metrics_patrol1_${mode}_seed${seed}.csv 2>/dev/null) rows"
  done
done

echo
echo "=== BATCH COMPLETE ==="
ls -la $OUT/*.csv | head
