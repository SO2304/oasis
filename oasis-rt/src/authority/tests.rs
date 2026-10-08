use super::*;
use crate::mesh_revocation::{encode_revocation_body, parse_revocation_body, revocation_transition, RevDecision, RevState};
use ml_dsa::{EncodedVerifyingKey, Keypair, MlDsa44, Signature as RcSignature, SigningKey as MlDsaSigningKey, VerifyingKey as RcVerifyingKey, B32};

const NET: [u8; 8] = *b"OASISnet";
const ED_SEED: [u8; 32] = [0x0E; 32];
const ML_SEED: [u8; 32] = [0x5A; 32];

fn ed_kp(seed: &[u8; 32]) -> ed25519_compact::KeyPair {
    ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(seed).unwrap())
}
fn ed_pub(seed: &[u8; 32]) -> [u8; 32] {
    let mut p = [0u8; 32];
    p.copy_from_slice(ed_kp(seed).pk.as_ref());
    p
}
fn ml_sk(seed: &[u8; 32]) -> MlDsaSigningKey<MlDsa44> {
    MlDsaSigningKey::<MlDsa44>::from_seed(&B32::from(*seed))
}
fn ml_pub(seed: &[u8; 32]) -> [u8; MLDSA44_PK_LEN] {
    let mut p = [0u8; MLDSA44_PK_LEN];
    p.copy_from_slice(&ml_sk(seed).verifying_key().encode());
    p
}
fn ml_sign(seed: &[u8; 32], msg: &[u8], ctx: &[u8]) -> Vec<u8> {
    ml_sk(seed).expanded_key().sign_deterministic(msg, ctx).unwrap().encode().to_vec()
}

/// Operator-side signing of an OAU1 message (on the PC in production).
fn signed_oau1(kind: u8, suite: u8, content: &[u8], ed_seed: &[u8; 32], ml_seed: &[u8; 32]) -> Vec<u8> {
    let msg = signed_message(kind, suite, &NET, content);
    let mut ed = [0u8; 64];
    ed.copy_from_slice(ed_kp(ed_seed).sk.sign(&msg, None).as_ref());
    let ml = if suite == SUITE_HYBRID { ml_sign(ml_seed, &msg, AUTH_DOMAIN) } else { Vec::new() };
    encode_oau1(kind, suite, &NET, content, &ed, &ml)
}

fn keys(mlpk: &[u8; MLDSA44_PK_LEN]) -> AuthorityKeys<'_> {
    AuthorityKeys { ed25519: ed_pub(&ED_SEED), mldsa44: mlpk }
}

/// The RustCrypto `ml-dsa` verifier: the independent oracle for `mldsa44_verify`
/// (which is libcrux).
fn rustcrypto_verify(pk: &[u8], msg: &[u8], ctx: &[u8], sig: &[u8]) -> bool {
    let enc = match EncodedVerifyingKey::<MlDsa44>::try_from(pk) {
        Ok(e) => e,
        Err(_) => return false,
    };
    match RcSignature::<MlDsa44>::try_from(sig) {
        Ok(s) => RcVerifyingKey::<MlDsa44>::decode(&enc).verify_with_context(msg, ctx, &s),
        Err(_) => false,
    }
}

fn hex(s: &str) -> Vec<u8> {
    if s == "-" {
        return Vec::new();
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

// ─── ML-DSA-44 correctness ───

#[test]
fn pq_acvp_mldsa44_sigver_official_vectors() {
    // NIST ACVP-Server, ML-DSA-44 external/pure sigVer: 3 valid + 12 tampered.
    // Checked against the device verifier (libcrux) AND the oracle (RustCrypto).
    let data = include_str!("acvp_mldsa44_sigver.txt");
    let (mut n, mut ok) = (0, 0);
    for line in data.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = line.split(' ').collect();
        let (tc, passed) = (f[0], f[1] == "1");
        let (pk, msg, ctx, sig) = (hex(f[2]), hex(f[3]), hex(f[4]), hex(f[5]));
        let got = mldsa44_verify(&pk, &msg, &ctx, &sig);
        assert_eq!(got, passed, "libcrux: ACVP tcId {} ({}) expected passed={}", tc, f[6], passed);
        let oracle = rustcrypto_verify(&pk, &msg, &ctx, &sig);
        assert_eq!(oracle, passed, "ml-dsa: ACVP tcId {} ({}) expected passed={}", tc, f[6], passed);
        n += 1;
        ok += passed as usize;
    }
    assert_eq!((n, ok), (15, 3), "all 15 official cases checked");
}

#[test]
fn pq_mldsa44_matches_libcrux_byte_for_byte() {
    // Two independent implementations (libcrux: formally verified arithmetic/NTT).
    // FIPS 204 KeyGen_internal(seed) and deterministic signing (rnd = 0) are
    // deterministic, so keys and signatures must be identical byte for byte.
    use libcrux_ml_dsa::ml_dsa_44 as lx;
    for i in 0..8u8 {
        let seed = [i.wrapping_mul(37).wrapping_add(1); 32];
        let lx_kp = lx::generate_key_pair(seed);
        assert_eq!(&ml_pub(&seed)[..], lx_kp.verification_key.as_slice(), "public key, seed {}", i);
        let msg: Vec<u8> = (0..(50 + 13 * i as usize)).map(|j| (j as u8) ^ i).collect();
        let ours = ml_sign(&seed, &msg, AUTH_DOMAIN);
        let theirs = lx::sign(&lx_kp.signing_key, &msg, AUTH_DOMAIN, [0u8; 32]).unwrap();
        assert_eq!(&ours[..], theirs.as_slice(), "deterministic signature, seed {}", i);
        // Cross-verify both ways, plus a tampered signature rejected by both.
        let mut sig_arr = [0u8; MLDSA44_SIG_LEN];
        sig_arr.copy_from_slice(&ours);
        // mldsa44_verify (libcrux) on the ml-dsa signature, ml-dsa on libcrux's.
        assert!(mldsa44_verify(lx_kp.verification_key.as_slice(), &msg, AUTH_DOMAIN, &sig_arr));
        assert!(rustcrypto_verify(&ml_pub(&seed), &msg, AUTH_DOMAIN, theirs.as_slice()));
        sig_arr[100] ^= 0x01;
        assert!(!mldsa44_verify(lx_kp.verification_key.as_slice(), &msg, AUTH_DOMAIN, &sig_arr));
        assert!(!rustcrypto_verify(&ml_pub(&seed), &msg, AUTH_DOMAIN, &sig_arr));
    }
}

// ─── container and hybrid rule ───

#[test]
fn pq_hybrid_valid_accepted() {
    let mlpk = ml_pub(&ML_SEED);
    let m = signed_oau1(kind::REVOCATION, SUITE_HYBRID, b"content", &ED_SEED, &ML_SEED);
    assert_eq!(m.len(), 16 + 7 + 64 + 2420);
    let p = verify_authority(&AuthPolicy::default(), &NET, &keys(&mlpk), &m).unwrap();
    assert_eq!((p.kind, p.suite, p.content), (kind::REVOCATION, SUITE_HYBRID, &b"content"[..]));
}

#[test]
fn pq_hybrid_requires_both_signatures() {
    let mlpk = ml_pub(&ML_SEED);
    let pol = AuthPolicy::default();
    // ML-DSA signed by another key, Ed25519 valid: refused.
    let bad_ml = signed_oau1(kind::REVOCATION, SUITE_HYBRID, b"x", &ED_SEED, &[0x77; 32]);
    assert_eq!(verify_authority(&pol, &NET, &keys(&mlpk), &bad_ml), Err(AuthReject::BadSignature));
    // Ed25519 signed by another key, ML-DSA valid: refused.
    let bad_ed = signed_oau1(kind::REVOCATION, SUITE_HYBRID, b"x", &[0x66; 32], &ML_SEED);
    assert_eq!(verify_authority(&pol, &NET, &keys(&mlpk), &bad_ed), Err(AuthReject::BadSignature));
}

#[test]
fn pq_unknown_and_reserved_suites_refused() {
    let mut m = signed_oau1(kind::REVOCATION, SUITE_ED25519, b"x", &ED_SEED, &ML_SEED);
    for s in [0x00u8, SUITE_MLDSA44, 0x04, 0xFF] {
        m[5] = s;
        assert_eq!(parse_oau1(&m).err(), Some(AuthReject::UnsupportedSuite), "suite {:#04x}", s);
    }
}

#[test]
fn pq_downgrade_refused_before_signature_check() {
    let mlpk = ml_pub(&ML_SEED);
    let mut pol = AuthPolicy::default();
    assert!(pol.raise(kind::REVOCATION, SUITE_HYBRID));
    // A perfectly valid Ed25519-only message is now a downgrade...
    let ed_only = signed_oau1(kind::REVOCATION, SUITE_ED25519, b"x", &ED_SEED, &ML_SEED);
    assert_eq!(verify_authority(&pol, &NET, &keys(&mlpk), &ed_only), Err(AuthReject::Downgrade));
    // ...refused even with garbage signatures (no expensive verification attempted).
    let mut garbage = ed_only.clone();
    let n = garbage.len();
    garbage[n - 1] ^= 0xFF;
    assert_eq!(verify_authority(&pol, &NET, &keys(&mlpk), &garbage), Err(AuthReject::Downgrade));
    assert!(!pol.legacy_orv1_allowed(), "legacy ORV1 lists are refused once revocation requires hybrid");
}

#[test]
fn pq_suite_relabel_in_transit_refused() {
    // Strip the ML-DSA signature and relabel the suite as Ed25519: the Ed25519
    // signature covered suite = hybrid, so it no longer verifies.
    let mlpk = ml_pub(&ML_SEED);
    let h = signed_oau1(kind::ENROLLMENT, SUITE_HYBRID, b"abc", &ED_SEED, &ML_SEED);
    let mut relabelled = h[..h.len() - MLDSA44_SIG_LEN].to_vec();
    relabelled[5] = SUITE_ED25519;
    assert_eq!(verify_authority(&AuthPolicy::default(), &NET, &keys(&mlpk), &relabelled), Err(AuthReject::BadSignature));
}

#[test]
fn pq_wrong_network_refused() {
    let mlpk = ml_pub(&ML_SEED);
    let m = signed_oau1(kind::REVOCATION, SUITE_HYBRID, b"x", &ED_SEED, &ML_SEED);
    assert_eq!(verify_authority(&AuthPolicy::default(), b"OTHERnet", &keys(&mlpk), &m), Err(AuthReject::WrongNetwork));
}

#[test]
fn pq_every_single_bit_flip_refused() {
    // Exhaustive over every bit of a hybrid message: 16 + 9 + 64 + 2420 = 2 509
    // bytes, 20 072 flips.
    let mlpk = ml_pub(&ML_SEED);
    let pol = AuthPolicy::default();
    let m = signed_oau1(kind::REVOCATION, SUITE_HYBRID, b"revoke-me", &ED_SEED, &ML_SEED);
    assert_eq!(m.len(), 2509);
    let (mut tried, mut accepted) = (0, 0);
    for i in 0..m.len() {
        for b in 0..8 {
            let mut x = m.clone();
            x[i] ^= 1 << b;
            tried += 1;
            if verify_authority(&pol, &NET, &keys(&mlpk), &x).is_ok() {
                accepted += 1;
            }
        }
    }
    assert_eq!((tried, accepted), (20_072, 0), "every single-bit flip must be refused");
}

#[test]
fn pq_precheck_refuses_unknown_kind_and_suite() {
    // Not reachable through verify_authority (parse_oau1 rejects first), but the
    // precheck must stand on its own: Kani found it accepted kind 0 + suite 0.
    let pol = AuthPolicy::default();
    for (k, s) in [(0u8, 0u8), (0, SUITE_HYBRID), (6, SUITE_HYBRID), (0xFF, 0x77)] {
        let p = ParsedAuthority { kind: k, suite: s, network_id: NET, content: &[], ed25519_sig: &[], mldsa44_sig: &[] };
        assert_eq!(precheck(&pol, &NET, &p), Err(AuthReject::UnknownKind), "kind {} suite {}", k, s);
    }
    let p = ParsedAuthority { kind: kind::ENROLLMENT, suite: 0x77, network_id: NET, content: &[], ed25519_sig: &[], mldsa44_sig: &[] };
    assert_eq!(precheck(&pol, &NET, &p), Err(AuthReject::Downgrade), "unknown suite has no rank");
}

#[test]
fn pq_parse_never_panics_on_prefixes_and_garbage() {
    let m = signed_oau1(kind::REVOCATION, SUITE_HYBRID, b"x", &ED_SEED, &ML_SEED);
    for n in 0..m.len() {
        let _ = parse_oau1(&m[..n]);
    }
    let mut x = 0x1234_5678u32;
    for _ in 0..2000 {
        let mut b = vec![0u8; (x % 64) as usize];
        for v in b.iter_mut() {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *v = x as u8;
        }
        let _ = parse_oau1(&b);
    }
}

// ─── policy ───

#[test]
fn pq_policy_monotone_and_persisted() {
    let mut p = AuthPolicy::default();
    assert_eq!(p.min_suite(kind::POLICY), SUITE_HYBRID, "policy updates always need hybrid");
    assert!(p.raise(kind::ENROLLMENT, SUITE_HYBRID));
    assert!(!p.raise(kind::ENROLLMENT, SUITE_ED25519), "never lowered");
    assert!(!p.raise(kind::ENROLLMENT, SUITE_MLDSA44), "reserved suite never accepted");
    assert!(!p.raise(9, SUITE_HYBRID), "unknown kind");
    let r = p.to_bytes();
    assert_eq!(AuthPolicy::from_bytes(&r), Some(p));
    for i in 0..r.len() {
        let mut c = r;
        c[i] ^= 0x01;
        assert_ne!(AuthPolicy::from_bytes(&c), Some(p), "corrupted byte {} accepted as-is", i);
    }
    // A record can never restore a policy weaker than the default.
    let mut weak = AuthPolicy::default().to_bytes();
    weak[4 + (kind::POLICY - 1) as usize] = SUITE_ED25519;
    assert!(AuthPolicy::from_bytes(&weak).map_or(true, |q| q.min_suite(kind::POLICY) == SUITE_HYBRID));
}

#[test]
fn pq_policy_two_slots_merge_to_the_strongest() {
    let mut raised = AuthPolicy::default();
    assert!(raised.raise(kind::REVOCATION, SUITE_HYBRID));
    let (old, new) = (AuthPolicy::default().to_bytes(), raised.to_bytes());
    let blank = [0xFFu8; POLICY_RECORD_LEN];
    // Both written: the new policy.
    assert_eq!(AuthPolicy::from_slots(&new, &new), raised);
    // Power cut while writing slot 0 (torn) after the raise: slot 1 still old, so
    // the device restarts on the old policy; while writing slot 1: slot 0 is new.
    let mut torn = new;
    torn[6] ^= 0xFF;
    assert_eq!(AuthPolicy::from_slots(&torn, &old), AuthPolicy::default());
    assert_eq!(AuthPolicy::from_slots(&new, &torn), raised);
    // Order never matters; erased flash and garbage fall back to the default,
    // which is never weaker than the default.
    assert_eq!(AuthPolicy::from_slots(&old, &new), raised);
    assert_eq!(AuthPolicy::from_slots(&new, &old), raised);
    assert_eq!(AuthPolicy::from_slots(&blank, &blank), AuthPolicy::default());
    assert_eq!(AuthPolicy::from_slots(&[], b"junk"), AuthPolicy::default());

    // A record with the right magic but another length is refused, and the fallback is
    // the *default* — so widening KIND_COUNT silently drops every raise a fleet made.
    // `POLICY_RECORD_LEN = 4 + KIND_COUNT + 4`, so this is what a build with one more
    // kind would write, and what this one makes of it. See `docs/CRYPTO_MIGRATION.md`
    // §6: the record must gain a version before the suite table gains a slot.
    let mut wider = [0u8; POLICY_RECORD_LEN + 1];
    wider[..POLICY_RECORD_LEN].copy_from_slice(&new);
    assert_eq!(AuthPolicy::from_bytes(&wider), None, "a longer record is unreadable");
    assert_eq!(AuthPolicy::from_slots(&wider, &wider), AuthPolicy::default());
    assert_ne!(AuthPolicy::from_slots(&wider, &wider), raised, "the raise is lost");
}

#[test]
fn pq_policy_update_message_is_hybrid_only() {
    let mlpk = ml_pub(&ML_SEED);
    let mut pol = AuthPolicy::default();
    let ed_only = signed_oau1(kind::POLICY, SUITE_ED25519, &[kind::REVOCATION, SUITE_HYBRID], &ED_SEED, &ML_SEED);
    assert_eq!(verify_authority(&pol, &NET, &keys(&mlpk), &ed_only), Err(AuthReject::Downgrade));
    let hybrid = signed_oau1(kind::POLICY, SUITE_HYBRID, &[kind::REVOCATION, SUITE_HYBRID], &ED_SEED, &ML_SEED);
    let p = verify_authority(&pol, &NET, &keys(&mlpk), &hybrid).unwrap();
    let (k, s) = parse_policy_update(p.content).unwrap();
    assert!(pol.raise(k, s));
    assert!(!pol.legacy_orv1_allowed());
}

// ─── revocation carried in OAU1 ───

#[test]
fn pq_hybrid_revocation_applies_through_existing_rules() {
    let mlpk = ml_pub(&ML_SEED);
    let body = encode_revocation_body(4, 1_790_000_004, &[[0xBB; 8], [0xDD; 8]]);
    let m = signed_oau1(kind::REVOCATION, SUITE_HYBRID, &body, &ED_SEED, &ML_SEED);
    let p = verify_authority(&AuthPolicy::default(), &NET, &keys(&mlpk), &m).unwrap();
    let rev = parse_revocation_body(p.network_id, p.content).unwrap();
    let (d, new) = revocation_transition(&RevState::default(), &NET, &rev, true);
    assert_eq!(d, RevDecision::Applied);
    assert_eq!(new.unwrap().revoked, vec![[0xBB; 8], [0xDD; 8]]);
    // Non-canonical bodies keep failing exactly like ORV1.
    let unsorted = encode_revocation_body(5, 0, &[[0xDD; 8], [0xBB; 8]]);
    assert!(parse_revocation_body(NET, &unsorted).is_err());
}

#[test]
fn pq_sizes_match_spec() {
    let m = signed_oau1(kind::REVOCATION, SUITE_HYBRID, &encode_revocation_body(1, 0, &[[0xBB; 8]]), &ED_SEED, &ML_SEED);
    // 16 header + 26 body + 64 + 2420 = 2526 bytes: needs fragmentation (spec §3).
    assert_eq!(m.len(), 2526);
    assert_eq!(ml_sk(&ML_SEED).verifying_key().encode().len(), MLDSA44_PK_LEN);
}
