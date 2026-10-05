#!/bin/bash
source /root/.cargo/env
cd /mnt/c/dev/oasis/oasis-rt
for p in \
  proof_transforms_compose_left_identity \
  proof_transforms_compose_right_identity \
  proof_transforms_inverse_of_identity_is_identity \
  proof_transforms_translation_inverse_cancels \
  proof_transforms_apply_identity_is_identity
do
  echo "=== $p ==="
  timeout 180 cargo kani --lib --harness "$p" 2>&1 | grep -E "VERIFICATION|Verification Time"
done
