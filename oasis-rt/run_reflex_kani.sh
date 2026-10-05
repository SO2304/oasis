#!/bin/bash
source /root/.cargo/env
cd /mnt/c/dev/oasis/oasis-rt
for p in \
  proof_reflex_uncalibrated_never_fires \
  proof_reflex_threshold_formula \
  proof_reflex_fires_when_above_baseline_zero_std
do
  echo "=== $p ==="
  timeout 180 cargo kani --lib --harness "$p" 2>&1 | grep -E "VERIFICATION|Verification Time"
done
