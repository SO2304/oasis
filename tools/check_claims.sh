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

# Same check, in a named document rather than CLAUDE.md. Added after a count was corrected
# in CLAUDE.md and left wrong in README.md on a public repository for an hour: the checker
# only ever read CLAUDE.md, so the one file a visitor sees first was the one file not
# checked. A document that does not carry the count at all is reported, not passed.
claim_in() { # claim_in <label> <file> <regex> <actual>
  local got
  got=$(grep -oE "$3" "$2" 2>/dev/null | head -1 | grep -oE '[0-9]+' | head -1)
  if [ -z "$got" ]; then
    printf '  ABSENT %-33s %s ne porte pas ce compte\n' "$1" "$2"
    fail=$((fail + 1))
    return
  fi
  claim "$1" "$got" "$4"
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
echo "== les memes comptes dans README.md (le premier fichier qu'un visiteur ouvre) =="
claim_in "README modules" README.md '[0-9]+ modules' \
  "$(grep -cE '^pub mod ' oasis-rt/src/lib.rs | tr -d ' ')"
claim_in "README bins" README.md '[0-9]+ bins' \
  "$(grep -c '^\[\[bin\]\]' oasis-rt/Cargo.toml | tr -d ' ')"
claim_in "README harnais Kani" README.md '[0-9]+ Kani proof' \
  "$(grep -rE 'kani::proof' oasis-rt/src | wc -l | tr -d ' ')"

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
echo "== chaque membre du workspace est copie dans le Dockerfile =="
# Cargo parses the whole workspace manifest before compiling anything, so a member the
# Dockerfile does not COPY fails the image build before a crate is touched. That broke CI
# twice in one day (oasis-operator-key, then oasis-test-plc), both times only visible on
# a push. Checked here instead.
miss=0; n=0
for p in $(awk '/^members = \[/{f=1;next} f&&/^\]/{exit} f&&/^[[:space:]]*#/{next} f&&/"/{gsub(/[",]/,"");print $1}' Cargo.toml); do
  n=$((n + 1))
  grep -q "^COPY $p " oasis-rt/Dockerfile || { echo "  MANQUE COPY $p dans oasis-rt/Dockerfile"; miss=$((miss + 1)); }
done
if [ "$miss" = 0 ]; then
  printf '  ok    %-34s %s membres
' "membres copies dans Dockerfile" "$n"
else
  printf '  DRIFT %-34s %s membre(s) absent(s)
' "membres copies dans Dockerfile" "$miss"
  fail=$((fail + 1))
fi

echo
echo "== inventaire 1.1.9 al. 3 : chaque chemin cite doit exister =="
# Annex III 1.1.9 para 3 wants the safety-critical software "identified as such". A list
# written once drifts into fiction the first time a module is renamed, so every path in
# docs/compliance/SOFTWARE_INVENTORY.md is checked against the tree. The judgement of what
# is critical stays human; only its existence is mechanical.
INV=docs/compliance/SOFTWARE_INVENTORY.md
if [ -f "$INV" ]; then
  n=0; miss=0
  # First table cell of each row, when it is a path in backticks.
  while IFS= read -r p; do
    [ -n "$p" ] || continue
    n=$((n + 1))
    if [ ! -e "$p" ]; then
      echo "  MANQUE $p"
      miss=$((miss + 1))
    fi
  done < <(grep -oE '^\| `[A-Za-z0-9_./-]+`' "$INV" | sed 's/^| `//; s/`$//' | sort -u)
  if [ "$miss" = 0 ]; then
    printf '  ok    %-34s %s chemins verifies\n' "inventaire 1.1.9 al. 3" "$n"
  else
    printf '  DRIFT %-34s %s chemin(s) absent(s) sur %s\n' "inventaire 1.1.9 al. 3" "$miss" "$n"
    fail=$((fail + 1))
  fi
else
  printf '  ABSENT %-33s %s\n' "inventaire 1.1.9 al. 3" "$INV"
  fail=$((fail + 1))
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
  claim_in "tests lib oasis-rt (CLAUDE)" CLAUDE.md '[*][*][0-9]+ .oasis-rt. lib tests' "$lib"
  claim_in "tests lib oasis-rt (README)" README.md '[0-9]+ lib tests' "$lib"

  # The workspace figure is what the README tells a visitor to run, so it is measured the
  # way they would see it: the number of tests that PASS. `-- --list` counts declared
  # tests, which is 2 higher here because two doc-test blocks are marked `ignore` — and
  # claiming the declared count as passing is how "689" got published against 686.
  ws=$(cargo test --workspace --release 2>/dev/null \
       | awk '/^test result/{p+=$4} END{print p+0}')
  claim_in "tests workspace (CLAUDE)" CLAUDE.md '[0-9]+ across the workspace' "$ws"
  claim_in "tests workspace (README)" README.md '[0-9]+ passing across the workspace' "$ws"

  # Every member must also build on its own: `cargo test --workspace` unifies features,
  # so a crate can be green in the workspace and fail to compile alone. That is exactly
  # what oasis-secure-element did — four examples needed its own `std` feature, which only
  # oasis-trl-harness turned on — and a visitor building one crate hit it, not CI.
  echo
  echo "== chaque membre compile seul (l'unification des features cache les manques) =="
  # Read the member list from the workspace manifest rather than naming them here,
  # so a new member is covered the day it is added.
  for p in $(awk '/^members = \[/{f=1;next} f&&/^\]/{exit} f&&/^[[:space:]]*#/{next} f&&/"/{gsub(/[",]/,"");print $1}' Cargo.toml); do
    if cargo test -p "$p" --release --no-run >/dev/null 2>&1; then
      printf '  ok    %-34s compile seul\n' "$p"
    else
      printf '  ECHEC %-34s ne compile pas seul\n' "$p"
      fail=$((fail + 1))
    fi
  done
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
