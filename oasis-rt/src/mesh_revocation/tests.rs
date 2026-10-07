use super::*;
use crate::mesh::{inner_slice, mesh_v10_pubkey_from_seed, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter};

const NET: [u8; 8] = *b"OASISnet";
const OP_SEED: [u8; 32] = [0x0E; 32];
const ATTACKER_SEED: [u8; 32] = [0x66; 32];

fn kp(seed: &[u8; 32]) -> ed25519_compact::KeyPair {
    ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(seed).unwrap())
}
fn op_pub() -> OpPub {
    let mut p = [0u8; 32];
    p.copy_from_slice(kp(&OP_SEED).pk.as_ref());
    p
}
fn fp(b: u8) -> Fp {
    [b; 8]
}

/// Sign a list with `seed` and encode it; the signature entry carries `claimed_pub`.
fn signed_list(seed: &[u8; 32], claimed_pub: OpPub, net: [u8; 8], epoch: u64, fps: &[Fp]) -> Vec<u8> {
    let msg = signed_message_parts(&net, epoch, 1_700_000_000, fps);
    let sig = kp(seed).sk.sign(&msg, None);
    let mut s = [0u8; 64];
    s.copy_from_slice(sig.as_ref());
    encode_orv1(&net, epoch, 1_700_000_000, fps, &[(claimed_pub, s)])
}
fn op_list(epoch: u64, fps: &[Fp]) -> Vec<u8> {
    signed_list(&OP_SEED, op_pub(), NET, epoch, fps)
}

/// Single-operator verification (the firmware uses oasis_operator_key for k-of-n;
/// that path is tested in oasis-operator-key/tests/mesh_revocation.rs).
fn verify_single(p: &ParsedRevocation) -> bool {
    if p.sigs.len() != 1 || p.sigs[0].0 != op_pub() {
        return false;
    }
    let pk = ed25519_compact::PublicKey::from_slice(&op_pub()).unwrap();
    match ed25519_compact::Signature::from_slice(&p.sigs[0].1) {
        Ok(s) => pk.verify(signed_message(p), &s).is_ok(),
        Err(_) => false,
    }
}

/// Relay ingest: parse -> verify -> pure transition. State changes only on Applied.
fn ingest(state: &mut RevState, blob: &[u8]) -> RevDecision {
    let p = match parse_orv1(blob) {
        Ok(p) => p,
        Err(e) => return RevDecision::Reject(e),
    };
    let (d, new) = revocation_transition(state, &NET, &p, verify_single(&p));
    if let Some(n) = new {
        *state = n;
    }
    d
}

fn seed_of(id: u8) -> [u8; 32] {
    [id; 32]
}
fn registry(ids: &[u8]) -> MeshPubRegistry {
    let mut r = MeshPubRegistry::new();
    for &i in ids {
        r.insert(fp(i), mesh_v10_pubkey_from_seed(&MeshEdSeed(seed_of(i))).unwrap());
    }
    r
}
fn node(id: u8, peers: &[u8]) -> MeshRouter {
    MeshRouter::new_v0b(fp(id), NET, MeshEdSeed(seed_of(id)), registry(peers))
}

#[derive(Clone)]
struct MockSlots {
    s: [Vec<u8>; 2],
    tear_next: bool,
    cap: usize,
}
impl MockSlots {
    fn new() -> Self {
        Self::with_cap(512)
    }
    fn with_cap(cap: usize) -> Self {
        MockSlots { s: [vec![0xFF; cap], vec![0xFF; cap]], tear_next: false, cap }
    }
}
impl BlobSlots for MockSlots {
    fn cap(&self) -> usize {
        self.cap
    }
    fn read(&self, slot: usize) -> Vec<u8> {
        self.s[slot].clone()
    }
    fn write(&mut self, slot: usize, rec: &[u8]) -> bool {
        let mut x = vec![0xFF; self.cap];
        if self.tear_next {
            self.tear_next = false;
            x[..rec.len() / 2].copy_from_slice(&rec[..rec.len() / 2]); // power lost mid-program
            self.s[slot] = x;
            return false;
        }
        x[..rec.len()].copy_from_slice(rec);
        self.s[slot] = x;
        true
    }
}

#[test]
fn rev_valid_list_applied() {
    let mut st = RevState::default();
    assert_eq!(ingest(&mut st, &op_list(1, &[fp(0xCC)])), RevDecision::Applied);
    assert_eq!(st, RevState { epoch: 1, revoked: vec![fp(0xCC)] });
    assert!(st.is_revoked(&fp(0xCC)));
}

#[test]
fn rev_wrong_key_rejected_state_unchanged() {
    let mut st = RevState { epoch: 1, revoked: vec![fp(0xCC)] };
    let before = st.clone();
    // Attacker signs with its own key, honest about its pubkey.
    let mut ap = [0u8; 32];
    ap.copy_from_slice(kp(&ATTACKER_SEED).pk.as_ref());
    let forged = signed_list(&ATTACKER_SEED, ap, NET, 2, &[fp(0xAA), fp(0xCC)]);
    assert_eq!(ingest(&mut st, &forged), RevDecision::Reject(RevReject::BadOperatorSig));
    // Attacker signs with its own key but claims the operator's pubkey.
    let forged2 = signed_list(&ATTACKER_SEED, op_pub(), NET, 2, &[fp(0xAA), fp(0xCC)]);
    assert_eq!(ingest(&mut st, &forged2), RevDecision::Reject(RevReject::BadOperatorSig));
    assert_eq!(st, before, "a rejected list must not change state");
}

#[test]
fn rev_old_epoch_rejected() {
    let mut st = RevState::default();
    assert_eq!(ingest(&mut st, &op_list(5, &[fp(0xCC)])), RevDecision::Applied);
    let before = st.clone();
    assert_eq!(ingest(&mut st, &op_list(5, &[fp(0xCC)])), RevDecision::Duplicate, "same epoch: ignore, never forward");
    assert_eq!(ingest(&mut st, &op_list(4, &[fp(0xCC)])), RevDecision::Reject(RevReject::Rollback));
    assert_eq!(ingest(&mut st, &op_list(1, &[])), RevDecision::Reject(RevReject::Rollback));
    assert_eq!(st, before);
}

#[test]
fn rev_tampered_byte_rejected() {
    let blob = op_list(3, &[fp(0x11), fp(0xCC)]);
    let mut applied = 0;
    for i in 0..blob.len() {
        for b in 0..8 {
            let mut x = blob.clone();
            x[i] ^= 1 << b;
            let mut st = RevState::default();
            if ingest(&mut st, &x) == RevDecision::Applied {
                applied += 1;
            }
            assert_eq!(st, RevState::default(), "byte {} bit {} changed state", i, b);
        }
    }
    assert_eq!(applied, 0, "every single-bit tampering must be rejected");
}

#[test]
fn rev_then_revoked_origin_dropped_before_signature() {
    // Relay B knows A (honest) and C (to be revoked).
    let mut b = node(0xBB, &[0xAA, 0xCC]);
    let mut a = node(0xAA, &[]);
    let mut c = node(0xCC, &[]);
    let mut st = RevState::default();
    assert_eq!(ingest(&mut st, &op_list(1, &[fp(0xCC)])), RevDecision::Applied);
    for f in &st.revoked {
        b.revoke(*f);
    }
    let from_c = c.origin_wrap_v0b(b"after revocation").unwrap();
    assert!(matches!(b.process(&from_c), MeshDecision::Drop("origin revoked")));
    let from_a = a.origin_wrap_v0b(b"honest").unwrap();
    assert!(matches!(b.process(&from_a), MeshDecision::Arrived { .. }), "revocation must not hit others");
}

#[test]
fn rev_persisted_across_reboot() {
    let mut slots = MockSlots::new();
    let blob = op_list(7, &[fp(0xCC)]);
    let mut st = RevState::default();
    assert_eq!(ingest(&mut st, &blob), RevDecision::Applied);
    assert!(store_blob(&mut slots, &blob), "persist BEFORE applying/forwarding");
    // Reboot: RAM lost. Restore from flash, re-verify, re-apply before any traffic.
    let mut st2 = RevState::default();
    let stored = load_latest_blob(&slots).expect("persisted list");
    assert_eq!(ingest(&mut st2, &stored), RevDecision::Applied);
    assert_eq!(st2.epoch, 7);
    let mut b = node(0xBB, &[0xCC]);
    for f in &st2.revoked {
        b.revoke(*f);
    }
    let from_c = node(0xCC, &[]).origin_wrap_v0b(b"x").unwrap();
    assert!(matches!(b.process(&from_c), MeshDecision::Drop("origin revoked")));
}

#[test]
fn rev_torn_blob_write_keeps_previous_list() {
    let mut slots = MockSlots::new();
    let e1 = op_list(1, &[fp(0xCC)]);
    assert!(store_blob(&mut slots, &e1));
    slots.tear_next = true;
    assert!(!store_blob(&mut slots, &op_list(2, &[fp(0xAA), fp(0xCC)])), "torn write is not durable");
    assert_eq!(load_latest_blob(&slots).as_deref(), Some(&e1[..]), "previous list survives the tear");
}

/// An OAU1 revocation blob as persisted (signature bytes are irrelevant to the
/// store: the loader's caller re-verifies).
fn oau1_rev_blob(epoch: u64, fps: &[Fp], suite: u8) -> Vec<u8> {
    use crate::authority::{encode_oau1, kind, MLDSA44_SIG_LEN, SUITE_HYBRID};
    let body = encode_revocation_body(epoch, 0, fps);
    let ml = if suite == SUITE_HYBRID { vec![0x5A; MLDSA44_SIG_LEN] } else { Vec::new() };
    encode_oau1(kind::REVOCATION, suite, &NET, &body, &[0x11; 64], &ml)
}

#[test]
fn rev_blob_store_orders_orv1_and_oau1_by_epoch() {
    use crate::authority::{SUITE_ED25519, SUITE_HYBRID};
    // Device slot size (one 4 KiB sector): a hybrid OAU1 revocation (~2.5 KB) fits.
    let mut slots = MockSlots::with_cap(4096);
    let e1 = op_list(1, &[fp(0xCC)]);
    assert!(store_blob(&mut slots, &e1));
    let e2 = oau1_rev_blob(2, &[fp(0xBB), fp(0xCC)], SUITE_HYBRID);
    assert_eq!(blob_epoch(&e2), Some(2));
    assert!(store_blob(&mut slots, &e2), "hybrid OAU1 revocation persisted");
    assert_eq!(load_latest_blob(&slots).as_deref(), Some(&e2[..]), "newest epoch wins across formats");
    // A torn write of epoch 3 keeps the hybrid epoch-2 list loadable.
    slots.tear_next = true;
    assert!(!store_blob(&mut slots, &oau1_rev_blob(3, &[fp(0xAA)], SUITE_ED25519)));
    assert_eq!(load_latest_blob(&slots).as_deref(), Some(&e2[..]));
    // The epoch-1 ORV1 slot was the one overwritten, never the newest.
    assert!(store_blob(&mut slots, &oau1_rev_blob(3, &[fp(0xAA), fp(0xBB), fp(0xCC)], SUITE_ED25519)));
    assert_eq!(load_latest_blob(&slots).map(|b| blob_epoch(&b)), Some(Some(3)));
    // Non-revocation OAU1 kinds and garbage have no epoch.
    let mut policy = e2.clone();
    policy[4] = crate::authority::kind::POLICY;
    assert_eq!(blob_epoch(&policy), None);
    assert_eq!(blob_epoch(b"OAU1garbage"), None);
    // A hybrid blob does not fit the old 512-byte ORV1 slot: the store refuses it.
    assert!(!store_blob(&mut MockSlots::new(), &e2));
}

#[test]
fn rev_disconnected_relay_catches_up() {
    // B got epoch 1 while C was offline.
    let mut b = node(0xBB, &[0xAA, 0xCC, 0xDD]);
    let mut c = node(0xCC, &[0xAA, 0xBB, 0xDD]);
    let mut b_state = RevState::default();
    let blob = op_list(1, &[fp(0xDD)]);
    assert_eq!(ingest(&mut b_state, &blob), RevDecision::Applied);
    let mut c_state = RevState::default();
    // C reconnects and announces its epoch to B.
    let beacon = c.origin_wrap_v0b_with_ttl(&encode_oep1(c_state.epoch), 0).unwrap();
    let heard = match b.process(&beacon) {
        MeshDecision::Arrived { envelope, .. } => parse_oep1(inner_slice(&envelope)).unwrap(),
        _ => panic!("beacon must arrive"),
    };
    assert!(should_serve_catchup(b_state.epoch, heard));
    // B re-originates the stored operator-signed list from itself.
    let reserve = b.origin_wrap_v0b(&blob).unwrap();
    match c.process(&reserve) {
        MeshDecision::Arrived { envelope, .. } => {
            assert_eq!(ingest(&mut c_state, inner_slice(&envelope)), RevDecision::Applied)
        }
        _ => panic!("re-served list must arrive"),
    }
    for f in &c_state.revoked {
        c.revoke(*f);
    }
    let from_d = node(0xDD, &[]).origin_wrap_v0b(b"still talking").unwrap();
    assert!(matches!(c.process(&from_d), MeshDecision::Drop("origin revoked")), "C caught up and now rejects D");
    assert!(!should_serve_catchup(c_state.epoch, b_state.epoch), "no ping-pong once equal");
}

#[test]
fn rev_counter_revocation_by_revoked_node_rejected() {
    let mut b = node(0xBB, &[0xAA, 0xCC]);
    let mut st = RevState::default();
    assert_eq!(ingest(&mut st, &op_list(1, &[fp(0xCC)])), RevDecision::Applied);
    for f in &st.revoked {
        b.revoke(*f);
    }
    // C forges a "counter-revocation" (epoch 2, empty would shrink; it tries revoking A).
    let counter = signed_list(&seed_of(0xCC), op_pub(), NET, 2, &[fp(0xAA), fp(0xCC)]);
    let mut c = node(0xCC, &[]);
    let env = c.origin_wrap_v0b(&counter).unwrap();
    assert!(matches!(b.process(&env), MeshDecision::Drop("origin revoked")), "dropped before even reading content");
    // Even if an honest node carries it, the operator signature fails.
    let before = st.clone();
    assert_eq!(ingest(&mut st, &counter), RevDecision::Reject(RevReject::BadOperatorSig));
    assert_eq!(st, before);
}

#[test]
fn rev_shrinking_list_rejected() {
    let mut st = RevState::default();
    assert_eq!(ingest(&mut st, &op_list(1, &[fp(0xAA), fp(0xCC)])), RevDecision::Applied);
    let before = st.clone();
    assert_eq!(ingest(&mut st, &op_list(2, &[fp(0xCC)])), RevDecision::Reject(RevReject::Shrink));
    assert_eq!(st, before);
    assert_eq!(ingest(&mut st, &op_list(2, &[fp(0xAA), fp(0xBB), fp(0xCC)])), RevDecision::Applied, "growth is fine");
}

#[test]
fn rev_wrong_network_rejected() {
    let mut st = RevState::default();
    let other = signed_list(&OP_SEED, op_pub(), *b"OTHERnet", 1, &[fp(0xCC)]);
    assert_eq!(ingest(&mut st, &other), RevDecision::Reject(RevReject::WrongNetwork));
    assert_eq!(st, RevState::default());
}

#[test]
fn rev_parse_rejects_noncanonical() {
    let ok = op_list(1, &[fp(1), fp(2)]);
    assert!(parse_orv1(&ok).is_ok());
    // unsorted / duplicate
    let msg = |f: &[Fp]| encode_orv1(&NET, 1, 0, f, &[([0; 32], [0; 64])]);
    assert_eq!(parse_orv1(&msg(&[fp(2), fp(1)])), Err(RevReject::Malformed));
    assert_eq!(parse_orv1(&msg(&[fp(2), fp(2)])), Err(RevReject::Malformed));
    // trailing byte, truncation, zero signatures, too many entries
    let mut t = ok.clone();
    t.push(0);
    assert_eq!(parse_orv1(&t), Err(RevReject::Malformed));
    assert_eq!(parse_orv1(&ok[..ok.len() - 1]), Err(RevReject::Malformed));
    assert_eq!(parse_orv1(&encode_orv1(&NET, 1, 0, &[fp(1)], &[])), Err(RevReject::Malformed));
    let many: Vec<Fp> = (0..=MAX_REVOKED as u8).map(fp).collect();
    assert_eq!(parse_orv1(&msg(&many)), Err(RevReject::Malformed));
    for n in 0..ok.len() {
        let _ = parse_orv1(&ok[..n]); // never panics on any prefix
    }
}

#[test]
fn rev_oep1_roundtrip() {
    assert_eq!(parse_oep1(&encode_oep1(42)), Some(42));
    assert_eq!(parse_oep1(b"OEP1"), None);
    assert!(should_serve_catchup(3, 1) && !should_serve_catchup(1, 1) && !should_serve_catchup(1, 3));
}
