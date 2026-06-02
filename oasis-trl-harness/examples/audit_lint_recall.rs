//! AI — recall benchmark for audit_lint.
//!
//! Builds a ground-truth corpus of expressions labeled as either
//! WRONG (must be flagged) or RIGHT (must NOT be flagged), spanning
//! 8 bug-class categories. Runs scan_line + flagging, classifies
//! each outcome as TP/FP/TN/FN, reports precision + recall + F1
//! per category and overall.
//!
//! This answers the AI-round question: "what fraction of WRONG
//! arithmetic does the linter actually catch?"
//!
//! Categories tested:
//!   1. simple_arith        — basic 2-operand wrong + right
//!   2. with_thin_space     — thousands separators "3 432"
//!   3. with_k_suffix       — "40k", "1.5M"
//!   4. with_percentage     — "0.4%"
//!   5. with_approx         — "≈" instead of "="
//!   6. division            — "100 / 4"
//!   7. multi_operand       — "a × b × c = d" (KNOWN limitation)
//!   8. boundary_tolerance  — within ±5% (must not flag)
//!   9. fragments_with_unit — "= 5 ms", "5-15 ms" (must not flag)
//!   10. obfuscated_in_text — "the answer 2 × 3 = 7 was wrong"
//!
//! Output: precision/recall/F1 table + per-category breakdown +
//! list of false negatives (missed real errors).

use oasis_trl_harness::audit_lint::scan_line;

const TOLERANCE_PCT: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Truth {
    Wrong,
    Right,
}

struct Fixture {
    text: &'static str,
    truth: Truth,
    category: &'static str,
}

fn corpus() -> Vec<Fixture> {
    use Truth::*;
    vec![
        // ── Category 1: simple_arith ────────────────────────────
        Fixture {
            text: "2 × 3 = 7",
            truth: Wrong,
            category: "simple_arith",
        },
        Fixture {
            text: "2 × 3 = 6",
            truth: Right,
            category: "simple_arith",
        },
        Fixture {
            text: "100 + 50 = 200",
            truth: Wrong,
            category: "simple_arith",
        },
        Fixture {
            text: "100 + 50 = 150",
            truth: Right,
            category: "simple_arith",
        },
        Fixture {
            text: "9 × 9 = 99",
            truth: Wrong,
            category: "simple_arith",
        },
        Fixture {
            text: "9 × 9 = 81",
            truth: Right,
            category: "simple_arith",
        },
        Fixture {
            text: "1000 - 250 = 800",
            truth: Wrong,
            category: "simple_arith",
        },
        Fixture {
            text: "1000 - 250 = 750",
            truth: Right,
            category: "simple_arith",
        },
        Fixture {
            text: "5 + 5 = 11",
            truth: Wrong,
            category: "simple_arith",
        },
        Fixture {
            text: "5 + 5 = 10",
            truth: Right,
            category: "simple_arith",
        },
        // ── Category 2: with_thin_space ─────────────────────────
        // (Original "= 7 000" / "= 2 500" mislabeled WRONG — error
        //  was 1.94% / 1.28%, BELOW 5% tolerance so correctly silent.
        //  Fixed to use unambiguously-wrong values per AI-audit
        //  meta-finding: fixture labels themselves needed verification.)
        Fixture {
            text: "3 432 × 2 = 8 500",
            truth: Wrong,
            category: "with_thin_space",
        },
        Fixture {
            text: "3 432 × 2 = 6 864",
            truth: Right,
            category: "with_thin_space",
        },
        Fixture {
            text: "1 234 + 1 234 = 3 200",
            truth: Wrong,
            category: "with_thin_space",
        },
        Fixture {
            text: "1 234 + 1 234 = 2 468",
            truth: Right,
            category: "with_thin_space",
        },
        Fixture {
            text: "86 400 / 2 = 50 000",
            truth: Wrong,
            category: "with_thin_space",
        },
        Fixture {
            text: "86 400 / 2 = 43 200",
            truth: Right,
            category: "with_thin_space",
        },
        // ── Category 3: with_k_suffix ───────────────────────────
        Fixture {
            text: "40k × 2 = 100k",
            truth: Wrong,
            category: "with_k_suffix",
        },
        Fixture {
            text: "40k × 2 = 80k",
            truth: Right,
            category: "with_k_suffix",
        },
        Fixture {
            text: "1M / 2 = 600k",
            truth: Wrong,
            category: "with_k_suffix",
        },
        Fixture {
            text: "1M / 2 = 500k",
            truth: Right,
            category: "with_k_suffix",
        },
        Fixture {
            text: "5k + 3k = 7k",
            truth: Wrong,
            category: "with_k_suffix",
        },
        Fixture {
            text: "5k + 3k = 8k",
            truth: Right,
            category: "with_k_suffix",
        },
        // ── Category 4: with_percentage ─────────────────────────
        // Note: "0.4% × 3600 = 14.4" → 0.004 × 3600 = 14.4. Right.
        Fixture {
            text: "0.4% × 3600 = 14.4",
            truth: Right,
            category: "with_percentage",
        },
        Fixture {
            text: "0.4% × 3600 = 25",
            truth: Wrong,
            category: "with_percentage",
        },
        Fixture {
            text: "5% × 200 = 10",
            truth: Right,
            category: "with_percentage",
        },
        Fixture {
            text: "5% × 200 = 50",
            truth: Wrong,
            category: "with_percentage",
        },
        Fixture {
            text: "10% × 50 = 5",
            truth: Right,
            category: "with_percentage",
        },
        // ── Category 5: with_approx ─────────────────────────────
        Fixture {
            text: "3 × 7 ≈ 21",
            truth: Right,
            category: "with_approx",
        },
        Fixture {
            text: "3 × 7 ≈ 25",
            truth: Wrong,
            category: "with_approx",
        },
        Fixture {
            text: "100 / 3 ≈ 33",
            truth: Right,
            category: "with_approx",
        },
        Fixture {
            text: "100 / 3 ≈ 50",
            truth: Wrong,
            category: "with_approx",
        },
        // ── Category 6: division ────────────────────────────────
        Fixture {
            text: "100 / 4 = 25",
            truth: Right,
            category: "division",
        },
        Fixture {
            text: "100 / 4 = 30",
            truth: Wrong,
            category: "division",
        },
        Fixture {
            text: "1000 / 8 = 125",
            truth: Right,
            category: "division",
        },
        Fixture {
            text: "1000 / 8 = 200",
            truth: Wrong,
            category: "division",
        },
        Fixture {
            text: "60 ÷ 5 = 12",
            truth: Right,
            category: "division",
        },
        Fixture {
            text: "60 ÷ 5 = 15",
            truth: Wrong,
            category: "division",
        },
        // ── Category 7: multi_operand (KNOWN linter limitation) ─
        // The linter only handles binary expressions; 3-operand are
        // expected to be MISSED (no flag, even if wrong). These
        // fixtures document the recall gap.
        Fixture {
            text: "2 × 3 × 4 = 30",
            truth: Wrong,
            category: "multi_operand",
        },
        Fixture {
            text: "2 × 3 × 4 = 24",
            truth: Right,
            category: "multi_operand",
        },
        Fixture {
            text: "60 / 7 × 115 = 500",
            truth: Wrong,
            category: "multi_operand",
        },
        Fixture {
            text: "60 / 7 × 115 = 985",
            truth: Right,
            category: "multi_operand",
        },
        Fixture {
            text: "100 + 50 + 50 = 250",
            truth: Wrong,
            category: "multi_operand",
        },
        Fixture {
            text: "100 + 50 + 50 = 200",
            truth: Right,
            category: "multi_operand",
        },
        // ── Category 8: boundary_tolerance (within ±5%) ─────────
        // |c - v| / max(c, v) × 100 ≤ 5 → must NOT flag.
        // Examples: 95 vs 100 = 5% off → boundary, NOT flagged
        // (5% is exclusive; only > 5% flags).
        Fixture {
            text: "100 × 1 = 95",
            truth: Right,
            category: "boundary_tolerance",
        },
        Fixture {
            text: "100 × 1 = 96",
            truth: Right,
            category: "boundary_tolerance",
        },
        Fixture {
            text: "100 × 1 = 97",
            truth: Right,
            category: "boundary_tolerance",
        },
        Fixture {
            text: "100 × 1 = 105",
            truth: Right,
            category: "boundary_tolerance",
        },
        Fixture {
            text: "100 × 1 = 94",
            truth: Wrong,
            category: "boundary_tolerance",
        },
        Fixture {
            text: "100 × 1 = 106",
            truth: Wrong,
            category: "boundary_tolerance",
        },
        // ── Category 9: fragments_with_unit ─────────────────────
        // These look like arithmetic but are decoration. Linter
        // MUST NOT flag, even if claimed-result is "wrong".
        Fixture {
            text: "Pattern A 10k × 100 = 5-15 ms",
            truth: Right,
            category: "fragments_with_unit",
        },
        Fixture {
            text: "baseline 2 × 5 = 10 ms",
            truth: Right,
            category: "fragments_with_unit",
        },
        Fixture {
            text: "throughput 3 × 4 = 14 envelopes",
            truth: Right,
            category: "fragments_with_unit",
        },
        Fixture {
            text: "drift 2 × 3 = 7 resets",
            truth: Right,
            category: "fragments_with_unit",
        },
        Fixture {
            text: "size 8 × 16 = 130 bytes",
            truth: Right,
            category: "fragments_with_unit",
        },
        // ── Category 10: obfuscated_in_text ─────────────────────
        Fixture {
            text: "the wrong claim 2 × 3 = 7 was made",
            truth: Wrong,
            category: "obfuscated_in_text",
        },
        Fixture {
            text: "given that 5 + 5 = 11, we proceed",
            truth: Wrong,
            category: "obfuscated_in_text",
        },
        Fixture {
            text: "as expected 7 × 8 = 56 holds",
            truth: Right,
            category: "obfuscated_in_text",
        },
        Fixture {
            text: "the formula 100 / 4 = 25 is right",
            truth: Right,
            category: "obfuscated_in_text",
        },
        Fixture {
            text: "see that 9 × 9 = 99 fails",
            truth: Wrong,
            category: "obfuscated_in_text",
        },
    ]
}

#[derive(Default, Debug)]
struct CatStats {
    name: String,
    tp: u32,  // wrong + flagged → caught
    fp: u32,  // right + flagged → false alarm
    tn: u32,  // right + silent → correctly silent
    fn_: u32, // wrong + silent → MISSED (recall failure)
}
impl CatStats {
    fn precision(&self) -> f64 {
        let denom = self.tp + self.fp;
        if denom == 0 {
            1.0
        } else {
            self.tp as f64 / denom as f64
        }
    }
    fn recall(&self) -> f64 {
        let denom = self.tp + self.fn_;
        if denom == 0 {
            1.0
        } else {
            self.tp as f64 / denom as f64
        }
    }
    fn f1(&self) -> f64 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

fn evaluate_one(text: &str) -> bool {
    // Returns true if the linter would FLAG this line at TOLERANCE_PCT.
    let matches = scan_line(text);
    matches.iter().any(|m| m.is_flagged_at(TOLERANCE_PCT))
}

fn main() {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AI — audit_lint recall benchmark                                ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let fixtures = corpus();
    println!(
        "  Corpus: {} fixtures across {} categories",
        fixtures.len(),
        {
            let mut cats: Vec<&str> = fixtures.iter().map(|f| f.category).collect();
            cats.sort();
            cats.dedup();
            cats.len()
        }
    );
    println!("  Tolerance: {}%", TOLERANCE_PCT);
    println!();

    use std::collections::BTreeMap;
    let mut stats: BTreeMap<String, CatStats> = BTreeMap::new();
    let mut false_negatives: Vec<&Fixture> = Vec::new();
    let mut false_positives: Vec<&Fixture> = Vec::new();

    for f in &fixtures {
        let entry = stats.entry(f.category.to_string()).or_insert_with(|| {
            let mut c = CatStats::default();
            c.name = f.category.to_string();
            c
        });
        let flagged = evaluate_one(f.text);
        match (f.truth, flagged) {
            (Truth::Wrong, true) => entry.tp += 1,
            (Truth::Wrong, false) => {
                entry.fn_ += 1;
                false_negatives.push(f);
            }
            (Truth::Right, true) => {
                entry.fp += 1;
                false_positives.push(f);
            }
            (Truth::Right, false) => entry.tn += 1,
        }
    }

    let mut overall = CatStats::default();
    overall.name = "OVERALL".to_string();
    println!(
        "  {:<25}  {:>4}  {:>4}  {:>4}  {:>4}  {:>9}  {:>9}  {:>9}",
        "category", "tp", "fp", "tn", "fn", "precision", "recall", "f1"
    );
    println!("  {}", "-".repeat(82));
    for (_, s) in &stats {
        println!(
            "  {:<25}  {:>4}  {:>4}  {:>4}  {:>4}  {:>8.2}%  {:>8.2}%  {:>8.2}%",
            s.name,
            s.tp,
            s.fp,
            s.tn,
            s.fn_,
            s.precision() * 100.0,
            s.recall() * 100.0,
            s.f1() * 100.0
        );
        overall.tp += s.tp;
        overall.fp += s.fp;
        overall.tn += s.tn;
        overall.fn_ += s.fn_;
    }
    println!("  {}", "-".repeat(82));
    println!(
        "  {:<25}  {:>4}  {:>4}  {:>4}  {:>4}  {:>8.2}%  {:>8.2}%  {:>8.2}%",
        overall.name,
        overall.tp,
        overall.fp,
        overall.tn,
        overall.fn_,
        overall.precision() * 100.0,
        overall.recall() * 100.0,
        overall.f1() * 100.0
    );

    if !false_negatives.is_empty() {
        println!();
        println!(
            "  False negatives (wrong arithmetic NOT caught — {} total):",
            false_negatives.len()
        );
        for f in &false_negatives {
            println!("    [{:>15}] \"{}\"", f.category, f.text);
        }
    }
    if !false_positives.is_empty() {
        println!();
        println!(
            "  False positives (right arithmetic incorrectly flagged — {} total):",
            false_positives.len()
        );
        for f in &false_positives {
            println!("    [{:>15}] \"{}\"", f.category, f.text);
        }
    }

    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" AI verdict");
    println!("──────────────────────────────────────────────────────────────────");
    let p = overall.precision() * 100.0;
    let r = overall.recall() * 100.0;
    let f1 = overall.f1() * 100.0;
    println!(
        "  Overall precision: {:.2}%  ({}TP / {} flagged)",
        p,
        overall.tp,
        overall.tp + overall.fp
    );
    println!(
        "  Overall recall:    {:.2}%  ({}TP / {} wrong)",
        r,
        overall.tp,
        overall.tp + overall.fn_
    );
    println!("  Overall F1:        {:.2}%", f1);
    println!();
    if r >= 75.0 && p >= 95.0 {
        println!("  [PASS] linter recall ≥ 75% AND precision ≥ 95%");
    } else {
        println!("  [PARTIAL] honest accounting:");
        if r < 75.0 {
            println!(
                "    - recall {:.1}% < 75% target (multi-operand limitation)",
                r
            );
        }
        if p < 95.0 {
            println!(
                "    - precision {:.1}% < 95% target (false-positive heuristics need tuning)",
                p
            );
        }
    }
    if r >= 75.0 && p >= 95.0 {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
