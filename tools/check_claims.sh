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

# `claim` judges ONE occurrence, the first. A document that states the same count twice
# therefore passed with the second one stale — which is exactly what happened: CLAUDE.md
# said "27 [[bin]]" on line 31 and "23 [[bin]]" on line 129, and the checker read only the
# first (2026-10-09). This judges EVERY occurrence of a pattern against the tree.
claim_all() { # claim_all <label> <file> <regex> <actual>
  local label="$1" file="$2" re="$3" want="$4" seen bad=0 n=0
  while IFS= read -r m; do
    n=$((n + 1))
    seen=$(printf '%s' "$m" | grep -oE '[0-9]+' | head -1)
    [ "$seen" = "$want" ] || { printf '  DRIFT %-34s %s dit %s, arbre dit %s\n' "$label" "$file" "$seen" "$want"; bad=$((bad + 1)); }
  done < <(grep -oE "$re" "$file" 2>/dev/null)
  if [ "$n" = 0 ]; then
    printf '  ABSENT %-33s aucune occurrence de /%s/ dans %s\n' "$label" "$re" "$file"
    fail=$((fail + 1))
  elif [ "$bad" = 0 ]; then
    printf '  ok    %-34s %s, %s occurrence(s)\n' "$label" "$want" "$n"
  else
    fail=$((fail + 1))
  fi
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
claim_all "[[bin]] dans CLAUDE.md" CLAUDE.md '[0-9]+ .\[\[bin\]\].|[0-9]+ production binaries' \
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
echo "== campagne pilote : le document, le journal et l'empreinte doivent concorder =="
# Four drifts an external audit found on 2026-10-09, each because nothing compared two
# places that had to agree. The campaign log is the authority in all four.
CAMP=evidence/pilot/2026-10-09
if [ -d "$CAMP" ] && ls "$CAMP"/run*.log >/dev/null 2>&1; then
  # (a) the number of cases the log reports, against what the gap register claims.
  log_cases=$(awk '/cases: [0-9]+ passed/{for(i=1;i<=NF;i++) if($i=="cases:"){print $(i+1); exit}}' "$CAMP/run1.log")
  doc_cases=$(grep -oE '[0-9]+ cas sur [0-9]+' partners/POSITIONING_GAPS.md | head -1 | grep -oE '^[0-9]+')
  claim "cas de campagne (section G)" "${doc_cases:-absent}" "${log_cases:-?}"

  # (b) the medians the README prints, against the medians in the logs. The README said
  # 1 017/1 159/1 248 us while the logs beside it said something else entirely.
  log_med=$(grep -hoE 'median [0-9]+ us' "$CAMP"/run*.log | grep -oE '[0-9]+' | sort -n | tr '\n' ' ')
  missing=0
  for m in $log_med; do
    grep -qE "(^|[^0-9])$(echo "$m" | sed 's/\(.\)\(...\)$/\1 \2/')([^0-9]|$)|(^|[^0-9])$m([^0-9]|$)" "$CAMP/README.md" || missing=$((missing + 1))
  done
  if [ "$missing" = 0 ]; then
    printf '  ok    %-34s %s\n' "latences du README vs journaux" "$(echo "$log_med" | wc -w | tr -d ' ') valeurs retrouvees"
  else
    printf '  DRIFT %-34s %s mediane(s) du journal absente(s) du README (%s)\n' "latences du README vs journaux" "$missing" "$log_med"
    fail=$((fail + 1))
  fi

  # (c) the tree stamp must name a commit that exists. A log stamped with a commit that
  # does not contain the code it exercised is not evidence -- which is what happened when
  # the campaign ran before the code was committed.
  bad_tree=0
  for t in $(grep -hoE 'tree=[0-9a-f]+' "$CAMP"/run*.log | sed 's/tree=//' | sort -u); do
    git cat-file -e "$t^{commit}" 2>/dev/null || { echo "  INCONNU tree=$t n'est pas un commit de ce depot"; bad_tree=$((bad_tree + 1)); }
  done
  if [ "$bad_tree" = 0 ]; then
    printf '  ok    %-34s %s\n' "empreintes des journaux" "$(grep -hoE 'tree=[0-9a-f]+' "$CAMP"/run*.log | sort -u | tr '\n' ' ')"
  else
    fail=$((fail + 1))
  fi
else
  printf '  ABSENT %-33s %s\n' "campagne pilote" "$CAMP/run*.log"
  fail=$((fail + 1))
fi

echo
echo "== identifiants uniques dans les tables qui s'en servent de cle =="
# `partners/DIFFERENTIATORS.md` used M6 twice, so two different differentiators answered to
# one name and a reader citing "M6" could not be understood.
for f in partners/DIFFERENTIATORS.md; do
  [ -f "$f" ] || continue
  dup=$(grep -oE '^\| ?M[0-9]+' "$f" | tr -d '| ' | sort | uniq -d | tr '\n' ' ')
  if [ -z "$dup" ]; then
    printf '  ok    %-34s %s identifiants, aucun double\n' "$(basename "$f")" "$(grep -cE '^\| ?M[0-9]+' "$f" | tr -d ' ')"
  else
    printf '  DRIFT %-34s identifiant(s) en double : %s\n' "$(basename "$f")" "$dup"
    fail=$((fail + 1))
  fi
done

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
echo "== lignes de tableau dont la largeur ne suit pas leur en-tete =="
# An unescaped `|` inside a cell adds a column, and every renderer drops what follows
# it: the text sits in the file and is invisible to the reader. Found 2026-10-09 in
# four documents, one of them hiding a FALSE claim behind the break — a cell asserting
# "182 des 189 en CI" that CLAUDE.md contradicts, cut in half by a pipe in a `grep`
# command. A document nobody can read whole cannot be audited, so this is a drift.
rowbad=$(find . -name '*.md' -not -path './.kilo/*' -not -path './target/*' \
  -not -path './node_modules/*' -print0 2>/dev/null | xargs -0 awk '
    function npipes(s) { gsub(/\\\|/, "\001", s); return gsub(/\|/, "|", s) }
    FNR == 1       { want = 0; fence = 0 }
    /^[ \t]*```/   { fence = !fence; next }
    fence          { next }
    {
      if (substr($0, 1, 1) != "|") { want = 0; next }
      if ($0 ~ /^[ \t|:-]+$/) next          # the ---|--- separator
      n = npipes($0)
      if (want == 0) { want = n; next }     # the header sets the width
      if (n != want) printf "  LARGEUR %s:%d  %d barres, en-tete %d\n", FILENAME, FNR, n, want
    }')
if [ -z "$rowbad" ]; then
  printf '  ok    %-34s %s fichier(s) markdown\n' "largeur des tableaux" \
    "$(find . -name '*.md' -not -path './.kilo/*' -not -path './target/*' -not -path './node_modules/*' 2>/dev/null | wc -l)"
else
  printf '%s\n' "$rowbad"
  printf '  DRIFT %-34s %s ligne(s) tronquee(s) au rendu\n' "largeur des tableaux" \
    "$(printf '%s\n' "$rowbad" | grep -c LARGEUR)"
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
  echo "== le build MCU que CLAUDE.md imprime doit compiler (--full) =="
  # The matrix claimed 0 errors for a command that failed with 4, because nothing ran it.
  # The command is taken FROM the document, so a wrong command in the document fails here
  # rather than being discovered by someone following it.
  mcu_cmd=$(awk '/^MCU build:/{f=1;next} f&&/^```$/{if(seen){exit}else{seen=1;next}} f&&seen{print;exit}' CLAUDE.md)
  if [ -z "$mcu_cmd" ]; then
    printf '  ABSENT %-33s %s\n' "commande MCU" "aucun bloc 'MCU build:' dans CLAUDE.md"
    fail=$((fail + 1))
  else
    echo "  commande: $mcu_cmd"
    if eval "$mcu_cmd" >/dev/null 2>&1; then
      printf '  ok    %-34s compile\n' "build MCU de CLAUDE.md"
    else
      n=$(eval "$mcu_cmd" 2>&1 | grep -cE '^error')
      printf '  ECHEC %-34s %s erreur(s)\n' "build MCU de CLAUDE.md" "$n"
      fail=$((fail + 1))
    fi
  fi

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

  # Completeness. `sha256sum -c` only judges the lines that are there, so a manifest
  # that lists some of its files passes while the rest are unprotected — and a file
  # can then vanish from the evidence with every check still green. Found 2026-10-09:
  # regenerating a manifest with `ls | xargs sha256sum` handed the `neg/` directory to
  # sha256sum, which dropped it on stderr, and the negative control's log — the one
  # file rule 4 most requires — went unlisted. Each file under evidence/ must appear
  # in the nearest manifest above it.
  miss=0; seen=0
  while IFS= read -r m; do
    d=$(dirname "$m")
    names=$(awk '{ n = $2; sub(/^\*/, "", n); print n }' "$m")
    while IFS= read -r f; do
      # A nested directory with its own SHA256SUMS owns its files; skip those.
      sub=$(dirname "$f"); owned=0
      while [ "$sub" != "$d" ]; do
        if [ -f "$sub/SHA256SUMS" ]; then owned=1; break; fi
        sub=$(dirname "$sub")
      done
      [ "$owned" = 1 ] && continue
      seen=$((seen + 1))
      rel=${f#"$d/"}
      printf '%s\n' "$names" | grep -qxF "$rel" || {
        printf '  ABSENT %s present mais absent de %s\n' "$f" "$m"
        miss=$((miss + 1))
      }
    done < <(find "$d" -type f ! -name SHA256SUMS 2>/dev/null)
  done < <(find evidence -name SHA256SUMS 2>/dev/null)
  echo "  ok    $((seen - miss))/$seen fichier(s) de preuve couverts par un manifeste"
  if [ "$miss" != 0 ]; then
    printf '  DRIFT %-34s %s fichier(s) non liste(s)\n' "completude des manifestes" "$miss"
    fail=$((fail + 1))
  fi
fi

echo
if [ "$fail" = 0 ]; then
  echo "AUCUNE DERIVE sur les claims verifiables."
else
  echo "$fail DERIVE(S). Corriger le document, pas le compte."
fi
exit "$fail"
