//! Kani harnesses for C9.
//!
//! `quorum_ok` is a `bool` here for the same reason `v0b_ok` is in the base rule: Ed25519
//! under CBMC does not terminate on this machine, so the cryptography is the caller's job
//! and the **rule** is what gets proved. The quorum's cryptographic behaviour — k distinct
//! operators, binding to the order body and to the network — is covered by tests against
//! the real `OperatorAuthority`, including the exhaustive per-field tamper loop in
//! `q_signatures_are_bound_to_the_order`.

use super::*;
use crate::actuation::GateInput;

fn any_input() -> GateInput {
    GateInput {
        v0b_ok: kani::any(),
        authorized: kani::any(),
        revoked: kani::any(),
        cmd_boot_id: kani::any(),
        deadline_ms: kani::any(),
        actuator_boot_id: kani::any(),
        now_ms: kani::any(),
        r14_safe: kani::any(),
        within_limits: kani::any(),
        cmd_seq: kani::any(),
        last_executed_seq: if kani::any() { Some(kani::any()) } else { None },
    }
}

fn any_ctx() -> GateContext {
    GateContext { stopped: kani::any(), supervision_expired: kani::any() }
}

/// PROVE (**the invariant of C9**): a critical order is never executed without a quorum.
/// For every input and every actuator state, `Act` implies `quorum_ok`.
#[kani::proof]
fn proof_quorum_required_to_act() {
    let i = any_input();
    let ctx = any_ctx();
    if quorum_decision(&ctx, &i, false) == Decision::Act {
        assert!(false, "acted without a quorum");
    }
}

/// PROVE: a quorum excuses nothing. With `quorum_ok` true, `Act` still implies every one of
/// the base rule's conditions — the two-person rule is an extra gate, never a bypass.
#[kani::proof]
fn proof_quorum_does_not_bypass_the_base_rule() {
    let i = any_input();
    let ctx = any_ctx();
    if quorum_decision(&ctx, &i, true) == Decision::Act {
        assert!(i.v0b_ok);
        assert!(i.authorized);
        assert!(!i.revoked);
        assert!(!ctx.stopped);
        assert!(!ctx.supervision_expired);
        assert!(i.cmd_boot_id == i.actuator_boot_id);
        assert!(i.now_ms <= i.deadline_ms);
        assert!(i.r14_safe);
        assert!(i.within_limits);
        if let Some(last) = i.last_executed_seq {
            assert!(i.cmd_seq > last);
        }
        // And it agrees with the ordinary rule on the same input.
        assert!(quorum_decision(&ctx, &i, true) == actuation_decision_ctx(&ctx, &i));
    }
}

/// PROVE: an unauthenticated or unauthorised sender is told so, and never told
/// `QuorumMissing` — it learns nothing about the quorum policy.
#[kani::proof]
fn proof_quorum_not_leaked_before_authorisation() {
    let i = any_input();
    let ctx = any_ctx();
    let q: bool = kani::any();
    let d = quorum_decision(&ctx, &i, q);
    if d == Decision::Reject(Reason::QuorumMissing) {
        assert!(i.v0b_ok);
        assert!(i.authorized);
        assert!(!q);
    }
}

/// PROVE: the `OAQ1` parser is total. Any slice either yields one order whose length,
/// reserved byte and signature count all agree, or nothing. `n_sigs == 0` is refused, so a
/// quorum order can never be mistaken for an ordinary order lacking a quorum.
#[kani::proof]
#[kani::unwind(5)]
fn proof_oaq1_parse_total() {
    // One signature keeps the symbolic buffer tractable; the length check is linear in
    // n_sigs and `oaq1_len` is proved consistent below.
    const N: usize = OAQ1_BODY_LEN + 1 + QUORUM_SIG_LEN;
    let n: usize = kani::any();
    kani::assume(n <= N);
    let buf: [u8; N] = kani::any();
    if let Some(o) = parse_oaq1(&buf[..n]) {
        assert!(o.n_sigs >= 1);
        assert!((o.n_sigs as usize) <= MAX_QUORUM_SIGS);
        assert!(n == oaq1_len(o.n_sigs as usize));
        assert!(buf[50] == 0);
    }
}

/// PROVE: `oaq1_len` is monotone and never overflows for any accepted count, so a length
/// check can never wrap into accepting a short buffer.
#[kani::proof]
fn proof_oaq1_len_is_sane() {
    let n: usize = kani::any();
    kani::assume(n <= MAX_QUORUM_SIGS);
    let l = oaq1_len(n);
    assert!(l >= OAQ1_BODY_LEN + 1);
    assert!(l == OAQ1_BODY_LEN + 1 + n * QUORUM_SIG_LEN);
    if n >= 1 {
        assert!(l > oaq1_len(n - 1));
    }
}
