#!/bin/bash
# OASIS — Extended-budget Kani runs for the 2 FP-heavy proofs that timed out
# at their default budget. These are known-hard floating-point SMT problems;
# we give them hours, not minutes. Results land in /root/oasis_kani_logs_long/.
#
# Both are MONOTONICITY proofs on weighted f64 arithmetic — notoriously slow
# for CBMC/SAT because the solver has to reason about real-valued ordering
# under bounded float inputs.
#
# Usage (inside WSL Ubuntu):
#   source /root/.cargo/env
#   cd /mnt/c/dev/oasis/oasis-rt
#   setsid nohup bash ./run_kani_long.sh > /root/oasis_kani_logs_long/_stdout.log 2>&1 < /dev/null &
#   disown

set -u
export CARGO_TARGET_DIR=/root/oasis_kani_target
LOG_DIR=/root/oasis_kani_logs_long
mkdir -p "$LOG_DIR"
SUMMARY="$LOG_DIR/_summary.txt"
echo "=== OASIS Kani LONG batch — started $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" > "$SUMMARY"

# Proofs with generous budgets.
# M3 monotone_in_magnitude: 4 hours (14 400 s)
# M4 fitness_monotone_in_goal: 2 hours (7 200 s)
PROOFS=(
  "proof_m3_pain_monotone_in_magnitude 14400"
  "proof_m4_fitness_monotone_in_goal 7200"
)

for entry in "${PROOFS[@]}"; do
  name=${entry% *}
  budget=${entry##* }
  log="$LOG_DIR/${name}.log"
  echo "[$(date -u +%H:%M:%S)] Running $name (budget=${budget}s)..." | tee -a "$SUMMARY"
  start=$(date +%s)
  timeout "$budget" cargo kani --lib --harness "$name" > "$log" 2>&1
  exit_code=$?
  elapsed=$(( $(date +%s) - start ))
  if grep -q "VERIFICATION:- SUCCESSFUL" "$log"; then
    verdict="SUCCESS"
  elif grep -q "VERIFICATION:- FAILED" "$log"; then
    verdict="FAIL"
  elif [ "$exit_code" = "124" ]; then
    verdict="TIMEOUT(${budget}s)"
  else
    verdict="ERROR(exit=$exit_code)"
  fi
  echo "  => $name: $verdict (${elapsed}s)" | tee -a "$SUMMARY"
done

echo "=== Done $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$SUMMARY"
