#!/bin/bash
source /root/.cargo/env
cd /mnt/c/dev/oasis/oasis-rt
for p in \
  proof_timers_not_due_does_not_fire \
  proof_timers_due_fires_and_rearms \
  proof_timers_no_immediate_refire \
  proof_timers_coalesces_missed_periods \
  proof_timers_fire_count_saturates
do
  echo "=== $p ==="
  timeout 180 cargo kani --lib --harness "$p" 2>&1 | grep -E "VERIFICATION|Verification Time"
done
