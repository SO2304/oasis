#!/bin/bash
source /root/.cargo/env
cd /mnt/c/dev/oasis/oasis-rt
for p in \
  proof_mesh_v9_magic_distinct_from_v8 \
  proof_mesh_v9_header_length_arithmetic \
  proof_mesh_hmac_tag_length_is_8
do
  echo "=== $p ==="
  timeout 300 cargo kani --lib --harness "$p" 2>&1 | grep -E "VERIFICATION|Verification Time"
done
