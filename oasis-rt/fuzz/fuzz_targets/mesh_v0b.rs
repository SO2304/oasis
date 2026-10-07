//! v0B envelope parsing and processing. Byte 0 picks the mode:
//! 0: arbitrary bytes into a v0B router (parse, window, signature);
//! 1: the rest as the payload of an envelope validly signed by a known peer (accept path);
//! 2: such a valid envelope with one byte XORed (position and mask from the input).
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::mesh::{mesh_v10_pubkey_from_seed, parse_envelope, MeshDecision, MeshEdSeed, MeshPubRegistry, MeshRouter};

const NET: [u8; 8] = *b"OASISnet";

fn router(id: u8, peer: Option<u8>) -> MeshRouter {
    let mut reg = MeshPubRegistry::new();
    if let Some(p) = peer {
        reg.insert([p; 8], mesh_v10_pubkey_from_seed(&MeshEdSeed([p; 32])).unwrap());
    }
    MeshRouter::new_v0b([id; 8], NET, MeshEdSeed([id; 32]), reg)
}

fuzz_target!(|data: &[u8]| {
    common::track("mesh_v0b", data);
    let Some((&mode, rest)) = data.split_first() else { return };
    let mut c = router(0xCC, Some(0xBB));
    match mode % 3 {
        0 => {
            let _ = parse_envelope(rest);
            let _ = c.process(rest);
        }
        1 => {
            let mut b = router(0xBB, None);
            if let Some(env) = b.origin_wrap_v0b(rest) {
                let d = c.process(&env);
                assert!(matches!(d, MeshDecision::Arrived { .. }), "valid envelope refused: {:?}", d);
                // the same envelope again is a replay
                assert!(matches!(c.process(&env), MeshDecision::Drop(_)));
            }
        }
        _ => {
            if rest.len() < 3 {
                return;
            }
            let (ctl, payload) = rest.split_at(3);
            let mut b = router(0xBB, None);
            if let Some(mut env) = b.origin_wrap_v0b(payload) {
                let pos = u16::from_le_bytes([ctl[0], ctl[1]]) as usize % env.len();
                let mask = ctl[2] | 1;
                env[pos] ^= mask;
                // ttl (byte 30) and hops (bytes 31-32) are unsigned by design (range-checked
                // only, MESH_V0B_HEADER_LEN layout in mesh.rs); any other modified byte must
                // be refused.
                let d = c.process(&env);
                if !(30..=32).contains(&pos) {
                    assert!(matches!(d, MeshDecision::Drop(_)), "modified byte {} accepted", pos);
                }
            }
        }
    }
});
