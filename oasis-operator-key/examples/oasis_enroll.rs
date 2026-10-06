//! Minimal PC enrollment tool (Phase 1.2, docs/specs/ENROLLMENT_OWNERSHIP_SPEC.md §5).
//! Owner keys (Ed25519 + ML-DSA-44) stay HERE; boards only ever see public keys and
//! signed messages. Test owners: o1 = owner #1 (same seeds as ef_payloads/pq_payloads),
//! o2 = the new owner, o3 = an attacker.
//!
//!   oasis_enroll challenge                           -> "<challenge_hex> <nonce_hex>" (OS RNG)
//!   oasis_enroll verify-pop <pk> <challenge> <sig>   -> "POP_OK fp=<hex>" (exit 0) or POP_FAIL (exit 1)
//!   oasis_enroll fp <pk>                             -> fingerprint
//!   oasis_enroll attest <owner> <pk> <role> <perms> <seq> [ed|hybrid]   -> OAU1 kind 2 (hex)
//!   oasis_enroll revoke <owner> <epoch> <fp>..       -> hybrid OAU1 revocation (hex)
//!   oasis_enroll offer <old> <new> <seq>             -> ownership offer, signed by <old> (hex)
//!   oasis_enroll accept <new> <seq> <offer_hex_file> -> acceptance, signed by <new> (hex)
//!   oasis_enroll fw-manifest <owner> <version> <image.bin> [hw_id] -> firmware manifest (OAU1 kind 3, hybrid)

use ml_dsa::{Keypair, MlDsa44, SigningKey, B32};
use oasis_operator_key::sign_with_seed;
use oasis_rt::authority::{
    encode_oau1, kind, signed_message, AUTH_DOMAIN, SUITE_ED25519, SUITE_HYBRID,
};
use oasis_rt::enrollment::{encode_attestation, Attestation};
use oasis_rt::firmware::{encode_manifest, image_version, Manifest, HW_ID};
use oasis_rt::identity::{fingerprint, pop_verify};
use oasis_rt::mesh_revocation::{encode_revocation_body, Fp};
use oasis_rt::ownership::{encode_accept, encode_offer, offer_digest, OwnerKeys};
use sha2::{Digest, Sha256};

const NETWORK_ID: [u8; 8] = *b"OASISnet";

struct Owner {
    ed_seed: [u8; 32],
    ml_seed: [u8; 32],
}

fn owner(name: &str) -> Owner {
    let (e, m) = match name {
        "o1" => (0x0E, 0x0F),
        "o2" => (0x2E, 0x2F),
        "o3" => (0x3E, 0x3F),
        _ => panic!("owner must be o1, o2 or o3"),
    };
    Owner {
        ed_seed: [e; 32],
        ml_seed: [m; 32],
    }
}

impl Owner {
    fn ed_pub(&self) -> [u8; 32] {
        let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(self.ed_seed));
        let mut p = [0u8; 32];
        p.copy_from_slice(kp.pk.as_ref());
        p
    }
    fn ml_sk(&self) -> SigningKey<MlDsa44> {
        SigningKey::<MlDsa44>::from_seed(&B32::from(self.ml_seed))
    }
    fn keys(&self) -> OwnerKeys {
        OwnerKeys::new(self.ed_pub(), &self.ml_sk().verifying_key().encode()).unwrap()
    }
    /// OAU1 message of `k` under `suite`, signed by this owner (deterministic ML-DSA).
    fn sign(&self, k: u8, suite: u8, content: &[u8]) -> Vec<u8> {
        let msg = signed_message(k, suite, &NETWORK_ID, content);
        let ed = sign_with_seed(&self.ed_seed, &msg);
        let ml = if suite == SUITE_HYBRID {
            self.ml_sk()
                .expanded_key()
                .sign_deterministic(&msg, AUTH_DOMAIN)
                .unwrap()
                .encode()
                .to_vec()
        } else {
            Vec::new()
        };
        encode_oau1(k, suite, &NETWORK_ID, content, &ed, &ml)
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn unhex<const N: usize>(s: &str) -> [u8; N] {
    let v = unhex_vec(s);
    v.try_into()
        .unwrap_or_else(|v: Vec<u8>| panic!("expected {} bytes, got {}", N, v.len()))
}

fn unhex_vec(s: &str) -> Vec<u8> {
    let s = s.trim();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("challenge") => {
            let mut b = [0u8; 64];
            getrandom::getrandom(&mut b).expect("OS RNG");
            println!("{} {}", hex(&b[..32]), hex(&b[32..]));
        }
        Some("verify-pop") => {
            let pk: [u8; 32] = unhex(&a[1]);
            let ch: [u8; 32] = unhex(&a[2]);
            let sig: [u8; 64] = unhex(&a[3]);
            if pop_verify(&pk, &NETWORK_ID, &ch, &sig) {
                println!("POP_OK fp={}", hex(&fingerprint(&pk)));
            } else {
                println!("POP_FAIL");
                std::process::exit(1);
            }
        }
        Some("fp") => println!("{}", hex(&fingerprint(&unhex::<32>(&a[1])))),
        Some("attest") => {
            let o = owner(&a[1]);
            let att = Attestation {
                node_pk: unhex(&a[2]),
                role: a[3].parse().unwrap(),
                permissions: a[4].parse().unwrap(),
                enroll_seq: a[5].parse().unwrap(),
            };
            let suite = if a.get(6).map(String::as_str) == Some("ed") {
                SUITE_ED25519
            } else {
                SUITE_HYBRID
            };
            println!(
                "{}",
                hex(&o.sign(kind::ENROLLMENT, suite, &encode_attestation(&att)))
            );
        }
        Some("revoke") => {
            let o = owner(&a[1]);
            let epoch: u64 = a[2].parse().unwrap();
            let mut fps: Vec<Fp> = a[3..].iter().map(|s| unhex::<8>(s)).collect();
            fps.sort();
            let body = encode_revocation_body(epoch, 1_790_000_000 + epoch, &fps);
            println!("{}", hex(&o.sign(kind::REVOCATION, SUITE_HYBRID, &body)));
        }
        Some("offer") => {
            let (old, new) = (owner(&a[1]), owner(&a[2]));
            let seq: u32 = a[3].parse().unwrap();
            println!(
                "{}",
                hex(&old.sign(
                    kind::OWNERSHIP_TRANSFER,
                    SUITE_HYBRID,
                    &encode_offer(seq, &new.keys())
                ))
            );
        }
        Some("accept") => {
            let new = owner(&a[1]);
            let seq: u32 = a[2].parse().unwrap();
            let offer = unhex_vec(&std::fs::read_to_string(&a[3]).expect("offer file"));
            println!(
                "{}",
                hex(&new.sign(
                    kind::OWNERSHIP_TRANSFER,
                    SUITE_HYBRID,
                    &encode_accept(seq, &offer_digest(&offer))
                ))
            );
        }
        Some("fw-manifest") => {
            let o = owner(&a[1]);
            let version: u32 = a[2].parse().unwrap();
            let img = std::fs::read(&a[3]).expect("image file");
            // The manifest must describe the image: refuse to sign a mismatch.
            assert_eq!(
                image_version(&img),
                Some(version),
                "image version header != requested version"
            );
            let hw_id: [u8; 8] = match a.get(4) {
                Some(h) => h.as_bytes().try_into().expect("hw_id: 8 ASCII bytes"),
                None => HW_ID,
            };
            let m = Manifest {
                version,
                image_len: img.len() as u32,
                sha256: Sha256::digest(&img).into(),
                hw_id,
            };
            eprintln!(
                "manifest: version={} image_len={} sha256={}",
                version,
                img.len(),
                hex(&m.sha256)
            );
            println!(
                "{}",
                hex(&o.sign(kind::FIRMWARE_MANIFEST, SUITE_HYBRID, &encode_manifest(&m)))
            );
        }
        _ => eprintln!("usage: see the header of oasis-operator-key/examples/oasis_enroll.rs"),
    }
}
