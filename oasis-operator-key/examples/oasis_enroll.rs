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
//!   oasis_enroll mb-order <gw_id> <cmd_seq> <boot_id> <deadline_ms> <unit_hex> <fc> <start_hex> <v1,v2,..>
//!                -> OMB1 Modbus write order (hex, unsigned: the origin board signs it in v0B).
//!                   Encoded by hand, so invalid orders (e.g. fc 5) can be built on purpose.
//!   oasis_enroll forge-v0b <origin_fp_hex> <counter> <payload_hex>
//!                -> v0B envelope claiming <origin_fp>, signed with a fresh random key (attack test)
//!   oasis_enroll mb-raw <unit_hex> <reg_hex> <value> -> bare Modbus RTU FC06 frame with CRC (hex)

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
        Some("mb-order") => {
            let num = |i: usize| -> u64 { a[i].parse().expect("decimal") };
            let hexn = |i: usize| -> u64 { u64::from_str_radix(&a[i], 16).expect("hex") };
            let values: Vec<u16> = a[8].split(',').map(|v| v.parse().expect("value")).collect();
            let fc = num(6) as u8;
            let mut b = Vec::new();
            b.extend_from_slice(&oasis_rt::modbus_gateway::OMB1_MAGIC);
            b.extend_from_slice(&(num(1) as u16).to_le_bytes());
            b.extend_from_slice(&(num(2) as u32).to_le_bytes());
            b.extend_from_slice(&num(3).to_le_bytes());
            b.extend_from_slice(&num(4).to_le_bytes());
            b.push(hexn(5) as u8);
            b.push(fc);
            b.extend_from_slice(&(hexn(7) as u16).to_le_bytes());
            b.push(values.len() as u8);
            for v in &values {
                b.extend_from_slice(&v.to_le_bytes());
            }
            match oasis_rt::modbus_gateway::parse_omb1(&b) {
                Some(o) => eprintln!("order: {:?}", o),
                None => eprintln!("WARNING: not a valid OMB1 (the gateway refuses it at parsing)"),
            }
            println!("{}", hex(&b));
        }
        Some("forge-v0b") => {
            let origin: [u8; 8] = unhex(&a[1]);
            let counter: u64 = a[2].parse().expect("counter");
            let payload = unhex_vec(&a[3]);
            let mut seed = [0u8; 32];
            getrandom::getrandom(&mut seed).expect("OS RNG");
            let mut r = oasis_rt::mesh::MeshRouter::new_v0b(
                origin,
                *b"OASISnet",
                oasis_rt::mesh::MeshEdSeed(seed),
                oasis_rt::mesh::MeshPubRegistry::new(),
            );
            r.set_tx_counter(counter);
            let env = r.origin_wrap_v0b(&payload).expect("wrap");
            eprintln!(
                "forged v0B: origin={} counter~{} len={} (random key)",
                hex(&origin),
                counter,
                env.len()
            );
            println!("{}", hex(&env));
        }
        Some("mb-raw") => {
            let unit = u8::from_str_radix(&a[1], 16).expect("unit hex");
            let reg = u16::from_str_radix(&a[2], 16).expect("reg hex");
            let value: u16 = a[3].parse().expect("value");
            let mut f = vec![unit, 0x06];
            f.extend_from_slice(&reg.to_be_bytes());
            f.extend_from_slice(&value.to_be_bytes());
            f.extend_from_slice(&oasis_rt::modbus_gateway::crc16(&f).to_le_bytes());
            println!("{}", hex(&f));
        }
        _ => eprintln!("usage: see the header of oasis-operator-key/examples/oasis_enroll.rs"),
    }
}
