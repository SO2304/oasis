//! Shared core for audit_lint + audit_lint_recall examples.
//!
//! Provides `scan_line(line) -> Vec<Match>` and `Match::is_flagged_at(tol)`.
//! Encapsulates the parser + heuristics so that the AH-round example
//! (audit_lint) and the AI-round example (audit_lint_recall) share
//! identical semantics.

/// One arithmetic match found on a line.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub expression: String,
    pub claimed: f64,
    pub computed: f64,
}

impl Match {
    /// Relative error, percent (0-100).
    pub fn error_pct(&self) -> f64 {
        let abs = (self.claimed - self.computed).abs();
        let larger = self.claimed.abs().max(self.computed.abs()).max(1e-9);
        (abs / larger) * 100.0
    }
    /// True if the match exceeds tolerance (would be reported).
    pub fn is_flagged_at(&self, tol_pct: f64) -> bool {
        self.error_pct() > tol_pct
    }
}

/// Parse a numeric literal: handles "3 432", "0.4", "40k", "0.05%".
pub fn parse_number(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() { return None; }
    let first = s.chars().next()?;
    if !first.is_ascii_digit() && first != '-' && first != '+' { return None; }
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{00A0}' && *c != '\u{202F}' && *c != '\u{2009}')
        .collect();
    let (numeric, suffix_mult) = if let Some(stripped) = cleaned.strip_suffix('%') {
        (stripped, 0.01)
    } else if let Some(stripped) = cleaned.strip_suffix('k').or_else(|| cleaned.strip_suffix('K')) {
        (stripped, 1000.0)
    } else if let Some(stripped) = cleaned.strip_suffix('M') {
        (stripped, 1_000_000.0)
    } else {
        (cleaned.as_str(), 1.0)
    };
    let value: f64 = numeric.parse().ok()?;
    Some(value * suffix_mult)
}

fn looks_like_continuation(chars: &[(usize, char)], res_end: usize, full_line: &str) -> bool {
    let mut k = res_end;
    while k < chars.len() && chars[k].1.is_whitespace() { k += 1; }
    if k >= chars.len() { return false; }
    let c = chars[k].1;
    if c == '-' || c == '–' || c == '—' {
        if k + 1 < chars.len() && chars[k + 1].1.is_ascii_digit() { return true; }
    }
    if matches!(c, '×' | '*' | '/' | '÷' | '+') { return true; }
    let start_byte = chars[k].0;
    let end_byte = chars.get(k + 12).map(|(b, _)| *b).unwrap_or(full_line.len());
    let suffix = &full_line[start_byte..end_byte];
    let unit_first_token: String = suffix
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ')' | '|' | ';' | '.'))
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_alphabetic() || *c == 'µ')
        .collect();
    let units = [
        "ms", "μs", "µs", "ns", "us", "sec", "min", "minutes", "hours", "days",
        "bytes", "byte", "B", "KiB", "kB", "MB", "GiB",
        "ops", "envelopes", "resets", "inserts", "K",
    ];
    if !unit_first_token.is_empty() && units.iter().any(|u| u.eq_ignore_ascii_case(&unit_first_token)) {
        return true;
    }
    false
}

fn looks_like_fragment_before(chars: &[(usize, char)], num_idx: usize) -> bool {
    if num_idx == 0 { return false; }
    let mut j = num_idx.saturating_sub(1);
    loop {
        let c = chars[j].1;
        if c.is_ascii_digit() || c == '.' || c.is_whitespace()
            || c == '\u{00A0}' || c == '\u{202F}' || c == '\u{2009}'
            || matches!(c, '%' | 'k' | 'K' | 'M')
        {
            if j == 0 { return false; }
            j -= 1;
            continue;
        }
        return matches!(c, '×' | '*' | '/' | '÷' | '+' | '-');
    }
}

/// Scan one line for all `N op M = R` patterns. See AH/AI audits for
/// the supported grammar + skip heuristics.
pub fn scan_line(line: &str) -> Vec<Match> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (number_start, _) = chars[i];
        let mut number_end = i;
        let mut has_digit = false;
        while number_end < chars.len() {
            let c = chars[number_end].1;
            if c.is_ascii_digit() { has_digit = true; number_end += 1; }
            else if c == '.' || c == ' ' || c == '\u{00A0}' || c == '\u{202F}' || c == '\u{2009}' {
                number_end += 1;
            } else { break; }
        }
        if !has_digit { i += 1; continue; }
        if looks_like_fragment_before(&chars, i) { i += 1; continue; }
        if number_end < chars.len() {
            let c = chars[number_end].1;
            if matches!(c, '%' | 'k' | 'K' | 'M') { number_end += 1; }
        }
        let number_end_byte = if number_end < chars.len() { chars[number_end].0 } else { line.len() };
        let num_str = &line[number_start..number_end_byte];
        let n1 = match parse_number(num_str) {
            Some(v) if num_str.trim().chars().any(|c| c.is_ascii_digit()) => v,
            _ => { i += 1; continue; }
        };
        let mut j = number_end;
        while j < chars.len() && chars[j].1.is_whitespace() { j += 1; }
        if j >= chars.len() { break; }
        let op = chars[j].1;
        if !matches!(op, '×' | '*' | '/' | '÷' | '+' | '-') { i = number_end.max(i + 1); continue; }
        j += 1;
        while j < chars.len() && chars[j].1.is_whitespace() { j += 1; }
        if j >= chars.len() { break; }
        let second_start = chars[j].0;
        let mut second_end = j;
        let mut has_digit2 = false;
        while second_end < chars.len() {
            let c = chars[second_end].1;
            if c.is_ascii_digit() { has_digit2 = true; second_end += 1; }
            else if c == '.' || c == ' ' || c == '\u{00A0}' || c == '\u{202F}' || c == '\u{2009}' {
                second_end += 1;
            } else { break; }
        }
        if !has_digit2 { i = j + 1; continue; }
        if second_end < chars.len() {
            let c = chars[second_end].1;
            if matches!(c, '%' | 'k' | 'K' | 'M') { second_end += 1; }
        }
        let second_end_byte = if second_end < chars.len() { chars[second_end].0 } else { line.len() };
        let num_str2 = &line[second_start..second_end_byte];
        let n2 = match parse_number(num_str2) {
            Some(v) => v,
            None => { i = second_end; continue; }
        };
        let mut k = second_end;
        while k < chars.len() && chars[k].1.is_whitespace() { k += 1; }
        if k >= chars.len() { break; }
        if !matches!(chars[k].1, '=' | '≈') { i = second_end; continue; }
        k += 1;
        while k < chars.len() && chars[k].1.is_whitespace() { k += 1; }
        if k >= chars.len() { break; }
        let res_start = chars[k].0;
        let mut res_end = k;
        let mut has_digit3 = false;
        while res_end < chars.len() {
            let c = chars[res_end].1;
            if c.is_ascii_digit() { has_digit3 = true; res_end += 1; }
            else if c == '.' || c == ' ' || c == '\u{00A0}' || c == '\u{202F}' || c == '\u{2009}' {
                res_end += 1;
            } else { break; }
        }
        if !has_digit3 { i = k; continue; }
        if res_end < chars.len() {
            let c = chars[res_end].1;
            if matches!(c, '%' | 'k' | 'K' | 'M') { res_end += 1; }
        }
        let res_end_byte = if res_end < chars.len() { chars[res_end].0 } else { line.len() };
        let res_str = &line[res_start..res_end_byte];
        let r = match parse_number(res_str) {
            Some(v) => v,
            None => { i = res_end; continue; }
        };
        if looks_like_continuation(&chars, res_end, line) { i = res_end; continue; }
        let computed = match op {
            '×' | '*' => n1 * n2,
            '/' | '÷' => if n2 != 0.0 { n1 / n2 } else { f64::NAN },
            '+' => n1 + n2,
            '-' => n1 - n2,
            _ => unreachable!(),
        };
        if computed.is_nan() { i = res_end.max(i + 1); continue; }
        let expression = format!("{} {} {} = {}",
            num_str.trim(), op, num_str2.trim(), res_str.trim());
        out.push(Match { expression, claimed: r, computed });
        i = res_end.max(i + 1);
    }
    out
}
