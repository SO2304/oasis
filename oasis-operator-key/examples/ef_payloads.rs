//! Host-side payload builder for the Part C silicon tests (E/F).
//! The operator key lives HERE, on the PC; boards only carry the signed blobs.
//!
//!   ef_payloads pub                         -> operator public key (hex)
//!   ef_payloads orv1 <epoch> <fp_hex>...    -> signed ORV1 content (hex)
//!   ef_payloads oac1 <actuator_id> <seq> <boot_id> <deadline_ms> <force> <torque> <velocity> <x> <y> <z>
//!                                           -> OAC1 command content (hex)

use oasis_operator_key::sign_with_seed;
use oasis_rt::actuation::{encode_oac1, ActCommand};
use oasis_rt::mesh_revocation::{encode_orv1, signed_message_parts, Fp};

/// Simulated operator seed (test only; never compiled into firmware).
const OPERATOR_SEED: [u8; 32] = [0x0E; 32];
const NETWORK_ID: [u8; 8] = *b"OASISnet";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn op_pub() -> [u8; 32] {
    let kp = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::from_slice(&OPERATOR_SEED).unwrap());
    let mut p = [0u8; 32];
    p.copy_from_slice(kp.pk.as_ref());
    p
}

fn parse_fp(s: &str) -> Fp {
    let mut fp = [0u8; 8];
    for i in 0..8 {
        fp[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("fp hex");
    }
    fp
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("pub") => println!("{}", hex(&op_pub())),
        Some("orv1") => {
            let epoch: u64 = a[1].parse().unwrap();
            let mut fps: Vec<Fp> = a[2..].iter().map(|s| parse_fp(s)).collect();
            fps.sort();
            let issued_at = 1_790_000_000 + epoch; // informational only
            let msg = signed_message_parts(&NETWORK_ID, epoch, issued_at, &fps);
            let sig = sign_with_seed(&OPERATOR_SEED, &msg);
            println!("{}", hex(&encode_orv1(&NETWORK_ID, epoch, issued_at, &fps, &[(op_pub(), sig)])));
        }
        Some("oac1") => {
            let f = |i: usize| a[i].parse::<f32>().unwrap();
            let c = ActCommand {
                actuator_id: a[1].parse().unwrap(),
                cmd_seq: a[2].parse().unwrap(),
                boot_id: a[3].parse().unwrap(),
                deadline_ms: a[4].parse().unwrap(),
                force: f(5),
                torque: f(6),
                velocity: f(7),
                pos: [f(8), f(9), f(10)],
            };
            println!("{}", hex(&encode_oac1(&c)));
        }
        _ => eprintln!("usage: ef_payloads pub | orv1 <epoch> <fp_hex>... | oac1 <aid> <seq> <boot_id> <deadline_ms> <f> <t> <v> <x> <y> <z>"),
    }
}
