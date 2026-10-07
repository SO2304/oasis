//! Kani harnesses for the decision journal.
//!
//! **What is proved here and what is not.** These harnesses cover the *structural*
//! properties: the parser is total, the decision byte space is injective, `seq` advances,
//! and nothing in an entry is lost on the way to bytes and back. They deliberately do
//! **not** run SHA-256 under CBMC — symbolic hashing exhausted the memory of this machine
//! in phase 1.2, and a harness that cannot terminate proves nothing.
//!
//! The hash-chain properties are covered by tests instead, exhaustively where it matters:
//! `jrn_every_byte_of_an_entry_is_chained` flips **every bit of every byte** of an entry
//! and requires each one to break verification. That is a test, not a proof, and the
//! distinction is stated rather than blurred.

use super::*;

fn any_decision() -> LoggedDecision {
    let k: u8 = kani::any();
    kani::assume(k < 3);
    match k {
        0 => LoggedDecision::Act,
        1 => LoggedDecision::Stop,
        _ => {
            let r: u8 = kani::any();
            kani::assume((r as usize) < REASON_COUNT);
            LoggedDecision::Reject(reason_from_index(r).unwrap())
        }
    }
}

/// PROVE: nothing in an entry is lost or altered on the way to bytes and back. Every
/// field is symbolic, so this covers all entries, and it is what guarantees the chain
/// hashes the whole entry rather than part of it.
#[kani::proof]
fn proof_entry_roundtrip_is_lossless() {
    let e = Entry {
        seq: kani::any(),
        boot_id: kani::any(),
        origin_fp: kani::any(),
        cmd_seq: kani::any(),
        class: if kani::any() { OrderClass::Act } else { OrderClass::Stop },
        decision: any_decision(),
        flags: kani::any(),
    };
    assert!(parse_entry(&encode_entry(&e)) == Some(e));
}

/// PROVE: `seq` advances by exactly one per append and `next_seq` never panics.
#[kani::proof]
fn proof_journal_seq_monotone() {
    let boot: u64 = kani::any();
    let mut j = Journal::new(boot);
    assert!(j.head.seq.is_none());
    assert!(j.next_seq() == 0);
    let _ = j.append(kani::any(), kani::any(), OrderClass::Act, any_decision(), kani::any());
    assert!(j.head.seq == Some(0));
    assert!(j.next_seq() == 1);
    let _ = j.append(kani::any(), kani::any(), OrderClass::Stop, any_decision(), kani::any());
    assert!(j.head.seq == Some(1));
    assert!(j.next_seq() == 2);
}

/// PROVE: the entry parser is total — any slice up to `ENTRY_LEN + 1` either parses to
/// exactly one entry or is refused, and nothing panics. The reserved byte and both enum
/// fields are enforced, so an entry has exactly one encoding.
#[kani::proof]
#[kani::unwind(36)]
fn proof_journal_parse_total() {
    let n: usize = kani::any();
    kani::assume(n <= ENTRY_LEN + 1);
    let buf: [u8; ENTRY_LEN + 1] = kani::any();
    if let Some(e) = parse_entry(&buf[..n]) {
        assert!(n == ENTRY_LEN);
        assert!(buf[27] == 0, "a parsed entry has a zero reserved byte");
        assert!(buf[24] <= 1, "a parsed entry has a known class");
        // And it re-encodes to the same 32 bytes it came from.
        assert!(encode_entry(&e) == buf[..ENTRY_LEN]);
    }
}

/// PROVE: the decision byte space is injective and total — `Act`, `Stop` and the nine
/// reasons never collide, and no other byte decodes. A journal whose reasons could
/// collide would be worthless as evidence of *why* an order was refused.
#[kani::proof]
fn proof_decision_byte_is_injective() {
    let b: u8 = kani::any();
    match LoggedDecision::from_byte(b) {
        Some(LoggedDecision::Act) => assert!(b == DEC_ACT),
        Some(LoggedDecision::Stop) => assert!(b == DEC_STOP),
        Some(LoggedDecision::Reject(_)) => {
            assert!(b >= DEC_REJECT_BASE);
            assert!((b - DEC_REJECT_BASE) as usize < REASON_COUNT);
        }
        None => {
            assert!(b > DEC_STOP);
            assert!(b < DEC_REJECT_BASE || (b - DEC_REJECT_BASE) as usize >= REASON_COUNT);
        }
    }
    let d = any_decision();
    assert!(LoggedDecision::from_byte(d.to_byte()) == Some(d));
}

/// PROVE: `append` has no path that returns without producing an entry and advancing the
/// head. This is what makes "every decision is logged" meaningful: the caller cannot
/// obtain a decision record the head does not cover. The hash itself is not recomputed
/// here (see the module note); what is proved is that `seq` and the recorded decision
/// always match what was asked for.
#[kani::proof]
fn proof_every_decision_is_logged() {
    let mut j = Journal::new(kani::any());
    let d = any_decision();
    let class = if kani::any() { OrderClass::Act } else { OrderClass::Stop };
    let cmd_seq: u32 = kani::any();
    let fp: [u8; 8] = kani::any();
    let bytes = j.append(fp, cmd_seq, class, d, kani::any());
    let parsed = parse_entry(&bytes).unwrap();
    assert!(parsed.decision == d);
    assert!(parsed.class == class);
    assert!(parsed.cmd_seq == cmd_seq);
    assert!(parsed.origin_fp == fp);
    assert!(j.head.seq == Some(parsed.seq));
}
