//! OAU1 authority messages (parse, then the full gate with fixed keys: every random
//! input must be refused), policy updates, OFR1 fragments and the bounded reassembler.
#![no_main]
use libfuzzer_sys::fuzz_target;
#[path = "common.rs"]
mod common;
use oasis_rt::authority::{parse_oau1, parse_policy_update, verify_authority, AuthPolicy, AuthorityKeys, MLDSA44_PK_LEN};
use oasis_rt::fragment::{parse_ofr1, Reassembler};

static MLDSA_PK: [u8; MLDSA44_PK_LEN] = [0x5A; MLDSA44_PK_LEN];

fuzz_target!(|data: &[u8]| {
    common::track("authority", data);
    let _ = parse_oau1(data);
    let _ = parse_policy_update(data);
    let keys = AuthorityKeys { ed25519: [0x11; 32], mldsa44: &MLDSA_PK };
    assert!(verify_authority(&AuthPolicy::default(), b"OASISnet", &keys, data).is_err());
    let _ = parse_ofr1(data);
    // the input cut into up to 4 fragments from the same origin
    let mut r = Reassembler::new();
    let n = (data.first().copied().unwrap_or(0) % 4) as usize + 1;
    let step = data.len().div_ceil(n).max(1);
    for (i, frag) in data.chunks(step).enumerate() {
        let _ = r.push([0xBB; 8], i as u64 * 10, frag);
    }
});
