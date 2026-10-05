#!/bin/bash
# OASIS — Run every #[kani::proof] harness and log the verdict to
# /root/oasis_kani_logs/<harness>.log. Rounds with expected multi-minute
# solver time (M3 pain monotone_in_magnitude, M10 field_linear) get a
# generous wall-clock budget of 30 min each; faster proofs finish quickly.
#
# Usage (inside WSL Ubuntu):
#   source /root/.cargo/env
#   cd /mnt/c/dev/oasis/oasis-rt
#   ./run_all_kani.sh
#
# Output:
#   /root/oasis_kani_logs/<harness>.log         — full Kani output per proof
#   /root/oasis_kani_logs/_summary.txt          — one line per proof with verdict
#
# Call ./check_kani_results.sh to view verdicts only (not yet written to disk).

set -u
export CARGO_TARGET_DIR=/root/oasis_kani_target
LOG_DIR=/root/oasis_kani_logs
mkdir -p "$LOG_DIR"
SUMMARY="$LOG_DIR/_summary.txt"
echo "=== OASIS Kani batch — started $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" > "$SUMMARY"

# Proofs to run. Each entry: <harness-name> <soft-timeout-seconds>
# Fast proofs (<10s): R14 + M3 bounded/nonincreasing + M5 + M6 + M8
# Medium proofs (<5min): M4 fitness, M10 sign
# Long (up to 30min): M3 monotone, M10 linear
PROOFS=(
  # R14 (already proven; re-confirm)
  "proof_r14_monotonic 60"
  "proof_r14_boundary_strict 60"
  "proof_r14_determinism 60"
  # M3 Efference
  "proof_m3_pain_bounded 60"
  "proof_m3_pain_nonincreasing_when_idle 120"
  "proof_m3_pain_monotone_in_magnitude 1800"
  # M4 Branching
  "proof_m4_fitness_bounded_in_unit_interval 300"
  "proof_m4_fitness_monotone_in_goal 300"
  # M5 Emotion
  "proof_m5_fear_bounded 60"
  "proof_m5_fear_idempotent 60"
  "proof_m5_fear_monotone 120"
  # M6 Morphogenesis (enum state machine — fast)
  "proof_m6_stem_terminal_after_first_diff 60"
  "proof_m6_stem_start_is_always_valid 60"
  # M8 Dreams
  "proof_m8_force_trigger_upper_bound 120"
  "proof_m8_no_retrigger_at_same_tick 120"
  # M10 World model
  "proof_m10_repulsive_points_away 600"
  "proof_m10_attractive_points_toward 600"
  "proof_m10_field_linear 1800"
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
    verdict="TIMEOUT"
  else
    verdict="ERROR(exit=$exit_code)"
  fi
  echo "  => $name: $verdict (${elapsed}s)" | tee -a "$SUMMARY"
done

echo "=== Done $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$SUMMARY"
