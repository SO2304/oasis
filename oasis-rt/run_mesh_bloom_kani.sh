#!/bin/bash
source /root/.cargo/env
cd /mnt/c/dev/oasis/oasis-rt
for p in \
  proof_mesh_bloom_bit_set_then_test_true \
  proof_mesh_bloom_empty_contains_nothing \
  proof_mesh_bloom_or_idempotent \
  proof_mesh_bloom_bit_index_bounded
do
  echo "=== $p ==="
  timeout 300 cargo kani --lib --harness "$p" 2>&1 | grep -E "VERIFICATION|Verification Time"
done
