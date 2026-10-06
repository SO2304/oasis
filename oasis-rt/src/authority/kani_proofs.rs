use super::*;

// Unwind bound: the [u8; 4] magic and [u8; 8] network comparisons compile to
// memcmp loops of <= 8 iterations, so the bound must be >= 9 (see
// mesh_revocation/kani_proofs.rs). Signature verification itself is not modelled:
// these proofs cover the pure decision rules around it.

/// PROVE: the hybrid suite is satisfied only when BOTH signatures verified, and a
/// reserved or unknown suite is never satisfied.
#[kani::proof]
fn proof_auth_hybrid_requires_both() {
    let suite: u8 = kani::any();
    let ed: bool = kani::any();
    let ml: bool = kani::any();
    let ok = signatures_satisfy(suite, ed, ml);
    if suite == SUITE_HYBRID {
        assert_eq!(ok, ed && ml);
    }
    if suite != SUITE_HYBRID && suite != SUITE_ED25519 {
        assert!(!ok);
    }
}

/// PROVE: `raise` never lowers the minimum suite of any kind, never installs an
/// unknown suite, and the POLICY kind stays hybrid.
#[kani::proof]
#[kani::unwind(10)]
fn proof_auth_policy_never_lowers() {
    let mut p = AuthPolicy::default();
    // Arbitrary reachable policy: two arbitrary raises from the default.
    p.raise(kani::any(), kani::any());
    p.raise(kani::any(), kani::any());
    let before = p;
    let changed = p.raise(kani::any(), kani::any());
    for k in 1..=KIND_COUNT as u8 {
        assert!(suite_rank(p.min_suite(k)) >= suite_rank(before.min_suite(k)));
        assert!(suite_rank(p.min_suite(k)) > 0, "always a known suite");
    }
    assert_eq!(p.min_suite(kind::POLICY), SUITE_HYBRID);
    assert_eq!(changed, p != before);
}

/// PROVE: `parse_oau1` never panics, and an accepted message has a known kind, a
/// supported suite, a 64-byte Ed25519 signature and exact lengths.
#[kani::proof]
#[kani::unwind(10)]
fn proof_auth_parse_total() {
    const N: usize = 96;
    let buf: [u8; N] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= N);
    if let Ok(p) = parse_oau1(&buf[..len]) {
        assert!(p.kind >= 1 && p.kind as usize <= KIND_COUNT);
        assert!(p.suite == SUITE_ED25519 || p.suite == SUITE_HYBRID);
        assert_eq!(p.ed25519_sig.len(), ED25519_SIG_LEN);
        // A hybrid message cannot fit in 96 bytes: only Ed25519 is reachable here.
        assert!(p.mldsa44_sig.is_empty());
        assert_eq!(HEADER_LEN + p.content.len() + ED25519_SIG_LEN, len);
    }
}

/// PROVE: `precheck` passes only on the right network and with a suite at least as
/// strong as the policy requires for that kind (no downgrade).
#[kani::proof]
#[kani::unwind(10)]
fn proof_auth_precheck_no_downgrade() {
    let mut pol = AuthPolicy::default();
    pol.raise(kani::any(), kani::any());
    let mine: [u8; 8] = kani::any();
    let p = ParsedAuthority { kind: kani::any(), suite: kani::any(), network_id: kani::any(), content: &[], ed25519_sig: &[], mldsa44_sig: &[] };
    if precheck(&pol, &mine, &p).is_ok() {
        assert!(p.network_id == mine);
        assert!(suite_rank(p.suite) >= suite_rank(pol.min_suite(p.kind)));
        assert!(suite_rank(p.suite) > 0, "known kind and suite only");
    }
}
