use super::*;
use crate::mesh::{mesh_v10_pubkey_from_seed, MeshEdSeed};

fn seed(b: u8) -> MeshEdSeed {
    MeshEdSeed([b; 32])
}
fn pubof(b: u8) -> MeshEdPub {
    mesh_v10_pubkey_from_seed(&seed(b)).unwrap()
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
