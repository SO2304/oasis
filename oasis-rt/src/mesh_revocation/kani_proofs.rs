use super::*;

// Unwind bound: every [u8; 8] comparison (fingerprints, network_id) compiles to a
// memcmp loop of 8 iterations, so the bound must be >= 9. With 4, CBMC hit the
// memcmp unwinding assertion and left every property UNDETERMINED (first run,
// evidence/kani/2026-10-06/kani_new_harnesses.log). The list loops are <= 3.

fn small_list(len: u8) -> Vec<Fp> {
    // Strictly ascending symbolic fingerprints (bounded length for Kani).
    let mut v = Vec::new();
    let mut last: u8 = kani::any();
    kani::assume(last < 200);
    for _ in 0..len {
        let step: u8 = kani::any();
        kani::assume(step > 0 && step < 20);
        last += step;
        v.push([last; FP_LEN]);
    }
    v
}

/// PROVE: the revocation epoch never decreases, whatever the input.
#[kani::proof]
#[kani::unwind(10)]
fn proof_rev_epoch_never_decreases() {
    let state = RevState { epoch: kani::any(), revoked: small_list(1) };
    let p = ParsedRevocation { network_id: [1; 8], epoch: kani::any(), issued_at: 0, fps: small_list(2), sigs: Vec::new() };
    let (_, new) = revocation_transition(&state, &[1; 8], &p, kani::any());
    if let Some(n) = new {
        assert!(n.epoch > state.epoch);
    }
}

/// PROVE: a rejected (or duplicate) list yields no new state, and Applied needs
/// a valid operator signature, the right network and a strictly newer epoch.
#[kani::proof]
#[kani::unwind(10)]
fn proof_rev_rejected_leaves_state_unchanged() {
    let state = RevState { epoch: kani::any(), revoked: small_list(1) };
    let net: [u8; 8] = kani::any();
    let mine: [u8; 8] = kani::any();
    let sig_ok: bool = kani::any();
    let p = ParsedRevocation { network_id: net, epoch: kani::any(), issued_at: 0, fps: small_list(2), sigs: Vec::new() };
    let (d, new) = revocation_transition(&state, &mine, &p, sig_ok);
    match d {
        RevDecision::Applied => {
            assert!(sig_ok && net == mine && p.epoch > state.epoch);
            assert!(new.is_some());
        }
        _ => assert!(new.is_none()),
    }
}

/// PROVE: an applied list never drops a fingerprint already revoked.
#[kani::proof]
#[kani::unwind(10)]
fn proof_rev_applied_is_superset() {
    let state = RevState { epoch: 0, revoked: small_list(1) };
    let p = ParsedRevocation { network_id: [1; 8], epoch: 1, issued_at: 0, fps: small_list(2), sigs: Vec::new() };
    let (_, new) = revocation_transition(&state, &[1; 8], &p, true);
    if let Some(n) = new {
        for f in &state.revoked {
            assert!(n.revoked.contains(f));
        }
    }
}
