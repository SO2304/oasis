#!/bin/bash
source /root/.cargo/env
cd /mnt/c/dev/oasis/oasis-rt
for p in \
  proof_synapse_reinforce_weight_bounded \
  proof_synapse_decay_non_expanding \
  proof_synapse_prune_symmetric \
  proof_synapse_below_threshold_prunes \
  proof_synapse_zero_reward_preserves_weight
do
  echo "=== $p ==="
  timeout 180 cargo kani --lib --harness "$p" 2>&1 | grep -E "VERIFICATION|Verification Time"
done
