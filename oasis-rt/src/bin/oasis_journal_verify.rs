//! `oasis_journal_verify` — recompute a node's decision-journal hash chain on the PC.
//!
//! Part I of `docs/AUTHORITY_HARDENING_SPEC.md`. Reads what the node's `@G` command
//! prints and says whether the chain is intact, broken, has a hole, or ends on an
//! unconfirmed entry. Exit-coded so a harness can assert on it.
//!
//! ```text
//! oasis_journal_verify <dump-file>
//! ```
//!
//! The dump is the node's own output, one token per line, in any order:
//!
//! ```text
//! JRN_BOOT boot_id=<u64 decimal>
//! JRN_HEAD seq=<u32 decimal|none> hash=<64 hex> overwritten=<u32>
//! JRN_E <64 hex>        # one per entry, oldest first
//! ```
//!
//! Exit codes: `0` intact, `1` broken chain, `2` sequence gap, `3` one unconfirmed
//! entry, `4` malformed input or usage error.
//!
//! Cross-checked against an independent Python implementation of the chain written from
//! the spec text alone (`journal_oracle.py`): the five verdicts and all five exit codes
//! agree, which is the same oracle discipline used for ML-DSA in phase 1.1 and for Modbus
//! in phase 1.4.
//!
//! ⚠️ On `broken`, the reported index is **where verification ended, not where the
//! tampering is** — a single head hash cannot localise a modification, because every
//! hash after the altered entry differs. A *deletion* is localised, by the `seq` hole.
//!
//! ⚠️ **What a code 0 means, and what it does not.** It means no entry was modified,
//! removed or inserted *since the head was written*. It does **not** mean the journal is
//! authentic: on an RP2040 an attacker with physical access (BOOTSEL, SWD) can rewrite
//! the entries and the head together and this tool would print `intact`. The external
//! anchor that would close that hole — the node signing `(seq, head)` so the operator
//! holds an earlier copy — is specified but not implemented. Claim "tamper-evident
//! against a remote attacker", never "tamper-proof".

use oasis_rt::journal::{parse_entry, verify, JournalHead, VerifyResult, ENTRY_LEN};
use std::process::ExitCode;

fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()).collect()
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("oasis_journal_verify: {msg}");
    ExitCode::from(4)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return fail("usage: oasis_journal_verify <dump-file>");
    }
    let text = match std::fs::read_to_string(&args[1]) {
        Ok(t) => t,
        Err(e) => return fail(&format!("cannot read {}: {e}", args[1])),
    };

    let mut boot_id: Option<u64> = None;
    let mut head: Option<JournalHead> = None;
    let mut entries: Vec<[u8; ENTRY_LEN]> = Vec::new();

    for (lineno, line) in text.lines().enumerate() {
        let line = line.trim();
        let mut it = line.split_whitespace();
        match it.next() {
            Some("JRN_BOOT") => {
                let v = it.next().and_then(|t| t.strip_prefix("boot_id=")).and_then(|v| v.parse::<u64>().ok());
                match v {
                    Some(v) => boot_id = Some(v),
                    None => return fail(&format!("line {}: malformed JRN_BOOT", lineno + 1)),
                }
            }
            Some("JRN_HEAD") => {
                let mut seq = None;
                let mut hash = None;
                let mut overwritten = 0u32;
                for tok in it {
                    if let Some(v) = tok.strip_prefix("seq=") {
                        seq = if v == "none" { Some(None) } else { v.parse::<u32>().ok().map(Some) };
                    } else if let Some(v) = tok.strip_prefix("hash=") {
                        hash = hex_to_bytes(v).filter(|b| b.len() == 32);
                    } else if let Some(v) = tok.strip_prefix("overwritten=") {
                        overwritten = v.parse().unwrap_or(0);
                    }
                }
                match (seq, hash) {
                    (Some(seq), Some(h)) => {
                        let mut hh = [0u8; 32];
                        hh.copy_from_slice(&h);
                        head = Some(JournalHead { boot_id: 0, seq, hash: hh, overwritten });
                    }
                    _ => return fail(&format!("line {}: malformed JRN_HEAD", lineno + 1)),
                }
            }
            Some("JRN_E") => {
                let b = match it.next().and_then(hex_to_bytes).filter(|b| b.len() == ENTRY_LEN) {
                    Some(b) => b,
                    None => return fail(&format!("line {}: entry is not {ENTRY_LEN} hex bytes", lineno + 1)),
                };
                let mut e = [0u8; ENTRY_LEN];
                e.copy_from_slice(&b);
                entries.push(e);
            }
            _ => {}
        }
    }

    let boot_id = match boot_id {
        Some(b) => b,
        None => return fail("no JRN_BOOT line"),
    };
    let mut head = match head {
        Some(h) => h,
        None => return fail("no JRN_HEAD line"),
    };
    head.boot_id = boot_id;

    // Keep only the entries of THIS boot. After a reboot the ring still holds the previous
    // boot entries while the head is a fresh chain, so a raw dump mixes two boots and
    // nothing would verify. Each entry carries its own boot_id, which is what makes the
    // separation possible — and it is how a power cut is checked: take the head captured
    // BEFORE the cut and the entries read AFTER it.
    let total = entries.len();
    entries.retain(|e| parse_entry(e).map(|x| x.boot_id) == Some(boot_id));
    if entries.len() != total {
        println!("filtered {} of {total} entries to boot_id={boot_id}", entries.len());
    }

    println!("boot_id={boot_id} entries={} head_seq={:?} overwritten={}", entries.len(), head.seq, head.overwritten);
    for (i, raw) in entries.iter().enumerate() {
        match parse_entry(raw) {
            Some(e) => println!("  [{i}] seq={} origin={:02x}{:02x} cmd_seq={} class={:?} decision={:?} flags={:#04x}", e.seq, e.origin_fp[0], e.origin_fp[1], e.cmd_seq, e.class, e.decision, e.flags),
            None => println!("  [{i}] UNPARSEABLE"),
        }
    }

    match verify(boot_id, &entries, &head) {
        VerifyResult::Intact { entries } => {
            println!("VERDICT intact entries={entries}");
            ExitCode::SUCCESS
        }
        VerifyResult::Broken { at } => {
            println!("VERDICT broken at={at}");
            ExitCode::from(1)
        }
        VerifyResult::SeqGap { expected, found } => {
            println!("VERDICT seq_gap expected={expected} found={found}");
            ExitCode::from(2)
        }
        VerifyResult::Unconfirmed { entries } => {
            println!("VERDICT unconfirmed confirmed_entries={entries}");
            ExitCode::from(3)
        }
    }
}
