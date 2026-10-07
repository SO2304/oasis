//! Host-side builder for the Phase 1.1 silicon tests (hybrid authority messages).
//! Both operator keys live HERE, on the PC; boards carry only the public keys and
//! the signed blobs (docs/specs/PQ_AUTHORITY_SPEC.md).
//!
//!   pq_payloads edpub                      -> Ed25519 operator public key (hex)
//!   pq_payloads mlpub                      -> ML-DSA-44 operator public key (hex, 1312 B)
//!   pq_payloads rev <suite> <epoch> <fp>.. -> OAU1 revocation (hex); suite = ed | hybrid
//!   pq_payloads policy <kind> <suite>      -> hybrid OAU1 POLICY raising <kind> to <suite>
//!   pq_payloads orv1 <epoch> <fp>..        -> legacy ORV1 (Ed25519 only), for the downgrade test

use ml_dsa::{Keypair, MlDsa44, SigningKey, B32};
use oasis_operator_key::sign_with_seed;
use oasis_rt::authority::{
    encode_oau1, kind, signed_message, AUTH_DOMAIN, SUITE_ED25519, SUITE_HYBRID,
};
use oasis_rt::mesh_revocation::{encode_orv1, encode_revocation_body, signed_message_parts, Fp};

/// Simulated operator seeds (test only; never compiled into firmware). The Ed25519
/// seed is the one of ef_payloads.rs, so boards already trust its public key.
const OPERATOR_SEED: [u8; 32] = [0x0E; 32];
const OPERATOR_MLDSA_SEED: [u8; 32] = [0x0F; 32];
const NETWORK_ID: [u8; 8] = *b"OASISnet";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn ed_pub() -> [u8; 32] {
    let kp = ed25519_compact::KeyPair::from_seed(
        ed25519_compact::Seed::from_slice(&OPERATOR_SEED).unwrap(),
    );
    let mut p = [0u8; 32];
    p.copy_from_slice(kp.pk.as_ref());
    p
}

fn ml_sk() -> SigningKey<MlDsa44> {
    SigningKey::<MlDsa44>::from_seed(&B32::from(OPERATOR_MLDSA_SEED))
}

fn parse_fp(s: &str) -> Fp {
    let mut fp = [0u8; 8];
    for i in 0..8 {
        fp[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("fp hex");
    }
    fp
}

fn suite_of(s: &str) -> u8 {
    match s {
        "ed" => SUITE_ED25519,
        "hybrid" => SUITE_HYBRID,
        _ => panic!("suite must be ed or hybrid"),
    }
}

/// Sign `content` as an OAU1 message of `kind` under `suite` (deterministic ML-DSA).
fn oau1(k: u8, suite: u8, content: &[u8]) -> Vec<u8> {
    let msg = signed_message(k, suite, &NETWORK_ID, content);
    let ed = sign_with_seed(&OPERATOR_SEED, &msg);
    let ml = if suite == SUITE_HYBRID {
        ml_sk()
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

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("edpub") => println!("{}", hex(&ed_pub())),
        Some("mlpub") => println!("{}", hex(&ml_sk().verifying_key().encode())),
        Some("rev") => {
            let suite = suite_of(&a[1]);
            let epoch: u64 = a[2].parse().unwrap();
            let mut fps: Vec<Fp> = a[3..].iter().map(|s| parse_fp(s)).collect();
            fps.sort();
            let body = encode_revocation_body(epoch, 1_790_000_000 + epoch, &fps);
            println!("{}", hex(&oau1(kind::REVOCATION, suite, &body)));
        }
        Some("policy") => {
            let target: u8 = a[1].parse().unwrap();
            let suite = suite_of(&a[2]);
            println!("{}", hex(&oau1(kind::POLICY, SUITE_HYBRID, &[target, suite])));
        }
        Some("orv1") => {
            let epoch: u64 = a[1].parse().unwrap();
            let mut fps: Vec<Fp> = a[2..].iter().map(|s| parse_fp(s)).collect();
            fps.sort();
            let issued_at = 1_790_000_000 + epoch;
            let msg = signed_message_parts(&NETWORK_ID, epoch, issued_at, &fps);
            let sig = sign_with_seed(&OPERATOR_SEED, &msg);
            println!("{}", hex(&encode_orv1(&NETWORK_ID, epoch, issued_at, &fps, &[(ed_pub(), sig)])));
        }
        _ => eprintln!("usage: pq_payloads edpub | mlpub | rev <ed|hybrid> <epoch> <fp>.. | policy <kind> <ed|hybrid> | orv1 <epoch> <fp>.."),
    }
}
