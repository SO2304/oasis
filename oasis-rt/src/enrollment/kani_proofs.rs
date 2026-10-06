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

/// PROVE: a node's `enroll_seq` never decreases; an accepted attestation installs
/// exactly its sequence, strictly above the one held, and leaves other nodes alone.
#[kani::proof]
#[kani::unwind(10)]
fn proof_enr_seq_never_decreases() {
    let reg = any_registry();
    let fp: Fp = kani::any();
    let a = any_attestation();
    let (_, new) = enrollment_transition(&reg, &fp, &a, false);
    if let Some(n) = new {
        let installed = n.entries.iter().find(|e| e.fp == fp).unwrap();
        assert_eq!(installed.seq, a.enroll_seq);
        if let Some(old) = reg.entries.iter().find(|e| e.fp == fp) {
            assert!(a.enroll_seq > old.seq);
        }
        for e in &reg.entries {
            if e.fp != fp {
                assert!(n.entries.contains(e));
            }
        }
        assert!(n.entries.len() <= MAX_ENTRIES);
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
