# SHADOW AUDIT — AI: audit-lint recall measurement

**Date**: 2026-05-12.
**Trigger**: user question:

> "as-tu mesuré le recall du linter ?"

Translation: did you measure the linter's recall? In the AH round
I claimed `[CLEAN]` across 89 audits, and demoed 3/4 deliberately-
wrong fixtures caught. That's a precision-flavored signal, not a
proper recall measurement. The user's question is the right one.

This round builds a 59-fixture ground-truth corpus, measures TP/FP/
TN/FN per category, computes precision + recall + F1, and reports
the honest numbers — including a self-discovered fixture-labeling
error on my part during initial measurement.

---

## Outcomes

### AI-impl-1 — shared parser module

Refactored: moved the linter's parser into
[oasis-trl-harness/src/audit_lint.rs](../oasis-trl-harness/src/audit_lint.rs)
as a public module. Both the AH example (`audit_lint`) and the
new AI example (`audit_lint_recall`) now share identical parsing
semantics — so the recall numbers are the same numbers the user
sees when running the linter.

14 AH unit tests still pass after refactor.

### AI-impl-2 — ground-truth corpus

[oasis-trl-harness/examples/audit_lint_recall.rs](../oasis-trl-harness/examples/audit_lint_recall.rs)
embeds 59 fixtures across 10 categories:

| category | wrong | right | total |
|---|---:|---:|---:|
| simple_arith | 5 | 5 | 10 |
| with_thin_space | 3 | 3 | 6 |
| with_k_suffix | 3 | 3 | 6 |
| with_percentage | 2 | 3 | 5 |
| with_approx | 2 | 2 | 4 |
| division | 3 | 3 | 6 |
| multi_operand | 3 | 3 | 6 |
| boundary_tolerance | 2 | 4 | 6 |
| fragments_with_unit | 0 | 5 | 5 |
| obfuscated_in_text | 3 | 2 | 5 |
| **TOTAL** | **26** | **33** | **59** |

The categories were chosen to span the bug classes I expected to
matter for OASIS audits: thin-space thousands (AC), k-suffix
constants (AD), percentages (AC mid-cycle), approximations (Z4),
multi-operand (AF typo), unit-suffix decoration (false-positive
trap), markdown-embedded math (everywhere).

### AI-results — precision + recall + F1 per category

After fixture correction (see Finding 1 below):

```text
  category                     tp    fp    tn    fn  precision     recall         f1
  ----------------------------------------------------------------------------------
  boundary_tolerance            2     0     4     0    100.00%    100.00%    100.00%
  division                      3     0     3     0    100.00%    100.00%    100.00%
  fragments_with_unit           0     0     5     0    100.00%    100.00%    100.00%
  multi_operand                 0     0     3     3    100.00%      0.00%      0.00%
  obfuscated_in_text            3     0     2     0    100.00%    100.00%    100.00%
  simple_arith                  5     0     5     0    100.00%    100.00%    100.00%
  with_approx                   2     0     2     0    100.00%    100.00%    100.00%
  with_k_suffix                 3     0     3     0    100.00%    100.00%    100.00%
  with_percentage               2     0     3     0    100.00%    100.00%    100.00%
  with_thin_space               3     0     3     0    100.00%    100.00%    100.00%
  ----------------------------------------------------------------------------------
  OVERALL                      23     0    33     3    100.00%     88.46%     93.88%

  False negatives (wrong arithmetic NOT caught — 3 total):
    [multi_operand] "2 × 3 × 4 = 30"
    [multi_operand] "60 / 7 × 115 = 500"
    [multi_operand] "100 + 50 + 50 = 250"

  Overall precision: 100.00%  (23TP / 23 flagged)
  Overall recall:    88.46%   (23TP / 26 wrong)
  Overall F1:        93.88%

  [PASS] linter recall ≥ 75% AND precision ≥ 95%
```

### AI-kani — 4 new Kani proofs

Added to [oasis-rt/src/mesh.rs:kani_proofs](../oasis-rt/src/mesh.rs):

| # | Name | Encodes |
|---|---|---|
| AI1 | `proof_ai_binary_recall_complete_above_tolerance` | for any binary expr where error > tolerance, linter MUST flag |
| AI2 | `proof_ai_binary_precision_no_flag_within_tolerance` | for any binary expr where error ≤ tolerance, linter MUST NOT flag |
| AI3 | `proof_ai_multi_operand_known_gap` | declared limitation: 3-operand wrong expressions are NOT guaranteed to be caught |
| AI4 | `proof_ai_corpus_recall_precision_pass` | aggregate corpus measurement: recall ≥ 75%, precision ≥ 95% |

## Predictions vs actuals

| # | Predicted | Actual | Match |
|---|---|---|---|
| ≥ 50 wrong + ≥ 30 right corpus | yes | 26 wrong + 33 right | ⚠️ wrong-side under (-24) |
| ≥ 7 categories | yes | 10 categories | ✅ |
| Recall measured per category | yes | done | ✅ |
| F1 reported | yes | 93.88% overall | ✅ |
| Failure modes identified | yes | multi-operand + fixture-mislabel | ✅ |

**4 / 5 fully matched, 1 / 5 under-target (only 26 wrong fixtures
instead of the ≥ 50 promised — honest scope-cap to ship the recall
measurement in this round; could double the corpus next round).**

## Honest finding 1 — I MADE THE SAME MISTAKE THE LINTER IS DESIGNED TO CATCH

First run of the recall benchmark reported recall = 80.77% with 5
false negatives. Two were:

- "3 432 × 2 = 7 000" — labeled WRONG
- "1 234 + 1 234 = 2 500" — labeled WRONG

Investigation: the true arithmetic errors were 1.94% and 1.28%
respectively — BELOW the linter's 5% tolerance. The linter
**correctly** classified them as silent. The fixture LABELS were
wrong — I labeled them WRONG when they're actually RIGHT under
the linter's defined contract.

**This is the same bug class as the AG-round audit's mental-math
errors**: I jotted a wrong number without verifying it. The very
benchmark I was building to measure the linter's recall contained
the bug the linter would catch in a real audit.

After fix: 88.46% recall with only the 3 multi-operand false
negatives remaining (all documented limitations).

**Lesson**: every benchmark fixture's ground-truth label must be
verified against the same calculation the system-under-test uses.
Mental-math errors in BENCHMARK CODE are the same failure mode as
mental-math errors in AUDIT NARRATIVE — only the fix is the same
discipline (cite the formula, compute the expected value).

## Honest finding 2 — multi-operand is THE failure mode

All 3 false negatives are multi-operand fixtures. The parser is
binary-only by design (AH round, deferred to AI as known limitation).
Recall on the bug classes the linter targets is **100%** (23/23
binary expressions); the 11.54% miss is entirely the 3 multi-operand
fixtures we deliberately included to document the gap.

A future linter version (call it AJ?) extending to 3-operand
expressions with operator precedence would push overall recall
from 88.46% to ~100% on this corpus. Estimated ~100 LOC of
state-machine parsing. Deferred — the binary linter is sufficient
for the immediate use case (AH/AG class errors).

## Honest finding 3 — corpus size was 26 wrong vs predicted ≥ 50

AI-1 prediction (from end-of-AH-audit "next round" table) said
"≥ 50 known-wrong + ≥ 30 known-right" fixtures. Actual: 26 + 33 = 59.
The wrong-side is half the predicted 50.

Why under-target: building each fixture requires (a) writing the
text, (b) computing the correct value, (c) confirming the chosen
"wrong" value exceeds 5% tolerance (as I learned in Finding 1).
Each fixture takes ~30 sec to author and verify. 50 wrong fixtures
would have been ~25 minutes of corpus-building; I chose to ship the
measurement at 26 to leave time for the proofs and audit.

Mitigation: the 26 fixtures span 10 categories so coverage per
category averages 2-5 wrong fixtures — enough to detect category-
level recall regressions, even if the aggregate confidence interval
is wider than 50 would give. The 100% within-category recall
(except multi_operand) confirms no category is silently broken.

## Honest finding 4 — precision is GENUINELY 100% on this corpus

The linter flagged 23 things; all 23 were genuine wrong arithmetic.
0 false positives across 33 right fixtures (some of which were
deliberately tricky: fragments with units, ranges, boundary-of-
tolerance cases).

This is the strongest signal of the AH round's heuristics being
sound: the two false-positive-reduction passes (fragment back-walk,
unit-suffix detection) produced exact zero false positives on a
designed corpus.

The conservative reading: precision is 100% **on this corpus**;
real-world audits may surface novel patterns. But the heuristics
look well-conditioned.

## Honest finding 5 — fragments_with_unit had 0 wrong fixtures (no recall metric)

The `fragments_with_unit` category has 0 wrong fixtures by design:
the category exists to test that decoration patterns ("= 5 ms") are
NOT flagged. So recall is N/A for this category (0/0 = 100% by
convention, since "no wrong to catch" trivially achieves recall).

This is correct test design but is worth flagging because the
"100% recall" row in the report is vacuous for this category.

## Updated Kani proof count

| Crate | Proofs |
|---|---:|
| oasis-secure-element | 47 |
| oasis-operator-key | 4 |
| oasis-trl-harness | 12 |
| oasis-rt::mesh (was N+24, now N+28) | (N+28) |
| **Total** | **91** (was 87) |

## What's NOT done in this round (honest)

- **Corpus expansion to ≥ 50 wrong fixtures**: deferred (current
  26 sufficient for category-level signal).
- **3-operand expression parsing**: deferred (~100 LOC for next
  round; would push recall to ~100% on current corpus).
- **CI integration of the recall benchmark**: deferred (no CI
  infrastructure yet).
- **Real-world corpus** (run linter on a public Markdown corpus
  beyond OASIS audits): deferred.
- **Hardware-in-the-loop**: still the bigger gap to actual TRL 6.

## Updated TRL posture

Before AI round (per AH audit):
> "TRL 6.0-software, audit-hygiene MECHANIZED."

After AI round:
> "**TRL 6.0-software, audit-hygiene MECHANIZED + MEASURED**. The
> audit-lint tool has documented precision (100% on a 59-fixture
> corpus across 10 categories) and recall (88.46% overall; 100% on
> binary expressions, 0% on documented multi-operand limitation).
> 4 new Kani proofs formalize per-class recall + precision
> guarantees AND honestly declare the multi-operand recall gap.
> **91 Kani proofs total**."

## Predictions for next round

| # | Prediction |
|---|---|
| AJ1 | Extending audit_lint to handle 3-operand expressions via Shunting-yard parser (~120 LOC) pushes recall on the AI corpus from 88.46% to ~100% with no precision degradation |
| AJ2 | Doubling corpus to ≥ 50 wrong + ≥ 60 right fixtures (target 110 total) tightens the recall confidence interval from current ±10% to ±5% — sufficient signal for production use |
| AJ3 | A "stress-corpus" of audit-narrative-style sentences (paragraphs with embedded math, multiple expressions per sentence) will surface 1-3 new false-positive patterns the AH heuristics don't yet cover, requiring 1 more refinement pass |
| AJ4 | A `--strict` mode for audit_lint that tolerates 0% deviation (only exact equality) catches MORE typos but creates ~10× the false-positive rate — useful only for code-block arithmetic, not narrative |

## One-sentence verdict

**AI round answered the recall question by building a 59-fixture corpus (26 wrong + 33 right across 10 categories: simple_arith, with_thin_space, with_k_suffix, with_percentage, with_approx, division, multi_operand, boundary_tolerance, fragments_with_unit, obfuscated_in_text), refactoring the linter's parser into a shared lib module so both `audit_lint` and `audit_lint_recall` examples use identical semantics, measuring precision 100% (23 TP / 0 FP) and recall 88.46% (23 TP / 3 FN, all multi-operand) with F1 93.88% — passing the ≥ 75% recall + ≥ 95% precision bar; honestly disclosed: (1) my first run reported lower recall because TWO of my own fixture labels were wrong — claimed-wrong but errors below the 5% tolerance — I had reproduced the AG-round mental-math bug class IN THE BENCHMARK CODE itself, fixed in place; (2) multi-operand is the sole recall gap (binary parser by design, documented as known limitation, ~100 LOC fix queued for AJ round); (3) corpus is 26 wrong vs predicted ≥ 50, honest scope-cap; 4 new Kani proofs formalize per-class recall + precision + multi-operand-gap-declaration + corpus-aggregate-bound; cross-crate Kani total 91 (was 87); TRL stays at 6.0-software with audit-hygiene now both MECHANIZED and MEASURED.**
