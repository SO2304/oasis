#!/usr/bin/env bash
# check_claims.sh — re-derive every countable claim in CLAUDE.md from the tree and fail
# on drift. Run from the repo root.
#
# Why this exists: in two days the repo was found claiming numbers the tree did not
# support — a harness list summing to 115 against 182, two test chains at 621 and 624
# against 623, a gate condition count wrong in nine documents — and, four times, claiming
# LESS than it had proven: pre-filter harnesses marked "written but NOT RUN" when the run
# existed, fuzzing marked "planned" when the campaign was checksummed, "nothing on PX4
# SITL" after it ran, and firmware sizes marked "NOT measured" when `size` answers in a
# second. Understating is as wrong as overstating, and both are cheap to catch.
#
#   ./tools/check_claims.sh            # counts only (fast)
#   ./tools/check_claims.sh --full     # + run the test suites and verify every SHA256SUMS
set -u
cd "$(dirname "$0")/.." || exit 2
FULL=0
[ "${1:-}" = "--full" ] && FULL=1
fail=0

claim() { # claim <label> <expected-in-doc> <actual>
  if [ "$2" = "$3" ]; then
    printf '  ok    %-34s %s\n' "$1" "$3"
  else
    printf '  DRIFT %-34s document dit %s, arbre dit %s\n' "$1" "$2" "$3"
    fail=$((fail + 1))
  fi
}

doc_num() { # doc_num <regex with one capture>
  grep -oE "$1" CLAUDE.md | head -1 | grep -oE '[0-9]+' | head -1
}

echo "== comptes de l'arbre =="
claim "harnais Kani" \
  "$(grep -oE '\*\*[0-9]+ Kani proof harnesses\*\*' CLAUDE.md | head -1 | grep -oE '[0-9]+')" \
  "$(grep -rE 'kani::proof' oasis-rt/src | wc -l | tr -d ' ')"
claim "fichiers src/*.rs" \
  "$(grep -oE '[*][*][0-9]+ .src/[*][.]rs.[*][*]' CLAUDE.md | head -1 | grep -oE '[0-9]+')" \
  "$(ls oasis-rt/src/*.rs | wc -l | tr -d ' ')"
claim "[[bin]] dans Cargo.toml" \
  "$(grep -oE '[*][*][0-9]+ .\[\[bin\]\].[*][*]' CLAUDE.md | head -1 | grep -oE '[0-9]+')" \
  "$(grep -c '^\[\[bin\]\]' oasis-rt/Cargo.toml | tr -d ' ')"
claim "modules declares dans lib.rs" \
  "$(grep -oE '\*\*[0-9]+ modules\*\*' CLAUDE.md | head -1 | grep -oE '[0-9]+')" \
  "$(grep -cE '^pub mod ' oasis-rt/src/lib.rs | tr -d ' ')"
claim "features Cargo" \
  "$(grep -oE 'Cargo features \([0-9]+\)' CLAUDE.md | grep -oE '[0-9]+')" \
  "$(awk '/^\[features\]/{f=1;next} f&&/^\[/{exit} f&&/^[a-z0-9_]+ *=/&&!/^default *=/{n++} END{print n+0}' oasis-rt/Cargo.toml | tr -d ' ')"

echo
echo "== sommes des repartitions (une liste qui ne somme pas est une liste fausse) =="
hs=$(grep -oE '\*\*[0-9]+ Kani proof harnesses\*\* \([^)]*\)' CLAUDE.md | head -1)
if [ -n "$hs" ]; then
  # One item per comma, each starting with its count. A naive grep over the whole
  # parenthetical caught version numbers too and reported 393 for 182.
  s=$(echo "$hs" | sed 's/^[^(]*(//; s/)$//' | tr ',' '
' | sed -E 's/^ *(and|et|puis) +//' | grep -oE '^ *[0-9]+' | awk '{t+=$1} END{print t+0}')
  tot=$(grep -rE 'kani::proof' oasis-rt/src | wc -l | tr -d ' ')
  claim "repartition des harnais somme a" "$tot" "${s:-?}"
fi

echo
echo "== affirmations d'absence a reverifier a la main =="
pat="planned|NOT RUN|not run|jamais exécuté|jamais tourné|NOT measured|non mesuré|est un stub|LoRa frame stub|pas encore testé|non testé"
hits=$(grep -rnoiE "$pat" CLAUDE.md partners/*.md partners/*.html docs/*.md SECURITY.md 2>/dev/null \
       | grep -viE "no radio|aucune radio|pas encore applicable|n'a jamais volé" | head -20)
if [ -z "$hits" ]; then
  echo "  aucune"
else
  echo "$hits" | sed 's/^/  revoir: /'
  echo "  (ce ne sont pas des erreurs par construction — chacune doit etre confrontee a l'arbre)"
fi

echo
echo "== empreinte des firmwares (le 'NOT measured' n'a pas de raison d'exister) =="
D=oasis-silicon-test/target/thumbv6m-none-eabi/release
if command -v size >/dev/null 2>&1 && [ -d "$D" ]; then
  for f in oasis-silicon-test uart_mesh modbus_device pq_bench; do
    [ -f "$D/$f" ] || continue
    size -A -d "$D/$f" 2>/dev/null | awk -v n="$f" '
      $1==".text"{t+=$2} $1==".rodata"{t+=$2} $1==".data"{d+=$2} $1==".bss"{b+=$2}
      END {printf "  %-22s flash %4d Kio   RAM %4d Kio\n", n, t/1024, (d+b)/1024}'
  done
else
  echo "  (pas de binutils 'size' ou pas de build thumbv6m: ignore)"
fi

if [ "$FULL" = 1 ]; then
  echo
  echo "== suites (--full) =="
  lib=$(cargo test -p oasis-rt --release --lib -- --list 2>/dev/null | grep -c ': test$')
  claim "tests lib oasis-rt" \
    "$(grep -oE '[*][*][0-9]+ .oasis-rt. lib tests' CLAUDE.md | head -1 | grep -oE '[0-9]+' | head -1)" "$lib"
  echo
  echo "== manifestes de preuves (--full) =="
  # The working tree is not the authority: a checkout can carry local divergence that
  # git itself does not report (two 2026-10-04 logs have 10 extra bytes on one machine
  # while the blobs match the manifest, and all 27 verify in a fresh clone). So a
  # working-tree failure is re-tested against the blobs before it is called a break.
  n=0; bad=0; local_only=0
  while IFS= read -r m; do
    n=$((n + 1))
    d=$(dirname "$m")
    if ( cd "$d" && sha256sum -c SHA256SUMS --quiet >/dev/null 2>&1 ); then
      continue
    fi
    # Same manifest, blob content.
    blob_bad=0
    while read -r want name; do
      name=${name#\*}
      got=$(git show "HEAD:$d/$name" 2>/dev/null | sha256sum | cut -d' ' -f1)
      [ "$got" = "$want" ] || blob_bad=$((blob_bad + 1))
    done < "$m"
    if [ "$blob_bad" = 0 ]; then
      echo "  local $d (disque divergent, blobs conformes — verifier dans un clone frais)"
      local_only=$((local_only + 1))
    else
      echo "  ECHEC $d ($blob_bad fichier(s) dont le blob ne correspond pas)"
      bad=$((bad + 1))
    fi
  done < <(find evidence -name SHA256SUMS 2>/dev/null)
  echo "  ok    $((n - bad - local_only))/$n manifestes verifies dans l arbre de travail"
  [ "$local_only" = 0 ] || echo "        + $local_only avec divergence locale seulement (blobs conformes)"
  if [ "$bad" != 0 ]; then
    echo "  $bad manifeste(s) reellement casse(s)"
    fail=$((fail + bad))
  fi
fi

echo
if [ "$fail" = 0 ]; then
  echo "AUCUNE DERIVE sur les claims verifiables."
else
  echo "$fail DERIVE(S). Corriger le document, pas le compte."
fi
exit "$fail"
