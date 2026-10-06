use super::*;

// Unwind >= 9 for the [u8; 8] fingerprint memcmp (see mesh_revocation/kani_proofs.rs);
// registries here hold at most 2 entries.

fn any_entry() -> Entry {
    Entry { fp: kani::any(), pk: [0u8; 32], role: role::RELAY, permissions: kani::any(), seq: kani::any() }
}

fn any_registry() -> Registry {
    let mut entries = Vec::new();
    let n: u8 = kani::any();
    kani::assume(n <= 2);
    for _ in 0..n {
        entries.push(any_entry());
    }
    kani::assume(entries.len() < 2 || entries[0].fp != entries[1].fp);
    Registry { gen: kani::any(), entries }
}

fn any_attestation() -> Attestation {
    Attestation { node_pk: [0u8; 32], role: role::RELAY, permissions: kani::any(), enroll_seq: kani::any() }
}

/// PROVE: a revoked fingerprint is never enrolled, and a rejected attestation never
/// yields a new registry.
#[kani::proof]
#[kani::unwind(10)]
fn proof_enr_revoked_never_enrolled() {
    let reg = any_registry();
    let fp: Fp = kani::any();
    let a = any_attestation();
    let revoked: bool = kani::any();
    let (d, new) = enrollment_transition(&reg, &fp, &a, revoked);
    if revoked {
        assert!(d == EnrollDecision::Reject(EnrollReject::Revoked) && new.is_none());
    }
    if let EnrollDecision::Reject(_) = d {
        assert!(new.is_none());
    }
}

/// PROVE: a node's `enroll_seq` never decreases: an attestation for a node already
/// held is accepted only with a strictly higher sequence, and installs exactly it.
/// (Concrete shape, one held entry with the same fingerprint: with a symbolic
/// 0-to-2 entry registry CBMC ran out of memory on the 3.3 GB WSL VM, runs 1 and
/// a dev run, evidence/kani/2026-10-06/enroll/.)
#[kani::proof]
#[kani::unwind(10)]
fn proof_enr_seq_never_decreases() {
    let held = any_entry();
    let mut entries = Vec::new();
    entries.push(held);
    let reg = Registry { gen: kani::any(), entries };
    let a = any_attestation();
    let (d, new) = enrollment_transition(&reg, &held.fp, &a, false);
    match new {
        Some(n) => {
            assert!(a.enroll_seq > held.seq && d == EnrollDecision::Updated);
            assert!(n.entries.len() == 1 && n.entries[0].seq == a.enroll_seq);
        }
        None => assert!(a.enroll_seq <= held.seq && d == EnrollDecision::Reject(EnrollReject::StaleSeq)),
    }
}

/// PROVE: enrolling one node leaves another node's entry unchanged (compared field
/// by field; keys are fixed in this harness).
#[kani::proof]
#[kani::unwind(10)]
fn proof_enr_other_entries_untouched() {
    let other = any_entry();
    let mut entries = Vec::new();
    entries.push(other);
    let reg = Registry { gen: kani::any(), entries };
    let fp: Fp = kani::any();
    kani::assume(fp != other.fp);
    let a = any_attestation();
    let (_, new) = enrollment_transition(&reg, &fp, &a, false);
    if let Some(n) = new {
        let o = n.entries.iter().find(|e| e.fp == other.fp).unwrap();
        assert!(o.seq == other.seq && o.permissions == other.permissions && o.role == other.role);
        assert!(n.entries.len() == 2);
    }
}

/// PROVE: `parse_attestation` never panics; an accepted one has a known role and
/// no reserved permission bit.
#[kani::proof]
fn proof_enr_parse_total() {
    let buf: [u8; ATT_LEN + 1] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= ATT_LEN + 1);
    if let Ok(a) = parse_attestation(&buf[..len]) {
        assert!(len == ATT_LEN);
        assert!(a.role >= role::SENSOR && a.role <= role::ACTUATOR);
        assert!(a.permissions & !perm::KNOWN == 0);
    }
}
