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

/// Cheap stand-in for [`chain`] so the two control-flow harnesses terminate.
///
/// Both `Journal::new` (via [`genesis`]) and `Journal::append` (via [`chain`]) hash, and a
/// GOTO program containing sha2 is killed by the OOM killer on this machine at the
/// instrumentation stage, before CBMC even solves (`goto-instrument exited with status
/// signal: 9`, "Trying to force one backedge per target") — the same wall hit in phase 1.2.
///
/// A first attempt stubbed only `chain` and **still** died: the diagnosis was incomplete,
/// because `Journal::new` hashes too. Both are stubbed now. The stubs keep the *shape* of
/// the functions (a value derived from the inputs) and drop the cryptography.
///
/// ⚠️ **What that costs.** `proof_journal_seq_monotone` and `proof_every_decision_is_logged`
/// therefore prove **sequencing and content preservation, not collision resistance**. The
/// chain's cryptographic behaviour is covered by tests instead, including
/// `jrn_every_byte_of_an_entry_is_chained`, which flips every bit of every byte. Any
/// report of these two harnesses must say they ran under a stub.
#[cfg(kani)]
fn genesis_stub(boot_id: u64) -> [u8; 32] {
    let mut h = [0u8; 32];
    h[..8].copy_from_slice(&boot_id.to_le_bytes());
    h
}

#[cfg(kani)]
fn chain_stub(prev: &[u8; 32], entry: &[u8; ENTRY_LEN]) -> [u8; 32] {
    let mut h = *prev;
    for (i, b) in h.iter_mut().enumerate() {
        *b ^= entry[i % ENTRY_LEN];
    }
    h
}

fn any_decision() -> LoggedDecision {
    let k: u8 = kani::any();
    kani::assume(k < 4);
    match k {
        0 => LoggedDecision::Act,
        1 => LoggedDecision::Stop,
        2 => {
            let r: u8 = kani::any();
            kani::assume((r as usize) < REASON_COUNT);
            LoggedDecision::Reject(reason_from_index(r).unwrap())
        }
        // The third region, added for Annex III 1.1.9 para 5. Leaving it out would make
        // every harness below silent about exactly the records that were added.
        _ => {
            let c: u8 = kani::any();
            kani::assume((c as usize) < CHANGE_KIND_COUNT);
            LoggedDecision::Change(ChangeKind::from_index(c).unwrap())
        }
    }
}

/// PROVE: nothing in an entry is lost or altered on the way to bytes and back. Every
/// field is symbolic, so this covers all entries, and it is what guarantees the chain
/// hashes the whole entry rather than part of it.
#[kani::proof]
fn proof_entry_roundtrip_is_lossless() {
    let decision = any_decision();
    let e = Entry {
        seq: kani::any(),
        boot_id: kani::any(),
        origin_fp: kani::any(),
        cmd_seq: kani::any(),
        // A change has exactly one valid class byte (see `parse_entry`), so only a real
        // decision may carry a symbolic one.
        class: if decision.is_change() || kani::any() { OrderClass::Act } else { OrderClass::Stop },
        decision,
        flags: kani::any(),
    };
    assert!(parse_entry(&encode_entry(&e)) == Some(e));
}

/// PROVE: a change record keeps everything an auditor reads off it — which change, which
/// identifying number, who authorised it, and whether it was applied or refused.
///
/// Annex III 1.1.9 para 5 asks for evidence of « une intervention legitime **ou
/// illegitime** » in the software or a modification of it or of its configuration, so a
/// record that lost the *refused* bit, or the authority, would not be the evidence the
/// clause asks for even though the chain over it still verified.
#[kani::proof]
#[kani::stub(super::chain, chain_stub)]
#[kani::stub(super::genesis, genesis_stub)]
fn proof_change_record_preserves_what_an_auditor_reads() {
    let c: u8 = kani::any();
    kani::assume((c as usize) < CHANGE_KIND_COUNT);
    let kind = ChangeKind::from_index(c).unwrap();
    let ident: u32 = kani::any();
    let fp: [u8; 8] = kani::any();
    let applied: bool = kani::any();

    let mut j = Journal::new(kani::any());
    let bytes = j.append_change(fp, ident, kind, applied);
    let e = parse_entry(&bytes).expect("a change this module wrote must parse");

    assert!(e.decision == LoggedDecision::Change(kind));
    assert!(e.decision.is_change());
    assert!(e.cmd_seq == ident);
    assert!(e.origin_fp == fp);
    assert!((e.flags & FLAG_CHANGE_APPLIED != 0) == applied);
    // And a change is never mistaken for a decision about an order.
    assert!(e.decision != LoggedDecision::Act);
    assert!(e.decision != LoggedDecision::Stop);
}

/// PROVE: `seq` advances by exactly one per append and `next_seq` never panics.
#[kani::proof]
#[kani::stub(super::chain, chain_stub)]
#[kani::stub(super::genesis, genesis_stub)]
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
        // A change entry has exactly one valid class byte, so its encoding is injective.
        if e.decision.is_change() {
            assert!(buf[24] == OrderClass::Act as u8);
        }
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
            assert!(((b - DEC_REJECT_BASE) as usize) < REASON_COUNT);
            assert!(b < DEC_CHANGE_BASE, "a refusal must never read as a change");
        }
        Some(LoggedDecision::Change(_)) => {
            assert!(b >= DEC_CHANGE_BASE);
            assert!(((b - DEC_CHANGE_BASE) as usize) < CHANGE_KIND_COUNT);
        }
        None => {
            assert!(b > DEC_STOP);
            assert!(b < DEC_REJECT_BASE || (b - DEC_REJECT_BASE) as usize >= REASON_COUNT);
            assert!(b < DEC_CHANGE_BASE || (b - DEC_CHANGE_BASE) as usize >= CHANGE_KIND_COUNT);
        }
    }
    // The three regions are disjoint by construction, which is only true while the
    // refusals cannot grow into the change region. Asserted so adding a `Reason` fails
    // here rather than silently turning a refusal into a configuration change.
    assert!(DEC_REJECT_BASE as usize + REASON_COUNT <= DEC_CHANGE_BASE as usize);
    let d = any_decision();
    assert!(LoggedDecision::from_byte(d.to_byte()) == Some(d));
}

/// PROVE: `append` has no path that returns without producing an entry and advancing the
/// head. This is what makes "every decision is logged" meaningful: the caller cannot
/// obtain a decision record the head does not cover. The hash itself is not recomputed
/// here (see the module note); what is proved is that `seq` and the recorded decision
/// always match what was asked for.
#[kani::proof]
#[kani::stub(super::chain, chain_stub)]
#[kani::stub(super::genesis, genesis_stub)]
fn proof_every_decision_is_logged() {
    let mut j = Journal::new(kani::any());
    let d = any_decision();
    // A change entry has no order class and `parse_entry` enforces byte 24 == Act, so a
    // symbolic class is only sound for a real decision. Without this the harness would
    // be refuted on a genuine property rather than a bug.
    let class = if d.is_change() || kani::any() { OrderClass::Act } else { OrderClass::Stop };
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
