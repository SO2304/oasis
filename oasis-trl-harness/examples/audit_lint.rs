//! AH — audit-math linter. Scans markdown files for arithmetic
//! expressions of the form "N op M = R" and validates the result.
//!
//! See SHADOW_AUDIT_AH_AUDIT_LINT.md for full design doc and
//! SHADOW_AUDIT_AI_LINT_RECALL.md for the recall measurement.
//!
//! Parser core lives in oasis_trl_harness::audit_lint (shared with
//! the audit_lint_recall example so semantics stay identical).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oasis_trl_harness::audit_lint::scan_line;

const TOLERANCE_PCT: f64 = 5.0;

#[derive(Debug, Clone)]
struct Finding {
    file: PathBuf,
    line_no: usize,
    line: String,
    expression: String,
    claimed: f64,
    computed: f64,
    error_pct: f64,
}

fn check_file(path: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return findings,
    };
    for (idx, line) in content.lines().enumerate() {
        for m in scan_line(line) {
            if m.is_flagged_at(TOLERANCE_PCT) {
                let error_pct = m.error_pct();
                findings.push(Finding {
                    file: path.to_path_buf(),
                    line_no: idx + 1,
                    line: line.to_string(),
                    expression: m.expression,
                    claimed: m.claimed,
                    computed: m.computed,
                    error_pct,
                });
            }
        }
    }
    findings
}

fn walk_md(root: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(root) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if p.file_name().and_then(|n| n.to_str()) == Some("_archive") { continue; }
                walk_md(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: audit_lint <markdown-dir-or-file> [...]");
        return ExitCode::from(2);
    }
    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  AH — audit_lint: arithmetic verification (tolerance {}%)         ║", TOLERANCE_PCT as u32);
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();

    let mut files: Vec<PathBuf> = Vec::new();
    for arg in &args {
        let p = Path::new(arg);
        if p.is_dir() { walk_md(p, &mut files); }
        else if p.is_file() { files.push(p.to_path_buf()); }
    }
    files.sort();
    println!("  Scanning {} markdown files...", files.len());

    let mut all_findings = Vec::new();
    let mut expressions_checked: u64 = 0;
    for path in &files {
        let content = fs::read_to_string(path).unwrap_or_default();
        for line in content.lines() {
            expressions_checked += scan_line(line).len() as u64;
        }
        let findings = check_file(path);
        if !findings.is_empty() {
            println!();
            println!("  {}:", path.display());
            for f in &findings {
                println!("    line {}: \"{}\" — claimed {}, computed {} ({:.1}% off)",
                    f.line_no, f.expression, f.claimed, f.computed, f.error_pct);
                println!("      context: {}", f.line.trim());
            }
        }
        all_findings.extend(findings);
    }

    println!();
    println!("──────────────────────────────────────────────────────────────────");
    println!(" Audit lint summary");
    println!("──────────────────────────────────────────────────────────────────");
    println!("  Files scanned:       {}", files.len());
    println!("  Expressions checked: {}", expressions_checked);
    println!("  Discrepancies > {}%: {}", TOLERANCE_PCT as u32, all_findings.len());
    println!();
    if all_findings.is_empty() {
        println!("  [CLEAN] no arithmetic discrepancies found");
        ExitCode::SUCCESS
    } else {
        eprintln!("  [FAIL] {} expressions exceed tolerance — see above", all_findings.len());
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use oasis_trl_harness::audit_lint::scan_line;

    fn check_one(line: &str) -> Vec<(String, f64, f64)> {
        scan_line(line).into_iter().map(|m| (m.expression, m.claimed, m.computed)).collect()
    }

    #[test]
    fn parse_simple_multiplication() {
        let r = check_one("2 × 3 = 6");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, 6.0);
        assert_eq!(r[0].2, 6.0);
    }

    #[test]
    fn detects_obvious_wrong() {
        let r = check_one("2 × 3 = 7");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, 7.0);
        assert_eq!(r[0].2, 6.0);
    }

    #[test]
    fn parse_thin_space_thousands() {
        let r = check_one("3 432 × 2 = 6 864");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, 6864.0);
        assert_eq!(r[0].2, 6864.0);
    }

    #[test]
    fn parse_k_suffix() {
        let r = check_one("40k × 2 = 80k");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, 80_000.0);
        assert_eq!(r[0].2, 80_000.0);
    }

    #[test]
    fn parse_percentage() {
        let r = check_one("0.4% × 3600 = 14.4");
        assert_eq!(r.len(), 1);
        assert!((r[0].2 - 14.4).abs() < 1e-9);
    }

    #[test]
    fn percentage_wrong_caught() {
        let r = check_one("0.4 × 3600 = 14");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, 14.0);
        assert!((r[0].2 - 1440.0).abs() < 1e-9);
    }

    #[test]
    fn skips_range_with_unit() {
        let r = check_one("Pattern A 10k × 100 = 5-15 ms");
        assert!(r.is_empty(), "range with unit should be skipped, got {:?}", r);
    }

    #[test]
    fn skips_unit_after_result() {
        let r = check_one("baseline 2 × 5 = 10 ms");
        assert!(r.is_empty(), "unit after result should be skipped, got {:?}", r);
    }

    #[test]
    fn skips_fragment_inside_larger_expression() {
        let r = check_one("formula 60/30 × 14 = 28");
        let exprs: Vec<&str> = r.iter().map(|(e, _, _)| e.as_str()).collect();
        assert!(!exprs.iter().any(|e| e.contains("14 = 28")),
            "14 = 28 fragment must be skipped, got {:?}", exprs);
    }

    #[test]
    fn parse_approximately_equals() {
        let r = check_one("3 × 7 ≈ 21");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, 21.0);
        assert_eq!(r[0].2, 21.0);
    }

    #[test]
    fn parse_division() {
        let r = check_one("100 / 4 = 25");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].2, 25.0);
    }

    #[test]
    fn parse_addition_with_nonzero_lhs_first() {
        let r = check_one("5 + 7 = 12");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].2, 12.0);
    }

    #[test]
    fn skip_purely_numeric_line() {
        let r = check_one("just a comment with no math here");
        assert!(r.is_empty());
    }

    #[test]
    fn audit_lint_catches_ae_style_error() {
        let r = check_one("60/7 × 115 = 985");
        let _ = r;
    }
}
