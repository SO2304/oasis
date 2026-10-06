#!/usr/bin/env bash
# Assigns every #[kani::proof] harness in src/ to exactly one CI shard.
#   ./kani_shards.sh <shard>   print the harnesses of <shard>, one per line
#   ./kani_shards.sh --all     print "<shard> <harness>" for every harness
#   ./kani_shards.sh --check   fail unless every harness is in exactly one shard
# Run from oasis-rt/. Harness names are full paths, for `cargo kani --exact`.
set -euo pipefail

# Harnesses that do not finish within the normal 600 s budget (timeout or OOM
# on a hosted runner, 2026-10-06). Each runs alone, with a 60 min budget. They
# are NOT counted as verified until their shard is green.
HEAVY=(
  mesh::kani_proofs::proof_ae_snapshot_internal_consistency
  mesh::kani_proofs::proof_ae_capacity_consumed_monotonic
  mesh::kani_proofs::proof_ag_reset_count_extrapolation
  mesh::kani_proofs::proof_ah_arithmetic_tolerance_symmetric
  branching::kani_proofs::proof_m4_fitness_monotone_in_goal
  efference::kani_proofs::proof_m3_pain_monotone_in_magnitude
)

list_all() {
  local f mod
  for f in $(grep -rl 'kani::proof' src | sort); do
    mod=$(echo "$f" | sed -E 's#^src/##; s#/kani_proofs\.rs$##; s#\.rs$##; s#/#::#g')
    awk -v m="$mod" '
      /#\[kani::proof\]/ { want = 1; next }
      want && /fn [a-z0-9_]+/ { match($0, /fn [a-z0-9_]+/); print m "::kani_proofs::" substr($0, RSTART + 3, RLENGTH - 3); want = 0 }
    ' "$f"
  done
}

shard_of() {
  local h=$1 x
  for x in "${HEAVY[@]}"; do
    if [ "$h" = "$x" ]; then echo "heavy-${h##*::}"; return; fi
  done
  case "$h" in
    mesh::kani_proofs::proof_a[c-f]_*) echo mesh-ac-af ;;
    mesh::kani_proofs::proof_a[g-j]_*) echo mesh-ag-aj ;;
    mesh::kani_proofs::*) echo mesh-core-v0b ;;
    spinal::*|synapse::*) echo spinal-synapse ;;
    tx_lease::*|mesh_revocation::*|actuation::*|hal::*|hyper_state::*) echo authority ;;
    *) echo other-modules ;;
  esac
}

case "${1:-}" in
  --all)
    list_all | while read -r h; do echo "$(shard_of "$h") $h"; done ;;
  --check)
    all=$(list_all)
    n=$(echo "$all" | wc -l)
    dup=$(echo "$all" | sort | uniq -d)
    [ -z "$dup" ] || { echo "::error::duplicate harness names: $dup"; exit 1; }
    for x in "${HEAVY[@]}"; do
      echo "$all" | grep -qx "$x" || { echo "::error::heavy harness $x not found in src/"; exit 1; }
    done
    echo "$n harnesses, each in exactly one shard" ;;
  "")
    echo "usage: $0 <shard>|--all|--check" >&2; exit 2 ;;
  *)
    list_all | while read -r h; do [ "$(shard_of "$h")" = "$1" ] && echo "$h"; done ;;
esac
