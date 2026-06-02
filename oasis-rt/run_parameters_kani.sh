#!/bin/bash
source /root/.cargo/env
cd /mnt/c/dev/oasis/oasis-rt
for p in \
  proof_params_clamp_float_respects_bounds \
  proof_params_clamp_float_unbounded_is_identity \
  proof_params_clamp_float_idempotent \
  proof_params_clamp_int_respects_bounds \
  proof_params_clamp_int_in_bounds_is_identity
do
  echo "=== $p ==="
  timeout 240 cargo kani --lib --harness "$p" 2>&1 | grep -E "VERIFICATION|Verification Time"
done
