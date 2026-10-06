use super::*;

const NET: [u8; 8] = *b"OASISnet";

/// Deterministic, balanced, run-free test stream (xorshift), packed LSB first.
fn good_bits(nbits: usize, mut x: u32) -> Vec<u8> {
    (0..nbits.div_ceil(8))
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

#[test]
fn id_health_accepts_balanced_and_refuses_stuck_or_biased() {
    let g = good_bits(RAW_SAMPLES, 0x1234_5678);
    assert!(health_ok(&g, RAW_SAMPLES));
    // Stuck at 0 or 1: the repetition count test fires.
    assert!(!rct_ok(&[0x00; RAW_SAMPLES / 8], RAW_SAMPLES));
    assert!(!rct_ok(&[0xFF; RAW_SAMPLES / 8], RAW_SAMPLES));
    // A run of exactly RCT_CUTOFF - 1 identical bits passes, RCT_CUTOFF fails.
    let with_run = |len: usize| {
        let mut v: Vec<bool> = (0..64).map(|i| i % 2 == 0).collect(); // alternating
        v.extend(core::iter::repeat(false).take(len)); // last alternating bit is `false`
        v.extend((0..64).map(|i| i % 2 == 0));
        let n = v.len();
        let mut b = vec![0u8; n.div_ceil(8)];
        for (i, &x) in v.iter().enumerate() {
            b[i / 8] |= (x as u8) << (i % 8);
        }
        (b, n)
    };
    // The alternating prefix ends on `false`, so the run is len + 1 long.
    let (b, n) = with_run(RCT_CUTOFF - 2);
    assert!(rct_ok(&b, n), "run of {} must pass", RCT_CUTOFF - 1);
    let (b, n) = with_run(RCT_CUTOFF - 1);
    assert!(!rct_ok(&b, n), "run of {} must fail", RCT_CUTOFF);
    // Biased (one 0 per 16 bits, runs of 15): only the proportion test catches it.
    // SP 800-90B's APT counts the window's FIRST sample value, so a window that
    // starts on the rare value is not caught; here every window starts on a 1.
    let biased: Vec<u8> = (0..RAW_SAMPLES / 8).map(|i| if i % 2 == 0 { 0xFF } else { 0xFE }).collect();
    assert!(rct_ok(&biased, RAW_SAMPLES) && !apt_ok(&biased, RAW_SAMPLES));
    let rare_first: Vec<u8> = (0..RAW_SAMPLES / 8).map(|i| if i % 2 == 0 { 0xFE } else { 0xFF }).collect();
    assert!(apt_ok(&rare_first, RAW_SAMPLES), "documented APT blind spot, as specified");
    // Malformed lengths never pass.
    assert!(!health_ok(&g, 0) && !health_ok(&g, RAW_SAMPLES + 1) && !apt_ok(&g, APT_WINDOW - 1));
}

#[test]
fn id_conditioning_and_rekey_are_deterministic_and_input_bound() {
    let raw = good_bits(RAW_SAMPLES, 7);
    let s = condition_seed(&raw, 42, &[1; 8]);
    assert_eq!(s, condition_seed(&raw, 42, &[1; 8]));
    assert_ne!(s, condition_seed(&raw, 43, &[1; 8]));
    assert_ne!(s, condition_seed(&raw, 42, &[2; 8]));
    let mut raw2 = raw.clone();
    raw2[100] ^= 1;
    assert_ne!(s, condition_seed(&raw2, 42, &[1; 8]));
    let k = rekey(&s, &[9; 32], &raw);
    assert_ne!(k, s);
    assert_ne!(k, rekey(&s, &[8; 32], &raw), "tool nonce is mixed in");
    let mut s2 = s;
    s2[0] ^= 1;
    assert_ne!(k, rekey(&s2, &[9; 32], &raw), "old secret seed is mixed in");
}

#[test]
fn id_proof_of_possession() {
    let seed = [0x42; 32];
    let pk = public_key(&seed);
    let ch = [0x5C; 32];
    let sig = pop_sign(&seed, &NET, &ch);
    assert!(pop_verify(&pk, &NET, &ch, &sig));
    assert!(!pop_verify(&pk, b"OTHERnet", &ch, &sig), "wrong network");
    assert!(!pop_verify(&pk, &NET, &[0x5D; 32], &sig), "wrong challenge");
    assert!(!pop_verify(&public_key(&[0x43; 32]), &NET, &ch, &sig), "another key");
    // Same key as the mesh: the v0B router derives the same public key.
    let mesh_pk = crate::mesh::mesh_v10_pubkey_from_seed(&crate::mesh::MeshEdSeed(seed)).unwrap();
    assert_eq!(mesh_pk.0, pk);
    assert_eq!(fingerprint(&pk), crate::spore_crypto::sender_fingerprint(&pk));
}

#[test]
fn id_record_roundtrip_and_corruption() {
    let r = encode_identity(&[7; 32], true);
    assert_eq!(decode_identity(&r), Some(([7; 32], true)));
    assert_eq!(decode_identity(&encode_identity(&[7; 32], false)), Some(([7; 32], false)));
    for i in 0..ID_RECORD_LEN {
        let mut c = r;
        c[i] ^= 0x01;
        assert_eq!(decode_identity(&c), None, "corrupted byte {}", i);
    }
    assert_eq!(decode_identity(&[0xFF; ID_RECORD_LEN]), None, "erased flash");
    assert_eq!(decode_identity(&r[..ID_RECORD_LEN - 1]), None);
    // An all-zero seed (would panic in ed25519-compact) is refused even if well-formed.
    assert!(!seed_usable(&[0; 32]) && seed_usable(&[0, 0, 1].repeat(11)[..32].try_into().unwrap()));
    assert_eq!(decode_identity(&encode_identity(&[0; 32], false)), None);
}

#[test]
fn id_entropy_statistics() {
    let s = bit_stats(&[0xFF; 16], 128);
    assert_eq!((s.n, s.ones, s.longest_run), (128, 128, 128));
    assert_eq!(mcv_min_entropy_milli(&s), 0, "constant source: no entropy");
    let alt = bit_stats(&[0xAA; 16], 128);
    assert_eq!((alt.ones, alt.longest_run), (64, 1));
    // Balanced 100 000-bit stream: MCV estimate close to (but below) 1 bit/sample.
    let g = good_bits(100_000, 99);
    let st = bit_stats(&g, 100_000);
    let h = mcv_min_entropy_milli(&st);
    // The 99 % upper bound alone costs ~12 milli-bits at n = 100 000.
    assert!((970..1000).contains(&h), "h = {} milli-bits", h);
}
