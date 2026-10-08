use super::*;
use crate::actuation::{Actuator, Decision, GateInput, Reason, StopInput};

const BOOT: u64 = 0x1234_5678_9abc_def0;
const FP: [u8; 8] = [0xA1; 8];

fn log_n(n: u32) -> (Journal, std::vec::Vec<[u8; ENTRY_LEN]>) {
    let mut j = Journal::new(BOOT);
    let mut v = std::vec::Vec::new();
    for i in 0..n {
        let d = if i % 3 == 0 {
            LoggedDecision::Act
        } else if i % 3 == 1 {
            LoggedDecision::Reject(Reason::StaleOrReplayed)
        } else {
            LoggedDecision::Stop
        };
        v.push(j.append(FP, i, OrderClass::Act, d, FLAG_R14_SAFE | FLAG_WITHIN_LIMITS));
    }
    (j, v)
}

#[test]
fn jrn_chain_of_100_verifies() {
    let (j, v) = log_n(100);
    assert_eq!(verify(BOOT, &v, &j.head), VerifyResult::Intact { entries: 100 });
    assert_eq!(j.head.seq, Some(99));
}

#[test]
fn jrn_empty_journal_is_intact() {
    let j = Journal::new(BOOT);
    assert_eq!(verify(BOOT, &[], &j.head), VerifyResult::Intact { entries: 0 });
}

/// A single bit changed in the middle of the journal is detected, and at the right place.
#[test]
fn jrn_modified_entry_is_detected() {
    let (j, mut v) = log_n(100);
    v[37][20] ^= 0x01;
    match verify(BOOT, &v, &j.head) {
        VerifyResult::Broken { at } => assert!(at >= 37, "detected at or after the tampered entry, got {at}"),
        other => panic!("tampering not detected: {other:?}"),
    }
}

/// Every byte of every entry is covered by the chain — not just the fields we remembered
/// to hash. Exhaustive over the 32 bytes of one entry.
#[test]
fn jrn_every_byte_of_an_entry_is_chained() {
    let (j, v) = log_n(3);
    for byte in 0..ENTRY_LEN {
        for bit in 0..8u32 {
            let mut t = v.clone();
            t[1][byte] ^= 1 << bit;
            if t[1] == v[1] {
                continue;
            }
            assert_ne!(verify(BOOT, &t, &j.head), VerifyResult::Intact { entries: 3 }, "byte {byte} bit {bit} is not covered by the chain");
        }
    }
}

/// A removed entry breaks the chain AND leaves a hole in `seq`. The spec asks for both to
/// be distinguishable, because the causes differ.
#[test]
fn jrn_deleted_entry_shows_a_seq_gap() {
    let (j, mut v) = log_n(100);
    v.remove(50);
    assert_eq!(verify(BOOT, &v, &j.head), VerifyResult::SeqGap { expected: 50, found: 51 });
}

#[test]
fn jrn_inserted_entry_is_detected() {
    let (j, mut v) = log_n(10);
    let mut extra = Journal::new(BOOT);
    let forged = extra.append(FP, 999, OrderClass::Act, LoggedDecision::Act, 0);
    v.insert(5, forged);
    assert!(!matches!(verify(BOOT, &v, &j.head), VerifyResult::Intact { .. }), "an inserted entry must not verify");
}

#[test]
fn jrn_truncated_journal_is_detected() {
    let (j, mut v) = log_n(100);
    v.truncate(99);
    assert_eq!(verify(BOOT, &v, &j.head), VerifyResult::Broken { at: 98 });
}

/// A power cut between writing the entry and updating the head: the chain is intact up to
/// the head and one entry is left over. Reported, not hidden.
#[test]
fn jrn_power_cut_leaves_one_unconfirmed_entry() {
    let mut j = Journal::new(BOOT);
    let mut v = std::vec::Vec::new();
    for i in 0..10 {
        v.push(j.append(FP, i, OrderClass::Act, LoggedDecision::Act, 0));
    }
    let head_before = j.head;
    // The entry is written but the head update never lands.
    let orphan = {
        let mut j2 = j;
        j2.append(FP, 10, OrderClass::Act, LoggedDecision::Act, 0)
    };
    v.push(orphan);
    assert_eq!(verify(BOOT, &v, &head_before), VerifyResult::Unconfirmed { entries: 10 });
}

/// The chain is bound to the boot: replaying a whole journal under a different boot id
/// does not verify.
#[test]
fn jrn_chain_is_bound_to_the_boot_id() {
    let (j, v) = log_n(5);
    assert!(!matches!(verify(BOOT ^ 1, &v, &j.head), VerifyResult::Intact { .. }));
    assert_ne!(genesis(1), genesis(2));
}

#[test]
fn jrn_entry_roundtrip_and_parser_is_strict() {
    let e = Entry {
        seq: 42,
        boot_id: BOOT,
        origin_fp: FP,
        cmd_seq: 7,
        class: OrderClass::Stop,
        decision: LoggedDecision::Reject(Reason::SupervisionLost),
        flags: FLAG_STOPPED,
    };
    let b = encode_entry(&e);
    assert_eq!(parse_entry(&b), Some(e));
    assert_eq!(parse_entry(&[]), None);
    assert_eq!(parse_entry(&b[..ENTRY_LEN - 1]), None);

    // Every reserved byte, not just the first. The first version of this module checked
    // only byte 27, and `proof_journal_parse_total` refuted the round trip because bytes
    // 28..32 were a hole: an entry had more than one encoding.
    for off in RESERVED_OFF..ENTRY_LEN {
        let mut r = b;
        r[off] = 1;
        assert_eq!(parse_entry(&r), None, "reserved byte {off} must be zero");
    }
    // And a parsed entry re-encodes to exactly the bytes it came from.
    assert_eq!(encode_entry(&parse_entry(&b).unwrap()), b);
    let mut c = b;
    c[24] = 9;
    assert_eq!(parse_entry(&c), None, "unknown order class");
    let mut d = b;
    d[25] = 200;
    assert_eq!(parse_entry(&d), None, "unknown decision");
}

/// Every reason, and both acceptances, survive a round trip through one byte.
#[test]
fn jrn_every_decision_encodes_distinctly() {
    let mut seen = std::vec::Vec::new();
    let all = [
        LoggedDecision::Act,
        LoggedDecision::Stop,
        LoggedDecision::Reject(Reason::NotVerified),
        LoggedDecision::Reject(Reason::NotAuthorized),
        LoggedDecision::Reject(Reason::Revoked),
        LoggedDecision::Reject(Reason::Expired),
        LoggedDecision::Reject(Reason::R14Unsafe),
        LoggedDecision::Reject(Reason::OutOfLimits),
        LoggedDecision::Reject(Reason::StaleOrReplayed),
        LoggedDecision::Reject(Reason::Stopped),
        LoggedDecision::Reject(Reason::SupervisionLost),
        LoggedDecision::Reject(Reason::QuorumMissing),
    ];
    for d in all {
        let b = d.to_byte();
        assert!(!seen.contains(&b), "decision byte {b} is used twice");
        seen.push(b);
        assert_eq!(LoggedDecision::from_byte(b), Some(d));
    }
    assert_eq!(seen.len(), 2 + crate::actuation::REASON_COUNT);
}

/// End to end: a gate that accepts and refuses, with the journal recording both. This is
/// the behaviour Annex III 1.1.9 asks for — "légitime **ou illégitime**".
#[test]
fn jrn_records_refusals_as_well_as_acceptances() {
    fn ok() -> GateInput {
        GateInput {
            v0b_ok: true,
            authorized: true,
            revoked: false,
            cmd_boot_id: 7,
            deadline_ms: 3_000,
            actuator_boot_id: 7,
            now_ms: 1_000,
            r14_safe: true,
            within_limits: true,
            cmd_seq: 1,
            last_executed_seq: None,
        }
    }
    let mut a = Actuator::new();
    let mut j = Journal::new(BOOT);
    let mut v = std::vec::Vec::new();

    let mut record = |a: &mut Actuator, j: &mut Journal, v: &mut std::vec::Vec<[u8; ENTRY_LEN]>, i: GateInput| {
        let d = a.decide(i);
        let flags = if i.r14_safe { FLAG_R14_SAFE } else { 0 };
        v.push(j.append(FP, i.cmd_seq, OrderClass::Act, d.into(), flags));
        d
    };

    assert_eq!(record(&mut a, &mut j, &mut v, ok()), Decision::Act);
    assert_eq!(record(&mut a, &mut j, &mut v, GateInput { cmd_seq: 2, r14_safe: false, ..ok() }), Decision::Reject(Reason::R14Unsafe));
    let sd = a.decide_stop(&StopInput { v0b_ok: true, stop_authorized: true, revoked: false });
    v.push(j.append(FP, 3, OrderClass::Stop, sd.into(), FLAG_STOPPED));
    assert_eq!(record(&mut a, &mut j, &mut v, GateInput { cmd_seq: 4, ..ok() }), Decision::Reject(Reason::Stopped));

    assert_eq!(verify(BOOT, &v, &j.head), VerifyResult::Intact { entries: 4 });
    let decisions: std::vec::Vec<LoggedDecision> = v.iter().map(|b| parse_entry(b).unwrap().decision).collect();
    assert_eq!(decisions, [LoggedDecision::Act, LoggedDecision::Reject(Reason::R14Unsafe), LoggedDecision::Stop, LoggedDecision::Reject(Reason::Stopped),]);
}
