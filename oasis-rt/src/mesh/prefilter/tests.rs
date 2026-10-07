use super::*;
use crate::mesh::{mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter};
use core::sync::atomic::Ordering;
use serial_test::serial;

const NET: [u8; 8] = *b"OASISnet";

fn seed(b: u8) -> MeshEdSeed {
    MeshEdSeed([b; 32])
}
fn pubof(b: u8) -> MeshEdPub {
    mesh_v10_pubkey_from_seed(&seed(b)).unwrap()
}
fn fp(b: u8) -> [u8; FP_LEN] {
    [b; FP_LEN]
}
/// A v0B/v0C router for node `id`, knowing the pubkeys of `peers`.
fn node(id: u8, peers: &[u8]) -> MeshRouter {
    let mut reg = MeshPubRegistry::new();
    for &p in peers {
        reg.insert(fp(p), pubof(p));
    }
    MeshRouter::new_v0b(fp(id), NET, seed(id), reg)
}

#[test]
fn link_key_is_symmetric() {
    // A derives K(A↔B) from its seed + B's pubkey; B derives the same from its seed +
    // A's pubkey. Both must match.
    let a_fp = [0xAA; FP_LEN];
    let b_fp = [0xBB; FP_LEN];
    let from_a = link_key(&seed(0xAA), a_fp, &pubof(0xBB), b_fp).unwrap();
    let from_b = link_key(&seed(0xBB), b_fp, &pubof(0xAA), a_fp).unwrap();
    assert_eq!(from_a, from_b);
}

#[test]
fn link_key_differs_per_peer_and_is_order_independent() {
    let a = [0xAA; FP_LEN];
    let b = [0xBB; FP_LEN];
    let c = [0xCC; FP_LEN];
    let kab = link_key(&seed(0xAA), a, &pubof(0xBB), b).unwrap();
    let kac = link_key(&seed(0xAA), a, &pubof(0xCC), c).unwrap();
    assert_ne!(kab, kac, "A↔B and A↔C must differ");
    // The fingerprint sort makes the salt order-independent; the DH is already
    // order-independent, so the two ends already match (covered above).
    assert_ne!(kab, [0u8; 32]);
}

#[test]
fn link_key_rejects_bad_peer_key() {
    // An all-zero "public key" is not a valid curve point.
    assert!(link_key(&seed(0xAA), [0xAA; FP_LEN], &MeshEdPub([0u8; 32]), [0xBB; FP_LEN]).is_none());
}

#[test]
fn hmac_sha256_rfc4231_case2() {
    // RFC 4231 test case 2 pins the crate to the real HMAC-SHA256 (not a home-grown one).
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(b"Jefe").unwrap();
    m.update(b"what do ya want for nothing?");
    let out = m.finalize().into_bytes();
    let expect = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";
    let got: String = out.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!(got, expect);
}

#[test]
fn tag_roundtrip_and_tamper() {
    let k = link_key(&seed(0xAA), [0xAA; FP_LEN], &pubof(0xBB), [0xBB; FP_LEN]).unwrap();
    let origin = [0x11; FP_LEN];
    let payload = b"setpoint=215";
    let tag = link_tag(&k, origin, 7, payload.len() as u16, payload);
    assert!(link_tag_verify(&k, origin, 7, payload.len() as u16, payload, &tag));
    // Each covered field, changed, must fail.
    assert!(!link_tag_verify(&k, [0x12; FP_LEN], 7, payload.len() as u16, payload, &tag), "origin");
    assert!(!link_tag_verify(&k, origin, 8, payload.len() as u16, payload, &tag), "counter");
    assert!(!link_tag_verify(&k, origin, 7, 99, payload, &tag), "len");
    assert!(!link_tag_verify(&k, origin, 7, payload.len() as u16, b"setpoint=216", &tag), "payload");
    // A one-bit flip of the tag itself fails.
    let mut bad = tag;
    bad[0] ^= 1;
    assert!(!link_tag_verify(&k, origin, 7, payload.len() as u16, payload, &bad), "tag bit");
    // Another link's key fails (this is what stops an outsider).
    let other = link_key(&seed(0xBB), [0xBB; FP_LEN], &pubof(0xCC), [0xCC; FP_LEN]).unwrap();
    assert!(!link_tag_verify(&other, origin, 7, payload.len() as u16, payload, &tag), "wrong link key");
}

#[test]
fn tag_is_hop_invariant_preimage_key_variant() {
    // Same origin core, two different link keys (A→B vs B→C) ⇒ two different tags, each
    // verifiable only under its own key. This is the relay recompute: same bytes, new key.
    let kab = link_key(&seed(0xAA), [0xAA; FP_LEN], &pubof(0xBB), [0xBB; FP_LEN]).unwrap();
    let kbc = link_key(&seed(0xBB), [0xBB; FP_LEN], &pubof(0xCC), [0xCC; FP_LEN]).unwrap();
    let origin = [0xAA; FP_LEN];
    let p = b"hello";
    let t_ab = link_tag(&kab, origin, 3, p.len() as u16, p);
    let t_bc = link_tag(&kbc, origin, 3, p.len() as u16, p);
    assert_ne!(t_ab, t_bc);
    assert!(link_tag_verify(&kbc, origin, 3, p.len() as u16, p, &t_bc));
    assert!(!link_tag_verify(&kbc, origin, 3, p.len() as u16, p, &t_ab));
}

#[test]
fn budget_burst_then_sustained() {
    let mut b = LinkBudget::new(BUDGET_RATE_PER_S, BUDGET_BURST);
    // Full bucket absorbs one burst at t=0.
    let mut taken = 0;
    for _ in 0..BUDGET_BURST + 5 {
        if b.try_take(0) {
            taken += 1;
        }
    }
    assert_eq!(taken, BUDGET_BURST, "exactly the burst at t=0");
    assert_eq!(b.tokens(), 0);
    // After 1 s, exactly RATE_PER_S more.
    assert!(b.try_take(1000));
    assert!(b.try_take(1000));
    assert!(!b.try_take(1000), "only rate/s after the burst is spent");
    // A long idle refills to the burst cap, not beyond.
    let mut c = LinkBudget::new(2, 24);
    for _ in 0..24 {
        c.try_take(0);
    }
    assert_eq!(c.tokens(), 0);
    c.try_take(1_000_000); // ~1000 s idle would be 2000 tokens, capped at 24
    assert_eq!(c.tokens(), 24 - 1);
}

#[test]
fn budget_flood_is_capped() {
    // 100 attempts within the same second: at most burst are allowed.
    let mut b = LinkBudget::new(2, 24);
    let mut allowed = 0;
    for _ in 0..100 {
        if b.try_take(500) {
            allowed += 1;
        }
    }
    assert_eq!(allowed, 24);
}

#[test]
fn budget_backwards_clock_is_safe() {
    let mut b = LinkBudget::new(2, 24);
    for _ in 0..24 {
        b.try_take(10_000);
    }
    assert_eq!(b.tokens(), 0);
    // Clock jumps back (reboot): no free tokens, just re-anchor.
    assert!(!b.try_take(5_000));
    assert_eq!(b.tokens(), 0);
    // Forward again from the new anchor.
    assert!(b.try_take(6_000));
}

// ── v0C envelope end-to-end (A → B → C), with the Ed25519-call counter ──────────

#[test]
#[serial]
fn v0c_relay_chain_a_b_c() {
    // A originates for B; B verifies and reseals for C; C verifies. Both hops reach
    // Ed25519 (valid traffic), the payload arrives intact.
    let mut a = node(0xAA, &[0xBB]);
    let mut b = node(0xBB, &[0xAA, 0xCC]);
    let mut c = node(0xCC, &[0xAA, 0xBB]);
    let (mut bb, mut cb) = (LinkBudget::default_budget(), LinkBudget::default_budget());
    let (mut ak, mut bk, mut ck) = (LinkKeys::new(), LinkKeys::new(), LinkKeys::new());
    let before = ED_VERIFY_CALLS.load(Ordering::Relaxed);

    let env = a.origin_wrap_v0c(b"setpoint=215", fp(0xBB), &mut ak).unwrap();
    assert_eq!(env.len(), MESH_V0C_HEADER_LEN + 12);
    assert_eq!(&env[..6], SPORE_V0C_MAGIC);

    let at_b = b.process_v0c(&env, &mut bk, &mut bb, 1000);
    let fwd = match at_b {
        MeshDecision::Arrived { forward, hops_seen, .. } => {
            assert!(forward);
            assert_eq!(hops_seen, 0);
            true
        }
        MeshDecision::Drop(r) => panic!("B dropped: {r}"),
    };
    assert!(fwd);
    let relayed = b.reseal_v0c(&env, fp(0xCC), &mut bk).unwrap();
    assert_eq!(&relayed[FWD_OFF..FWD_OFF + FP_LEN], &fp(0xBB), "forwarder rewritten to B");
    assert_eq!(relayed[30], env[30] - 1, "ttl decremented");

    match c.process_v0c(&relayed, &mut ck, &mut cb, 1000) {
        MeshDecision::Arrived { hops_seen, envelope, .. } => {
            assert_eq!(hops_seen, 1);
            assert_eq!(&envelope[MESH_V0C_HEADER_LEN..], b"setpoint=215");
        }
        MeshDecision::Drop(r) => panic!("C dropped: {r}"),
    }
    assert_eq!(ED_VERIFY_CALLS.load(Ordering::Relaxed) - before, 2, "one Ed25519 per hop");
}

#[test]
#[serial]
fn v0c_outsider_bad_tag_never_reaches_ed25519() {
    // An outsider forges an envelope claiming forwarder = A but signs the link tag with
    // a key it does not have (it is not A). B must drop it on the tag, before Ed25519.
    let mut b = node(0xBB, &[0xAA, 0xCC]);
    let mut outsider = node(0xAA, &[0xBB]); // same fp as A, but a DIFFERENT seed below
                                            // rebuild the "A" router with the real A seed to make a valid envelope, then corrupt the tag
    let mut real_a = node(0xAA, &[0xBB]);
    let mut bb = LinkBudget::default_budget();
    let (mut ak, mut bk) = (LinkKeys::new(), LinkKeys::new());
    let before = ED_VERIFY_CALLS.load(Ordering::Relaxed);

    let mut env = real_a.origin_wrap_v0c(b"x", fp(0xBB), &mut ak).unwrap();
    // flip the tag: now no key produces it → bad link tag
    env[TAG_OFF] ^= 0x01;
    assert!(matches!(b.process_v0c(&env, &mut bk, &mut bb, 0), MeshDecision::Drop("bad link tag")));
    assert_eq!(ED_VERIFY_CALLS.load(Ordering::Relaxed), before, "no Ed25519 on a bad tag");
    assert_eq!(bb.tokens(), BUDGET_BURST, "a bad tag spends no budget");

    // An unknown forwarder is dropped even earlier.
    let _ = &mut outsider;
    let mut env2 = real_a.origin_wrap_v0c(b"y", fp(0xBB), &mut ak).unwrap();
    env2[FWD_OFF] = 0x99; // forwarder 0x99.. not in B's registry
    assert!(matches!(b.process_v0c(&env2, &mut bk, &mut bb, 0), MeshDecision::Drop("unknown forwarder")));
    assert_eq!(ED_VERIFY_CALLS.load(Ordering::Relaxed), before);
}

#[test]
#[serial]
fn v0c_budget_caps_before_ed25519() {
    // A link budget of 1 token: the first valid envelope reaches Ed25519, a second valid
    // one (fresh counter) is dropped by the budget BEFORE Ed25519.
    let mut a = node(0xAA, &[0xBB]);
    let mut b = node(0xBB, &[0xAA]);
    let mut bb = LinkBudget::new(BUDGET_RATE_PER_S, 1);
    let (mut ak, mut bk) = (LinkKeys::new(), LinkKeys::new());
    let before = ED_VERIFY_CALLS.load(Ordering::Relaxed);

    let e1 = a.origin_wrap_v0c(b"a", fp(0xBB), &mut ak).unwrap();
    let e2 = a.origin_wrap_v0c(b"b", fp(0xBB), &mut ak).unwrap();
    assert!(matches!(b.process_v0c(&e1, &mut bk, &mut bb, 0), MeshDecision::Arrived { .. }));
    assert!(matches!(b.process_v0c(&e2, &mut bk, &mut bb, 0), MeshDecision::Drop("link budget exceeded")));
    assert_eq!(ED_VERIFY_CALLS.load(Ordering::Relaxed) - before, 1, "budget stops the 2nd before Ed25519");
}

#[test]
#[serial]
fn v0c_refuses_downgrade_to_v0b() {
    // A v0B envelope handed to a v0C node is dropped at the magic check, no Ed25519.
    let mut a = node(0xAA, &[0xBB]);
    let mut b = node(0xBB, &[0xAA]);
    let mut bb = LinkBudget::default_budget();
    let mut bk = LinkKeys::new();
    let before = ED_VERIFY_CALLS.load(Ordering::Relaxed);
    let v0b = a.origin_wrap_v0b(b"x").unwrap();
    assert!(matches!(b.process_v0c(&v0b, &mut bk, &mut bb, 0), MeshDecision::Drop("not a v0C envelope (downgrade refused)")));
    assert_eq!(ED_VERIFY_CALLS.load(Ordering::Relaxed), before);
}

#[test]
#[serial]
fn v0c_replay_refused() {
    // The same v0C envelope twice: the second is a counter replay (after Ed25519 here,
    // since the tag and budget both pass — matches v0B semantics).
    let mut a = node(0xAA, &[0xBB]);
    let mut b = node(0xBB, &[0xAA]);
    let mut bb = LinkBudget::default_budget();
    let (mut ak, mut bk) = (LinkKeys::new(), LinkKeys::new());
    let env = a.origin_wrap_v0c(b"once", fp(0xBB), &mut ak).unwrap();
    assert!(matches!(b.process_v0c(&env, &mut bk, &mut bb, 0), MeshDecision::Arrived { .. }));
    assert!(matches!(b.process_v0c(&env, &mut bk, &mut bb, 0), MeshDecision::Drop(_)));
}

#[test]
#[serial] // shares the global ED_VERIFY_CALLS counter with the other v0C tests
fn link_keys_derive_once_per_peer() {
    // The whole point of the cache: one X25519 per peer, however many frames arrive.
    // On a Cortex-M0+ an X25519 costs about as much as the Ed25519 verify the filter is
    // meant to avoid, so per-frame derivation would double the cost (measured 370 ms/
    // frame on silicon, 2026-10-07) instead of pre-filtering it.
    let mut a = node(0xAA, &[0xBB]);
    let mut b = node(0xBB, &[0xAA]);
    let (mut ak, mut bk) = (LinkKeys::new(), LinkKeys::new());
    let mut bb = LinkBudget::default_budget();
    for _ in 0..5 {
        let env = a.origin_wrap_v0c(b"z", fp(0xBB), &mut ak).unwrap();
        let _ = b.process_v0c(&env, &mut bk, &mut bb, 0);
    }
    assert_eq!(ak.derivations(), 1, "origin derives once for its next hop");
    assert_eq!(bk.derivations(), 1, "relay derives once for its forwarder");
    // Forgetting a peer (revocation, re-enrolment) forces one fresh derivation.
    bk.forget(&fp(0xAA));
    let env = a.origin_wrap_v0c(b"z", fp(0xBB), &mut ak).unwrap();
    let _ = b.process_v0c(&env, &mut bk, &mut bb, 0);
    assert_eq!(bk.derivations(), 2);
}

#[test]
#[serial] // shares the global ED_VERIFY_CALLS counter with the other v0C tests
fn v0c_insider_valid_tag_bad_signature_is_capped_by_the_budget() {
    // The insider model: a node that holds the link key (B does, legitimately) can make a
    // frame with a VALID tag and a broken signature, because the tag covers
    // origin/counter/length/payload but NOT the signature. It passes the pre-filter, so
    // only the budget stops it forcing Ed25519 verifications.
    let mut a = node(0xAA, &[0xBB]);
    let mut b = node(0xBB, &[0xAA]);
    let (mut ak, mut bk) = (LinkKeys::new(), LinkKeys::new());
    let mut bb = LinkBudget::new(BUDGET_RATE_PER_S, 3); // burst 3, so the cap is visible
    let before = ED_VERIFY_CALLS.load(Ordering::Relaxed);

    let env = a.origin_wrap_v0c(b"insider", fp(0xBB), &mut ak).unwrap();
    // Break the signature (byte 40 is inside the 35..99 signature field); the tag stays valid.
    let mut bad = env.clone();
    bad[40] ^= 0x01;
    // The tag must still verify, so the frame reaches the budget and then Ed25519.
    let mut reached = 0;
    let mut capped = 0;
    for _ in 0..10 {
        match b.process_v0c(&bad, &mut bk, &mut bb, 0) {
            MeshDecision::Drop("bad mesh signature") => reached += 1,
            MeshDecision::Drop("link budget exceeded") => capped += 1,
            d => panic!("unexpected: {d:?}"),
        }
    }
    assert_eq!(reached, 3, "exactly the burst reached Ed25519");
    assert_eq!(capped, 7, "the rest were refused before it");
    assert_eq!(ED_VERIFY_CALLS.load(Ordering::Relaxed) - before, 3, "the budget bounds the verifications");
}
