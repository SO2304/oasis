#!/bin/bash
source /root/.cargo/env
cd /mnt/c/dev/oasis/oasis-rt
for p in \
  proof_m1_ttl_monotone_non_increasing \
  proof_m1_deactivate_iff_zero \
  proof_m1_tick_terminates \
  proof_m11_hex_digit_range_and_rejection \
  proof_m11_trust_clamp_bounded
do
  echo "=== $p ==="
  timeout 180 cargo kani --lib --harness "$p" 2>&1 | grep -E "VERIFICATION|Verification Time"
done
