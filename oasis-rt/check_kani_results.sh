#!/bin/bash
# OASIS — Summarize Kani results after `run_all_kani.sh` finished (or while
# it's still running). Prints one line per proof with verdict + wall time.
#
# Usage (inside WSL Ubuntu):
#   ./check_kani_results.sh           # summary only
#   ./check_kani_results.sh -v        # show full log for failed/timeout

LOG_DIR=/root/oasis_kani_logs
VERBOSE=${1:-}

if [ ! -d "$LOG_DIR" ]; then
  echo "no logs yet at $LOG_DIR — run ./run_all_kani.sh first"
  exit 1
fi

echo "=== Kani batch summary ==="
pass=0; fail=0; timeout=0; running=0
for log in "$LOG_DIR"/*.log; do
  [ -f "$log" ] || continue
  name=$(basename "$log" .log)
  if grep -q "VERIFICATION:- SUCCESSFUL" "$log"; then
    verdict="✅ SUCCESS"
    t=$(grep "Verification Time:" "$log" | tail -1 | awk '{print $3}')
    echo "  $verdict  $name  ($t)"
    pass=$((pass + 1))
  elif grep -q "VERIFICATION:- FAILED" "$log"; then
    verdict="❌ FAIL"
    echo "  $verdict  $name"
    fail=$((fail + 1))
    if [ "$VERBOSE" = "-v" ]; then
      echo "     --- failed assertions ---"
      grep -A 1 "Failed Checks" "$log" | sed 's/^/     /'
    fi
  elif grep -q "Verification Time:" "$log"; then
    # Kani produced output but no final verdict — partial run
    verdict="⚠️  RUNNING"
    echo "  $verdict  $name"
    running=$((running + 1))
  else
    verdict="⏱  TIMEOUT/ERROR"
    echo "  $verdict  $name"
    timeout=$((timeout + 1))
  fi
done
echo
echo "Total: $pass pass / $fail fail / $timeout timeout-or-error / $running still-running"
